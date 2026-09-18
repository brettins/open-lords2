#![allow(unused_imports)]
use super::*;
use super::strength_part::*;
use super::ranking::*;
use super::outcome::*;
use crate::county::{County, MAX_COUNTIES};
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use crate::unit::{UnitKind, Units};
use l2_net::{Quirk, Quirks};

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    const Q: Quirks = Quirks::FAITHFUL;
    use crate::unit::Unit;

    const T: &Tables = &Tables::DEFAULT;

    fn world() -> ([County; MAX_COUNTIES], [Realm; MAX_REALMS], Units) {
        let counties = core::array::from_fn(|_| County::new());
        let mut realms: [Realm; MAX_REALMS] = core::array::from_fn(|_| Realm::new());
        for r in realms.iter_mut().skip(1) {
            r.in_play = true;
            r.strength = 3;
        }
        (counties, realms, Units::default())
    }

    fn own(counties: &mut [County; MAX_COUNTIES], ids: &[usize], realm: u8) {
        for &id in ids {
            counties[id].owner = realm;
        }
    }

    fn army(units: &mut Units, owner: u8) {
        units.spawn(Unit::new(UnitKind::Army, owner, 0, 0));
    }


    #[test]
    fn strength_is_three_a_county_and_one_an_army() {
        let (mut counties, _, mut units) = world();
        own(&mut counties, &[1, 2, 3], 1);
        army(&mut units, 1);
        army(&mut units, 1);
        assert_eq!(strength(&counties, 14, &units, 1), 11);
    }

    #[test]
    fn only_armies_count_not_merchants_or_mobs() {
        let (counties, _, mut units) = world();
        for kind in [UnitKind::PeasantMob, UnitKind::Merchant, UnitKind::Transport] {
            units.spawn(Unit::new(kind, 1, 0, 0));
        }
        assert_eq!(strength(&counties, 14, &units, 1), 0, "three units, none of them an army");
        army(&mut units, 1);
        assert_eq!(strength(&counties, 14, &units, 1), 1);
    }

    #[test]
    fn the_count_cannot_overflow_its_byte() {
        let (mut counties, _, mut units) = world();
        own(&mut counties, &(1..=16).collect::<Vec<_>>(), 1);
        for _ in 1..=150 {
            army(&mut units, 1);
        }
        assert_eq!(strength(&counties, 16, &units, 1), 198);
    }


    #[test]
    fn a_realm_with_nothing_left_is_eliminated_and_an_ai_says_so() {
        let (counties, mut realms, units) = world();
        realms[2].is_human = false;
        let msg = recount_strength(&mut realms, &counties, 14, &units, 2, 1);
        assert!(!realms[2].in_play);
        assert_eq!(realms[2].strength, 0);
        assert_eq!(msg.map(|m| m.group), Some(MSG_AI_ELIMINATED));
        assert_eq!(msg.map(|m| m.from), Some(2));
    }

    #[test]
    fn the_local_player_gets_group_224_instead() {
        let (counties, mut realms, units) = world();
        realms[1].is_human = true;
        let msg = recount_strength(&mut realms, &counties, 14, &units, 1, 1);
        assert_eq!(msg.map(|m| m.group), Some(MSG_DEFEAT));
        assert_eq!(msg.map(|m| (m.from, m.to)), Some((1, 1)));
    }

    /// The third arm of `FUN_0049B42B`: a human who is not the local player.
    #[test]
    fn a_second_human_in_a_network_game_is_told_nothing() {
        let (counties, mut realms, units) = world();
        realms[3].is_human = true;
        let msg = recount_strength(&mut realms, &counties, 14, &units, 3, 1);
        assert_eq!(msg, None, "eliminated in silence");
        assert!(!realms[3].in_play, "but eliminated all the same");
        assert_eq!(realms[3].voice_rotation, 1, "and the voice still rotates");
    }

    #[test]
    fn a_realm_that_still_holds_something_is_not_eliminated() {
        let (mut counties, mut realms, mut units) = world();
        own(&mut counties, &[4], 2);
        assert_eq!(recount_strength(&mut realms, &counties, 14, &units, 2, 1), None);
        assert!(realms[2].in_play);
        assert_eq!(realms[2].strength, 3);

        realms[3].strength = 3;
        army(&mut units, 3);
        assert_eq!(recount_strength(&mut realms, &counties, 14, &units, 3, 1), None);
        assert_eq!(realms[3].strength, 1);
    }

    #[test]
    fn an_already_eliminated_realm_is_skipped_entirely() {
        let (counties, mut realms, units) = world();
        assert!(recount_strength(&mut realms, &counties, 14, &units, 2, 1).is_some());
        let rotation = realms[2].voice_rotation;
        assert_eq!(recount_strength(&mut realms, &counties, 14, &units, 2, 1), None);
        assert_eq!(realms[2].voice_rotation, rotation, "no second message, no second rotation");
    }

    #[test]
    fn realm_zero_and_out_of_range_are_refused() {
        let (counties, mut realms, units) = world();
        assert_eq!(recount_strength(&mut realms, &counties, 14, &units, 0, 1), None);
        assert_eq!(recount_strength(&mut realms, &counties, 14, &units, 99, 1), None);
    }


    #[test]
    fn opponents_remaining_excludes_the_local_player() {
        let (mut counties, mut realms, units) = world();
        own(&mut counties, &[1], 1);
        own(&mut counties, &[2], 2);
        own(&mut counties, &[3], 3);
        for id in 4..MAX_REALMS {
            realms[id].in_play = false;
            realms[id].strength = 0;
        }
        let _ = &units;
        let mut out = Vec::new();
        let r = rank_and_crown(T, &mut realms, 1, Q, &mut out);
        assert_eq!(r.realms_in_play, 3);
        assert_eq!(r.opponents_remaining, 2);
        assert!(!r.sole_survivor());
        assert!(out.is_empty());
    }

    #[test]
    fn one_realm_standing_and_it_is_the_human_is_a_victory() {
        let (_, mut realms, _) = world();
        for id in 2..MAX_REALMS {
            realms[id].in_play = false;
            realms[id].strength = 0;
        }
        realms[1].is_human = true;
        let mut out = Vec::new();
        let r = rank_and_crown(T, &mut realms, 1, Q, &mut out);
        assert!(r.sole_survivor());
        assert_eq!(r.leader, 1);
        assert_eq!(r.opponents_remaining, 0);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].group, MSG_VICTORY);
        assert_eq!(out[0].to, 1);
        assert!(realms[1].crowned_once);
    }

    #[test]
    fn a_lone_ai_taunts_once_and_then_hands_the_human_a_victory() {
        let (_, mut realms, _) = world();
        for id in 1..MAX_REALMS {
            realms[id].in_play = id == 3;
            realms[id].strength = if id == 3 { 3 } else { 0 };
        }
        realms[3].is_human = false;
        let mut out = Vec::new();
        rank_and_crown(T, &mut realms, 1, Q, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].group, MSG_AI_CROWNED, "\"Just call me king.\"");
        assert_eq!(out[0].category, 1, "a taunt, so it cannot set an outcome");
        assert!(!out[0].sets_outcome());
        assert!(realms[3].crowned_once);

        // `Score_RankRealms` runs several times a turn. The guard is now set, so
        // the other branch fires and the *human* is sent group 225 — in a game
        // the human has already lost. Reproduced deliberately.
        out.clear();
        rank_and_crown(T, &mut realms, 1, Q, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].group, MSG_VICTORY);
        assert!(out[0].sets_outcome());
    }

    #[test]
    fn with_nobody_in_play_the_leader_and_the_trailer_are_both_the_array_slot() {
        let (_, mut realms, _) = world();
        for id in 1..MAX_REALMS {
            realms[id].in_play = false;
            realms[id].strength = 0;
        }
        let mut out = Vec::new();
        let r = rank_and_crown(T, &mut realms, 1, Q, &mut out);
        assert_eq!((r.leader, r.trailer), (0, 0));
        assert!(r.sole_survivor(), "0 == 0, and the original crowns g_realms[0]");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].group, MSG_AI_CROWNED, "realm 0 is not human and not yet crowned");
        assert!(realms[0].crowned_once, "the array slot is written, exactly as it is there");
    }


    #[test]
    fn group_225_is_a_win_whatever_else_is_true() {
        let msg = victory_message(1);
        for opponents in 0..=4 {
            let r = Ranking { opponents_remaining: opponents, ..Ranking::default() };
            assert_eq!(outcome_of(msg, 1, r, Q), OutcomeStep::Set(Outcome::Won));
        }
    }

    #[test]
    fn my_own_elimination_with_opponents_left_is_a_loss() {
        let msg = Ending { group: MSG_DEFEAT, from: 1, to: 1, category: CATEGORY_ENDING, variant: 0 };
        let r = Ranking { opponents_remaining: 2, ..Ranking::default() };
        assert_eq!(outcome_of(msg, 1, r, Q), OutcomeStep::Set(Outcome::Lost));
    }

    #[test]
    fn somebody_elses_elimination_ends_nothing() {
        let msg = Ending { group: MSG_AI_ELIMINATED, from: 3, to: 0, category: CATEGORY_ENDING, variant: 0 };
        let r = Ranking { opponents_remaining: 2, ..Ranking::default() };
        assert_eq!(outcome_of(msg, 1, r, Q), OutcomeStep::Set(Outcome::InPlay));
    }

    #[test]
    fn the_last_ais_death_notice_with_no_opponents_left_enqueues_the_victory() {
        let msg = Ending { group: MSG_AI_ELIMINATED, from: 3, to: 0, category: CATEGORY_ENDING, variant: 0 };
        let r = Ranking { opponents_remaining: 0, ..Ranking::default() };
        assert_eq!(outcome_of(msg, 1, r, Q), OutcomeStep::EnqueueVictory);
        assert_eq!(
            outcome_of(victory_message(1), 1, r, Q),
            OutcomeStep::Set(Outcome::Won),
            "the enqueued 225 is what actually sets the outcome"
        );
    }

    #[test]
    fn dying_at_the_same_moment_as_the_last_opponent_is_scored_a_win() {
        let msg = Ending { group: MSG_DEFEAT, from: 1, to: 1, category: CATEGORY_ENDING, variant: 0 };
        let r = Ranking { opponents_remaining: 0, ..Ranking::default() };
        assert_eq!(
            outcome_of(msg, 1, r, Q),
            OutcomeStep::EnqueueVictory,
            "the opponents test is checked before the is-it-me test"
        );
    }

    #[test]
    fn the_outcome_bytes_are_ten_and_eleven() {
        assert_eq!(Outcome::InPlay.value(), 0);
        assert_eq!(Outcome::Won.value(), 10);
        assert_eq!(Outcome::Lost.value(), 11);
        for v in [0u8, 10, 11] {
            assert_eq!(Outcome::from_value(v).map(Outcome::value), Some(v));
        }
        assert_eq!(Outcome::from_value(1), None);
        assert!(!Outcome::InPlay.is_over());
        assert!(Outcome::Won.is_over() && Outcome::Lost.is_over());
    }
}

