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

    let crowning = r.sole_survivor()
        && (quirks.reproduces(Quirk::EmptyGameIsWonBySlotZero) || r.realms_in_play > 0);
    if crowning {
        let winner = r.leader as usize;
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

