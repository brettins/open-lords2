#![allow(unused_imports)]
use super::*;
use super::inbox::*;
use super::replies::*;
use super::ai::*;
use super::tests::*;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use l2_net::Pcg32;

// ------------------------------------------------------- §3 the small helpers

/// Clamp a standing to `[-30, +30]`, which every write site in the original
/// does inline.
fn clamp_standing(v: i8) -> i8 {
    v.clamp(STANDING_MIN, STANDING_MAX)
}

/// Add to `realms[me].pair(them).standing` and clamp.
pub(super) fn move_standing(realms: &mut [Realm], me: u8, them: u8, delta: i8) {
    let p = realms[me as usize].pair_mut(them);
    p.standing = clamp_standing(p.standing.wrapping_add(delta));
}

/// `realm.voiceRotation` is advanced after **every** message a realm sends, so
/// the lord's four recorded takes cycle. This is the
/// `Msg_Enqueue` + rotation pair the original repeats at fourteen sites.
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

// ----------------------------------------------------------------- §4 the init

/// `Diplo_Init` (`0x004A1C53`) — the new game.
///
/// **The one place every field of the pair record is written**, which is what
/// makes `docs/diplomacy.md` §1's field map a reading. An
/// in-play AI realm's opening view of *everyone* is [`STANDING_START_AI`]; a
/// human's and a dead realm's is 0.
///
/// Three details worth having in the model:
///
/// * the inner loop runs `1..6` with **no `other != me` guard**, so a realm
///   ends up with an opinion of itself, at the same opening value;
/// * the opening standing is decided once per realm, from *that realm's* own
///   `strength` and `is_human` — not per pair — so an AI opens at 5 towards a
///   human as well, and a human opens at 0 towards everybody;
/// * `help_price_multiple` opens at **1**, and it is the one field whose zero
///   would be wrong: a never-initialised record would price military help at
///   nothing.
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

// ------------------------------------------------------------ §5 the alliance

/// `Diplo_FormAlliance` (`0x004A1774`) — **six bytes, and that is the whole of
/// what an alliance is**: `allied` and a cleared `grudge` in both directions,
/// and `ally` on both sides.
///
/// It is not shared vision, not shared victory and not an automatic call to
/// arms. What it buys is the two request kinds, an icon on the map and the lord
/// card, and [`action_allowed`] returning false — which suppresses the offence
/// hook and charges grudge instead.
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
///
/// **It does not touch standing.** Every caller applies its own penalty, and
/// they differ: an insult costs 20, ending it from the screen costs 15, and
/// step 2's grudge break goes through [`offend`] for 5.
///
/// The two halves are guarded separately — `if realms[a].ally == b` and
/// `if realms[b].ally == a` — so a one-sided pairing is cleared on the side
/// that holds it and left alone on the side that does not.
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
///
/// > **This corrects `docs/diplomacy.md` §4.1.** The document says it *"drops
/// > any pairing that is one-sided or whose partner has been eliminated"*. It
/// > does not: a pairing where the partner points at **nobody** is **repaired**
/// > — the function writes `ally` back on the partner and sets `allied` both
/// > ways. Only a partner pointing at a *third* realm, or a partner already
/// > marked dead, drops it. So the function's job is to make the matrix agree
/// > with the `ally` bytes, and its tie-break favours the lower index.
///
/// The dead-partner test reads a `handled` array the same loop is filling, in
/// ascending order, so it can only ever see realms **below** the one being
/// walked. A realm allied to a *higher*-indexed eliminated realm is therefore
/// not dropped here at all — the partner still points back, so the else branch
/// takes it and the alliance survives. `docs/bugs.md`. Reproduced.
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
///
/// It answers *"does this act count against me?"* and, when the answer is no
/// because the target is the actor's own ally, it **charges the actor a point
/// of grudge against that ally on the way out**. A target of 0 is allowed;
/// acting on yourself is not.
///
/// `docs/bugs.md` B74: both AI county choosers call this once per county and
/// two of the mission searches call it once per unit slot, so an AI hemmed in
/// by its ally accumulates grudge purely by looking at the map — and the
/// Knight's tolerance of 5 is reached in a handful of turns. **Reproduced**;
/// whether it is intended is not established.
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

// ------------------------------------------------------------- §6 the offence

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
/// Three things the document does not say, all of them from the guard chain:
///
/// 1. **The whole function is a no-op when the offended realm is human.**
///    `g_realms[offended].isHuman == 0` is in the entry guard, beside the range
///    checks. A person's realm keeps no standing towards anybody, so
///    `Diplo_Init` opens a human's row at 0 and why nothing ever moves it.
///    Everything that reads a standing reads an AI's.
/// 2. **The Bishop's guard covers `warTarget` as well as `atWar`.**
///    `docs/diplomacy.md` §5 says the test *"guards only the `atWar` write"*;
///    the `atWar` store is a comma-expression inside the same `&&` chain as the
/// `warTarget == 0` test, so a betrayed Bishop sets **neither**. He breaks
///    the alliance, takes the standing hit, sends group 182 and stays
///    technically at peace with a realm he has not even resolved to attack.
/// 3. **Betraying an ally is the only act in the game that costs reputation
///    with third parties** — [`offend_all`] is reached from nowhere else.
///
/// Returns the letters. The offence amounts are in [`offence`].
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
///
/// It clamps at the floor only, walks 1..5 with no in-play test, and is reached
/// from exactly one place: [`offend`]'s betrayal branch.
pub fn offend_all(realms: &mut [Realm], realm: u8, amount: i8) {
    for id in 1..MAX_REALMS.min(realms.len()) {
        if id as u8 == realm {
            continue;
        }
        let p = realms[id].pair_mut(realm);
        p.standing = p.standing.wrapping_sub(amount).max(STANDING_MIN);
    }
}

// --------------------------------------------------------------- §7 the inbox

