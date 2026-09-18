#![allow(unused_imports)]
use super::*;
use super::standing::*;
use super::inbox::*;
use super::replies::*;
use super::tests::*;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use l2_net::Pcg32;


/// **AI turn step 2** — `AI_Diplomacy` (`0x004A0C1D`), the real diplomacy
/// driver, and the writer of three of the four starved fields.
///
/// and past the lord's [`crate::tables::AI_PERSONALITY_GRUDGE_TOLERANCE`] —
/// Knight **5**, Baron 10, Countess 15, Bishop 20 — the alliance breaks,
/// [`offend`] fires for 5 and group 181 goes out. Note the order: the alliance
/// is broken *before* `offend` runs, so `offend`'s betrayal branch cannot see
/// it and the extra 15, the `atWar` flag and [`offend_all`] never happen. A
/// grudge break is much cheaper than a betrayal.
///
/// Against another AI it forms the alliance with no message; against a
/// person it sets `offer_pending` and sends group 180 with the prompt layout.
pub fn ai_diplomacy(
    realms: &mut [Realm],
    tables: &Tables,
    me: u8,
    year: i32,
    rank_leader: u8,
) -> Vec<Letter> {
    let mut out = Vec::new();
    let n = MAX_REALMS.min(realms.len());
    for other in 1..n {
        if realms[other].is_human {
            continue;
        }
        let p = realms[me as usize].pair_mut(other as u8);
        p.standing = p.standing.wrapping_add(1).min(STANDING_MAX);
    }

    let active = realms_active(realms);
    let ally = realms[me as usize].ally;
    if ally != 0 {
        let tolerance =
            tables.ai_personality(realms[me as usize].lord).map(|p| p.grudge_tolerance);
        if realms[ally as usize].strength == 0 {
            break_alliance(realms, me, ally);
            return out;
        }
        if realms[me as usize].pair(ally).standing < -10 {
            bump_grudge(realms, me, ally, 10);
        }
        if active < 3 {
            bump_grudge(realms, me, ally, 25);
        }
        if rank_leader == ally && realms[ally as usize].share_of_map_pct >= 0x15 {
            bump_grudge(realms, me, ally, 1);
        }
        if let Some(tolerance) = tolerance {
            if (realms[me as usize].pair(ally).grudge as i32) > tolerance {
                break_alliance(realms, me, ally);
                out.extend(offend(realms, me, ally, offence::GRUDGE_BREAK));
                out.push(speak(
                    realms,
                    me,
                    ally,
                    group::ALLIANCE_ENDED,
                    category::LETTER,
                    0,
                ));
            }
        }
    }

    if realms[me as usize].ally != 0 || year <= 1268 || realms[me as usize].rank == 1 {
        return out;
    }
    let candidate = pick_ally_candidate(realms, me);
    realms[me as usize].ally_candidate = candidate;
    if candidate == 0 {
        return out;
    }
    realms[me as usize].offer_timer = realms[me as usize].offer_timer.saturating_add(1);
    let Some(interval) = tables.ai_personality(realms[me as usize].lord).map(|p| p.offer_interval)
    else {
        return out;
    };
    if (realms[me as usize].offer_timer as i32) < interval {
        return out;
    }
    realms[me as usize].offer_timer = 0;
    if !realms[candidate as usize].is_human {
        form_alliance(realms, me, candidate);
    } else if realms[candidate as usize].ally == 0 {
        realms[me as usize].offer_pending = true;
        out.push(speak(
            realms,
            me,
            candidate,
            group::ALLIANCE_OFFER,
            category::ALLIANCE_PROMPT,
            0,
        ));
    }
    out
}

fn bump_grudge(realms: &mut [Realm], me: u8, them: u8, by: u8) {
    let p = realms[me as usize].pair_mut(them);
    p.grudge = p.grudge.wrapping_add(by);
}

/// `Diplo_PickAllyCandidate` (`0x004A1241`) — the **best-ranked** realm that is
/// in play, not ranked first, unallied, not already being courted by somebody
/// else, not at war with me, and whose standing with me is above −10. 0 for
/// none.
pub fn pick_ally_candidate(realms: &[Realm], me: u8) -> u8 {
    let mut best = 0u8;
    let mut best_rank = 6u8;
    for id in 1..MAX_REALMS.min(realms.len()) {
        let r = &realms[id];
        if r.strength == 0
            || id as u8 == me
            || r.rank == 1
            || r.ally != 0
            || r.offer_pending
            || realms[me as usize].pair(id as u8).standing <= -11
            || realms[me as usize].pair(id as u8).at_war
            || r.rank >= best_rank
        {
            continue;
        }
        best = id as u8;
        best_rank = r.rank;
    }
    best
}

