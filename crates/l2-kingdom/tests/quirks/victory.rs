#![allow(unused_imports)]
use super::*;
use super::economy::*;
use super::counties::*;
use super::wire::*;
use l2_kingdom::county::County;
use l2_kingdom::kingdom::Kingdom;
use l2_kingdom::realm::Realm;
use l2_kingdom::tables::{Season, Tables, Weather};
use l2_kingdom::{Quirk, Quirks};


#[test]
fn b42_a_band_that_made_an_offer_stands_off_the_end_of_the_map_or_does_not() {
    let (faithful, fixed) = pair(Quirk::MercenaryBandOvershoots);

    let overshot = |quirks: Quirks| {
        let mut bands = l2_kingdom::MercenaryBands::init(14);
        let mut counties: [County; l2_kingdom::county::MAX_COUNTIES] =
            core::array::from_fn(|_| County::new());
        let mut seen = false;
        for _ in 0..64 {
            bands.advance(&mut counties, 14, quirks);
            for band in 1..l2_kingdom::mercenary::BAND_SLOTS {
                if bands.band_raw(band).next_county > 14 {
                    seen = true;
                }
            }
        }
        seen
    };

    assert!(overshot(faithful), "reproduced: a band sits at countyCount + 1 for a season");
    assert!(!overshot(fixed), "fixed: the second increment gets the wrap guard the first has");
}


#[test]
fn b51_a_game_with_nobody_in_it_is_won_by_the_array_slot_or_by_nobody() {
    let (faithful, fixed) = pair(Quirk::EmptyGameIsWonBySlotZero);

    let endings = |quirks: Quirks| {
        let mut realms: [Realm; l2_kingdom::realm::MAX_REALMS] =
            core::array::from_fn(|_| Realm::new());
        for r in realms.iter_mut() {
            r.in_play = false;
            r.strength = 0;
        }
        let mut out = Vec::new();
        l2_kingdom::victory::rank_and_crown(T, &mut realms, 1, quirks, &mut out);
        out
    };

    assert!(!endings(faithful).is_empty(), "reproduced: the empty game crowns realm 0");
    assert!(endings(fixed).is_empty(), "fixed: nobody standing, nobody crowned");
}

#[test]
fn b52_the_human_wins_a_game_the_human_is_dead_in_or_does_not() {
    let (faithful, fixed) = pair(Quirk::DeadHumanCanStillWin);

    let second_call = |quirks: Quirks| {
        let mut realms: [Realm; l2_kingdom::realm::MAX_REALMS] =
            core::array::from_fn(|_| Realm::new());
        for r in realms.iter_mut() {
            r.in_play = false;
            r.strength = 0;
        }
        realms[2].in_play = true;
        realms[2].strength = 9;
        realms[2].is_human = false;
        realms[2].lord = 1;

        let mut out = Vec::new();
        l2_kingdom::victory::rank_and_crown(T, &mut realms, 1, quirks, &mut out);
        out.clear();
        l2_kingdom::victory::rank_and_crown(T, &mut realms, 1, quirks, &mut out);
        out
    };

    let a = second_call(faithful);
    assert!(
        a.iter().any(|e| e.group == l2_kingdom::victory::MSG_VICTORY),
        "reproduced: the second call sends the dead human group 225"
    );
    assert!(
        second_call(fixed).is_empty(),
        "fixed: a victory goes only to a local player who is the realm left standing"
    );
}

#[test]
fn b53_dying_with_the_last_opponent_is_a_win_or_a_loss() {
    use l2_kingdom::victory::{Ending, Outcome, OutcomeStep, Ranking, CATEGORY_ENDING};
    let (faithful, fixed) = pair(Quirk::MutualDestructionIsAWin);

    let mine =
        Ending { group: l2_kingdom::victory::MSG_DEFEAT, from: 1, to: 1, category: CATEGORY_ENDING, variant: 0 };
    let ranking = Ranking { opponents_remaining: 0, ..Ranking::default() };

    assert_eq!(
        l2_kingdom::victory::outcome_of(mine, 1, ranking, faithful),
        OutcomeStep::EnqueueVictory,
        "reproduced: the opponents test is asked first, so my own defeat wins the game"
    );
    assert_eq!(
        l2_kingdom::victory::outcome_of(mine, 1, ranking, fixed),
        OutcomeStep::Set(Outcome::Lost),
        "fixed: whose defeat this is, first"
    );
}

#[test]
fn b53_an_ordinary_elimination_is_the_same_step_either_way() {
    use l2_kingdom::victory::{Ending, Ranking, CATEGORY_ENDING};
    let (faithful, fixed) = pair(Quirk::MutualDestructionIsAWin);
    for opponents in 1..=4u8 {
        for from in 1..=5u8 {
            let msg = Ending {
                group: l2_kingdom::victory::MSG_AI_ELIMINATED,
                from,
                to: 0,
                category: CATEGORY_ENDING,
                variant: 0,
            };
            let r = Ranking { opponents_remaining: opponents, ..Ranking::default() };
            assert_eq!(
                l2_kingdom::victory::outcome_of(msg, 1, r, faithful),
                l2_kingdom::victory::outcome_of(msg, 1, r, fixed),
                "{opponents} left, message from {from}"
            );
        }
    }
}


