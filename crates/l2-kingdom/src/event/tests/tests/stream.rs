#![allow(unused_imports)]
use super::*;
use super::deck::*;
use super::super::*;
use std::collections::BTreeSet;

#[test]
fn the_stream_advances_identically_whoever_owns_the_counties() {
    let run = |human: bool| {
        let mut rng = Pcg32::from_seed(1234);
        let mut c = vec![County::new(); 17];
        let mut purses = vec![RealmPurse::default(); 6];
        for id in 1..=14 {
            c[id].owner = if id % 2 == 1 { 1 } else { 0 };
            c[id].grain = 5_000;
            c[id].herd = 500;
            c[id].population = 500;
            c[id].happiness = 60;
            c[id].health_meter = 60;
            c[id].tax_shown = 100;
            c[id].fields_grain = 6;
        }
        let mut out = Vec::new();
        for _ in 0..40 {
            roll_all(T, 
                &mut c,
                14,
                &|_| human,
                &mut purses,
                1300,
                Season::Spring,
                &mut rng,
                Q,
                &mut out,
            );
        }
        (rng, out.len())
    };
    let (a, events_a) = run(true);
    let (b, events_b) = run(false);
    assert_eq!(a, b, "the generator must be in the same place either way");
    assert!(events_a > 0, "somebody should have drawn something");
    assert_eq!(events_b, 0, "and an all-AI kingdom draws nothing");
}

#[test]
fn the_season_draws_exactly_one_number_however_many_counties_there_are() {
    for n in [1usize, 4, 14, 16] {
        let mut rng = Pcg32::from_seed(9);
        let mut c = vec![County::new(); 17];
        let mut purses = vec![RealmPurse::default(); 6];
        let mut out = Vec::new();
        roll_all(T, &mut c, n, &|_| true, &mut purses, 1300, Season::Spring, &mut rng, Q, &mut out);
        let mut reference = Pcg32::from_seed(9);
        reference.below(EVENT_SEED_BOUND);
        assert_eq!(rng, reference, "kingdom of {n}");
    }
}

#[test]
fn the_same_seed_rolls_the_same_events() {
    let run = || {
        let mut rng = Pcg32::from_seed(7);
        let mut c = vec![County::new(); 17];
        let mut purses = vec![RealmPurse::default(); 6];
        for id in 1..=14 {
            c[id].owner = 1;
            c[id].grain = 900;
            c[id].herd = 200;
            c[id].population = 500;
        }
        let mut out = Vec::new();
        for _ in 0..10 {
            roll_all(T, 
                &mut c,
                14,
                &|_| true,
                &mut purses,
                1300,
                Season::Autumn,
                &mut rng,
                Q,
                &mut out,
            );
        }
        out
    };
    assert_eq!(run(), run());
}

#[test]
fn no_two_neighbouring_counties_can_draw_in_the_same_season() {
    for pair in EVENT_DECK.windows(2) {
        assert!(pair[1].0 - pair[0].0 >= 8, "slots {} and {}", pair[0].0, pair[1].0);
    }
}

#[test]
fn only_odd_numbered_counties_can_ever_draw_an_event() {
    for slot in EVENT_DECK.iter().map(|(s, _)| *s) {
        assert_eq!(slot % 2, 1, "slot {slot} is even");
    }
    let mut drew: Vec<usize> = Vec::new();
    for seed in 0..EVENT_SEED_BOUND as usize {
        let mut index = seed * 2;
        for county in 1..=crate::county::MAX_COUNTY_ID as usize {
            index += 1;
            if index > EVENT_DECK_SLOTS - 1 {
                index = 0;
            }
            if deck_slot(index).is_some() && !drew.contains(&county) {
                drew.push(county);
            }
        }
    }
    drew.sort_unstable();
    assert_eq!(drew, vec![1, 3, 5, 7, 9, 11, 13, 15], "the even counties never come up");
}

#[test]
fn a_kingdom_of_even_numbered_counties_never_sees_an_event() {
    let mut rng = Pcg32::from_seed(4242);
    let mut c = vec![County::new(); 17];
    let mut purses = vec![RealmPurse::default(); 6];
    for id in 1..=16 {
        c[id].owner = if id % 2 == 0 { 1 } else { 0 };
        c[id].grain = 5_000;
        c[id].herd = 500;
        c[id].population = 500;
        c[id].happiness = 60;
        c[id].health_meter = 60;
        c[id].tax_shown = 100;
    }
    let mut out = Vec::new();
    for _ in 0..500 {
        roll_all(T, &mut c, 16, &|_| true, &mut purses, 1300, Season::Spring, &mut rng, Q, &mut out);
    }
    assert!(out.is_empty(), "five hundred seasons and not one event");
}

#[test]
fn a_letter_nobody_read_is_still_waiting_seasons_later() {
    let mut c = county(1);
    c.herd = 500;
    let mut purse = RealmPurse::default();
    assert!(fire(&mut c, 1, &mut purse, EventKind::Wolves, Season::Spring, Q));
    assert!(c.event_fired);

    let mut counties = vec![County::new(); 17];
    counties[1] = c;
    let mut rng = Pcg32::from_seed(11);
    let mut purses = vec![RealmPurse::default(); 6];
    let mut out = Vec::new();
    for _ in 0..10 {
        roll_all(T, &mut counties, 16, &|_| false, &mut purses, 1300, Season::Summer, &mut rng, Q, &mut out);
    }
    assert!(out.is_empty(), "nothing new was dealt");
    assert!(counties[1].event_fired, "the letter is still waiting");
    assert_eq!(counties[1].event_id, EventKind::Wolves.id());
    assert_eq!(counties[1].event_herd_pct, 0, "but the modifier was spent and cleared");
    assert!(!counties[1].tax_suppressed);
}

#[test]
fn a_new_event_that_cannot_fire_takes_the_old_letter_with_it() {
    let mut c = county(1);
    c.herd = 500;
    let mut purse = RealmPurse::default();
    assert!(fire(&mut c, 1, &mut purse, EventKind::Wolves, Season::Spring, Q));
    c.herd = 0; // the wolves and a bad winter between them
    assert!(!fire(&mut c, 1, &mut purse, EventKind::MadCows, Season::Spring, Q));
    assert!(!c.event_fired, "the Wolves letter went with the failed Mad cows");
    assert_eq!(c.event_id, 0);
}

#[test]
fn an_event_is_consumed_by_the_pass_that_reads_it() {
    let mut c = county(1);
    c.grain = 1000;
    c.weather = crate::tables::Weather::Cloudy;
    let mut purse = RealmPurse::default();
    assert!(fire(&mut c, 1, &mut purse, EventKind::Rats, Season::Winter, Q));
    crate::land::grain_season_tick(T, &mut c, Season::Winter, true, Q);
    assert_eq!(c.grain, 600, "a 40% winter loss");
    assert_eq!(c.event_grain_pct, 0);
}

#[test]
fn every_id_round_trips_through_from_id() {
    for kind in EVENT_KINDS {
        assert_eq!(EventKind::from_id(kind.id()), Some(kind));
    }
    assert_eq!(EventKind::from_id(0), None);
    assert_eq!(EventKind::from_id(0x8F), None, "the gap between the two runs");
}

