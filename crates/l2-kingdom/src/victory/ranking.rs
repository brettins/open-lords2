#![allow(unused_imports)]
use super::*;
use super::strength_part::*;
use super::outcome::*;
use super::tests_part::*;
use crate::county::{County, MAX_COUNTIES};
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use crate::unit::{UnitKind, Units};
use l2_net::{Quirk, Quirks};

/// `Score_RankRealms` (`0x0049AA0E`) — score, rank, and crown the last realm
/// standing.
///
/// The ranking itself is [`crate::ai::rank_realms`], which was already here. This
/// adds the three globals that function drops on the floor and the crowning at
/// the bottom of it.
///
/// Five callers in the original: `Turn_Tick`'s phase 7, `Game_NewGame`,
/// `Turn_AdvancePhase`, [`recount_strength`], and one UI path. It is **not** in
/// `Season_Advance`'s call list — `docs/kingdom.md` §3.4 said it was and
/// `crates/l2-kingdom/src/phase.rs` already records the correction.
pub fn rank_and_crown(
    t: &Tables,
    realms: &mut [Realm; MAX_REALMS],
    local_player: u8,
    quirks: Quirks,
    out: &mut Vec<Ending>,
) -> Ranking {
    crate::ai::rank_realms(t, realms);

    let mut r = Ranking::default();
    for id in 1..MAX_REALMS {
        if !realms[id].in_play {
            continue;
        }
        r.realms_in_play += 1;
        if id as u8 != local_player {
            r.opponents_remaining += 1;
        }
    }
    // `g_rankLeader` / `g_rankTrailer` are read off the *sorted* table, but the
    // table's live entries are exactly the in-play realms and the sort is by
    // rank, so the first and last live entries are the rank-1 and rank-n realms.
    // Walking ranks avoids materialising a sorted array whose order would have
    // to be argued about (`docs/netcode.md` §3).
    for id in 1..MAX_REALMS {
        if !realms[id].in_play {
            continue;
        }
        if r.leader == 0 || realms[id].rank < realms[r.leader as usize].rank {
            r.leader = id as u8;
        }
        if r.trailer == 0 || realms[id].rank > realms[r.trailer as usize].rank {
            r.trailer = id as u8;
        }
    }

    // **Switchable** — [`Quirk::EmptyGameIsWonBySlotZero`], `docs/bugs.md` B51.
    // With nobody in play leader and trailer are both 0, `0 == 0` passes, and
    // the original crowns `g_realms[0]`, The fixed path
    // requires somebody to be standing before anyone is crowned.
    let crowning = r.sole_survivor()
        && (quirks.reproduces(Quirk::EmptyGameIsWonBySlotZero) || r.realms_in_play > 0);
    if crowning {
        let winner = r.leader as usize;
        // **Switchable** — [`Quirk::DeadHumanCanStillWin`], `docs/bugs.md` B52.
        // The `else` limb is reached twice for an AI winner: `Score_RankRealms`
        // runs many times a turn, and the second call finds `crowned_once` set,
        // falls through, and sends the *human* group 225 *"Victory!"* — in a
        // game the human is not in. The fixed path sends the victory only to a
        // local player
        let victory_is_the_local_players =
            quirks.reproduces(Quirk::DeadHumanCanStillWin) || winner == local_player as usize;
        if !realms[winner].crowned_once && !realms[winner].is_human {
            realms[winner].crowned_once = true;
            out.push(Ending {
                group: MSG_AI_CROWNED,
                from: r.leader,
                to: 0,
                // Category 1, not 0x0E: an AI's coronation is a taunt and
                // cannot set an outcome.
                category: 1,
                variant: voice_variant(&realms[winner]),
            });
            advance_voice(&mut realms[winner]);
        } else {
            realms[winner].crowned_once = true;
            if victory_is_the_local_players {
                out.push(Ending {
                    group: MSG_VICTORY,
                    from: 0,
                    to: local_player,
                    category: CATEGORY_ENDING,
                    variant: 0,
                });
            }
        }
    }
    r
}

