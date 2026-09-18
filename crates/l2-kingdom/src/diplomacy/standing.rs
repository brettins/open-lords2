#![allow(unused_imports)]
use super::*;
use super::inbox::*;
use super::replies::*;
use super::ai::*;
use super::tests::*;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use l2_net::Pcg32;


fn clamp_standing(v: i8) -> i8 {
    v.clamp(STANDING_MIN, STANDING_MAX)
}

pub(super) fn move_standing(realms: &mut [Realm], me: u8, them: u8, delta: i8) {
    let p = realms[me as usize].pair_mut(them);
    p.standing = clamp_standing(p.standing.wrapping_add(delta));
}

pub(super) fn speak(realms: &mut [Realm], from: u8, to: u8, group: u16, cat: u8, county: u8) -> Letter {
    let realm = &mut realms[from as usize];
    let letter = Letter {
        from,
        to,
        group,
        variant: realm.message_variant(),
        category: cat,
        county,
        payload: 0,
    };
    realm.advance_voice();
    letter
}


/// `Diplo_Init` (`0x004A1C53`) — the new game.
pub fn init(realms: &mut [Realm], diplomacy: &mut Diplomacy) {
    diplomacy.help_price = 0;
    diplomacy.help_county = 0;
    diplomacy.clear_inboxes();
    for me in 1..MAX_REALMS.min(realms.len()) {
        let opening =
            if realms[me].strength == 0 || realms[me].is_human { 0 } else { STANDING_START_AI };
        realms[me].war_target = 0;
        realms[me].ally = 0;
        for other in 1..MAX_REALMS {
            let p = realms[me].pair_mut(other as u8);
            *p = crate::realm::Pair::new();
            p.standing = opening;
        }
    }
}

/// `Diplo_DefaultTarget` (`0x004A1E6C`) — the first in-play realm that is not
/// the local player, or 0. It is what the diplomacy screen opens on.
pub fn default_target(realms: &[Realm], local_player: u8) -> u8 {
    for id in 1..MAX_REALMS.min(realms.len()) {
        if realms[id].strength != 0 && local_player as usize != id {
            return id as u8;
        }
    }
    0
}


/// `Diplo_FormAlliance` (`0x004A1774`) — **six bytes, and that is the whole of
/// what an alliance is**: `allied` and a cleared `grudge` in both directions,
/// and `ally` on both sides.
///
/// **Exclusivity is the width of the
/// field.** `realm +0x81` is one byte.
pub fn form_alliance(realms: &mut [Realm], a: u8, b: u8) {
    realms[a as usize].pair_mut(b).allied = true;
    realms[a as usize].pair_mut(b).grudge = 0;
    realms[b as usize].pair_mut(a).allied = true;
    realms[b as usize].pair_mut(a).grudge = 0;
    realms[a as usize].ally = b;
    realms[b as usize].ally = a;
}

/// `Diplo_BreakAlliance` (`0x004A1A54`) — clear `allied` and `ally` on
/// whichever sides point at the other.
pub fn break_alliance(realms: &mut [Realm], a: u8, b: u8) {
    if realms[a as usize].ally == b {
        realms[a as usize].pair_mut(b).allied = false;
        realms[a as usize].ally = 0;
    }
    if realms[b as usize].ally == a {
        realms[b as usize].pair_mut(a).allied = false;
        realms[b as usize].ally = 0;
    }
}

/// `Diplo_ReconcileAlliances` (`0x004A1847`), called from `Turn_Tick`.
pub fn reconcile_alliances(realms: &mut [Realm]) {
    let n = MAX_REALMS.min(realms.len());
    for me in 1..n {
        for other in 1..MAX_REALMS {
            realms[me].pair_mut(other as u8).allied = false;
        }
    }
    let mut handled = [false; MAX_REALMS];
    let mut ally: [u8; MAX_REALMS] = [0; MAX_REALMS];
    for id in 1..n {
        ally[id] = realms[id].ally;
    }
    for me in 1..n {
        if realms[me].strength == 0 {
            handled[me] = true;
            continue;
        }
        let partner = ally[me] as usize;
        if partner == 0 || partner >= MAX_REALMS {
            continue;
        }
        if handled[partner] || (ally[partner] != 0 && ally[partner] as usize != me) {
            realms[me].ally = 0;
            realms[me].pair_mut(partner as u8).allied = false;
            handled[me] = true;
            ally[me] = 0;
        } else {
            realms[me].ally = partner as u8;
            realms[partner].ally = me as u8;
            realms[me].pair_mut(partner as u8).allied = true;
            realms[partner].pair_mut(me as u8).allied = true;
        }
    }
}

/// `Diplo_ActionAllowed` (`0x004A16F7`) — **not a predicate.**
pub fn action_allowed(realms: &mut [Realm], actor: u8, target: u8) -> bool {
    if target == 0 {
        return true;
    }
    if realms[actor as usize].ally == target {
        let p = realms[actor as usize].pair_mut(target);
        p.grudge = p.grudge.wrapping_add(1);
        return false;
    }
    actor != target
}


/// `Diplo_Offend(offended, offender, amount)` (`0x004A1EE1`) — **the single
/// hook every relationship-damaging act goes through.**
///
/// ```text
/// if the offender was my ally:
///       break the alliance
///       set pair.atWar and my warTarget      -- UNLESS I am the Bishop
///       offend_all(offender, 15)             -- everyone else's opinion drops 15
///       send group 182 "Broken alliance."
///       amount += 15
///
/// standing -= amount, clamped to [-30, +30]
///
/// if not already at war, standing has bottomed out at -30, and the offender is
/// HUMAN:  warningsSent 0->1: group 189; 1->2: group 190; 2->3: group 191 and war
/// ```
///
///    `docs/diplomacy.md` §5 says the test *"guards only the `atWar` write"*;
///    the `atWar` store is a comma-expression inside the same `&&` chain as the
/// `warTarget == 0` test, so a betrayed Bishop sets **neither**. He breaks
///    the alliance, takes the standing hit, sends group 182 and stays
///    technically at peace with a realm he has not even resolved to attack.
pub fn offend(realms: &mut [Realm], offended: u8, offender: u8, amount: i8) -> Vec<Letter> {
    let mut out = Vec::new();
    let n = MAX_REALMS.min(realms.len()) as u8;
    if offended == 0
        || offended >= n
        || offender == 0
        || offender >= n
        || offender == offended
        || realms[offended as usize].strength == 0
        || realms[offended as usize].is_human
    {
        return out;
    }
    let mut amount = amount;
    if realms[offender as usize].ally == offended {
        break_alliance(realms, offended, offender);
        if realms[offended as usize].lord != crate::realm::LORD_BISHOP {
            realms[offended as usize].pair_mut(offender).at_war = true;
            if realms[offended as usize].war_target == 0 {
                realms[offended as usize].war_target = offender;
            }
        }
        offend_all(realms, offender, offence::BETRAYAL);
        out.push(speak(
            realms,
            offended,
            offender,
            group::ALLIANCE_BROKEN,
            category::LETTER,
            0,
        ));
        amount = amount.wrapping_add(offence::BETRAYAL);
    }
    move_standing(realms, offended, offender, amount.wrapping_neg());

    let pair = *realms[offended as usize].pair(offender);
    if pair.at_war || pair.standing != STANDING_MIN || !realms[offender as usize].is_human {
        return out;
    }
    let group = match pair.warnings_sent {
        0 => group::WARNING_FIRST,
        1 => group::WARNING_SECOND,
        2 => group::NOTICE_OF_REVENGE,
        _ => return out,
    };
    realms[offended as usize].pair_mut(offender).warnings_sent = pair.warnings_sent + 1;
    if group == group::NOTICE_OF_REVENGE {
        realms[offended as usize].pair_mut(offender).at_war = true;
        if realms[offended as usize].war_target == 0 {
            realms[offended as usize].war_target = offender;
        }
    }
    out.push(speak(realms, offended, offender, group, category::LETTER, 0));
    out
}

/// `Diplo_OffendAll` (`0x004A24D1`) — lower **every other realm's** standing
/// towards this one.
pub fn offend_all(realms: &mut [Realm], realm: u8, amount: i8) {
    for id in 1..MAX_REALMS.min(realms.len()) {
        if id as u8 == realm {
            continue;
        }
        let p = realms[id].pair_mut(realm);
        p.standing = p.standing.wrapping_sub(amount).max(STANDING_MIN);
    }
}


