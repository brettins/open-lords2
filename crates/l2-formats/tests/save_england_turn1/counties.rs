#![allow(unused_imports)]
use super::*;
use super::globals::*;
use super::merchants::*;
use l2_formats::save::COUNTY_RECORDS;
use l2_testkit::{england, england_county_of_realm, ENGLAND_TURN1_COUNTIES};

#[test]
fn the_england_map_starts_with_five_owned_counties_one_realm_each() {
    let save = england!();
    let counties = save.counties().unwrap();
    assert_eq!(counties.len(), COUNTY_RECORDS);

    let owned: Vec<usize> = counties.iter().filter(|c| c.is_owned()).map(|c| c.index).collect();
    assert_eq!(owned, ENGLAND_TURN1_COUNTIES, "the five starting counties");

    let mut realms: Vec<u8> = counties.iter().filter(|c| c.is_owned()).map(|c| c.owner).collect();
    realms.sort_unstable();
    assert_eq!(realms, [1, 2, 3, 4, 5], "one county each, in some order");

    let real: Vec<_> = counties.iter().filter(|c| c.is_county()).collect();
    assert_eq!(real.len(), 14, "fourteen counties on the England map");
    assert_eq!(real.iter().filter(|c| !c.is_owned()).count(), 9, "nine unowned");

    for i in [0, 15, 16] {
        assert!(!counties[i].is_county(), "record {i} is not a county");
    }
}

#[test]
fn owned_counties_store_seventy_two_and_unowned_seventy_seven() {
    let save = england!();
    for c in save.counties().unwrap().iter().filter(|c| c.is_county()) {
        let expected = if c.is_owned() { 72 } else { 77 };
        assert_eq!(c.happiness, expected, "county {} happiness", c.index);
        assert_eq!(c.happiness_last, 65, "county {} last turn", c.index);
        assert_eq!((c.shown_tax, c.shown_health, c.shown_ration), (5, 1, 1));
        assert_eq!(
            c.shown_events,
            if c.is_owned() { 0 } else { 5 },
            "county {}: the unowned bonus arrives through shownEvents",
            c.index
        );
        assert_eq!(c.health_meter, 67);
        assert_eq!(c.health_band, 3);
    }
}

#[test]
fn population_grew_from_the_same_starting_number_everywhere() {
    let save = england!();
    for c in save.counties().unwrap().iter().filter(|c| c.is_county()) {
        assert_eq!(c.pop_last, 417, "county {}", c.index);
        assert_eq!(c.deaths, 45, "county {}", c.index);
        if c.is_owned() {
            assert_eq!(c.births, 63, "owned county {}", c.index);
            assert_eq!(c.population, 435, "417 + 63 - 45");
            assert_eq!(c.pop_band, 18);
        } else {
            assert_eq!(c.births, 84, "unowned county {}", c.index);
            assert_eq!(c.population, 456, "417 + 84 - 45");
            assert_eq!(c.pop_band, 19);
        }
    }
}

#[test]
fn turn_one_has_no_tax_and_castles_only_where_someone_lives() {
    let save = england!();
    for c in save.counties().unwrap().iter().filter(|c| c.is_county()) {
        assert_eq!(c.tax_rate, 0, "county {}", c.index);
        assert_eq!(c.tax_collected, 0, "county {}", c.index);
        assert_eq!(c.weather, 3, "cloudy everywhere");
        assert_eq!(c.fertility, 0, "basic farming leaves fertility at zero");
        assert_eq!(
            c.castle_type != 0,
            c.is_owned(),
            "county {} has a castle iff it is owned",
            c.index
        );
    }
}

#[test]
fn the_documented_food_split_is_what_the_file_stores() {
    let save = england!();
    let unowned: Vec<_> =
        save.counties().unwrap().into_iter().filter(|c| c.is_county() && !c.is_owned()).collect();
    assert_eq!(unowned.len(), 9);
    for c in &unowned {
        assert_eq!(c.population, 456);
        assert_eq!(c.herd, 67, "county {}", c.index);
        assert_eq!(c.herd_eaten, 13, "county {}: DivCeil(456 - 67*5, 10)", c.index);
        assert_eq!(c.ration_wanted, 3, "normal rations");
    }
}

/// The realm records say the same thing the county owner bytes do, from a
/// different field: five realms in play, one county each. `+0x29` is the
/// realm's own count of what it owns.
#[test]
fn five_realms_are_in_play_and_each_owns_exactly_one_county() {
    let save = england!();
    let realms = save.realms().unwrap();
    assert_eq!(realms.len(), 6, "six records, index 0 unused");
    assert!(!realms[0].in_play(), "realm 0 is an array slot");

    for r in realms.iter().skip(1) {
        assert!(r.in_play(), "realm {} is in play", r.index);
        assert_eq!(r.county_count, 1, "realm {} owns one county", r.index);
        assert_eq!(r.gold, 1000, "realm {} starts on a thousand crowns", r.index);
        assert_eq!((r.iron, r.stone), (50, 50), "realm {}", r.index);
        assert_eq!(r.ai_step, 0, "realm {} has not begun a turn", r.index);
        assert_eq!(r.rank, 0, "nothing has been ranked yet");
        assert_eq!(r.tax_hap_empire, 0, "every rate is zero");
        assert_eq!(r.wages, 0);
    }

    let human: Vec<usize> = realms.iter().filter(|r| r.is_human).map(|r| r.index).collect();
    assert_eq!(human, vec![1], "one human realm");
    assert_eq!(realms[1].lord, 0, "row 0 of every lord-indexed table is the person's");

    let mut lords: Vec<u8> = realms.iter().skip(1).map(|r| r.lord).collect();
    lords.sort_unstable();
    assert_eq!(lords, [0, 1, 2, 3, 4], "five distinct lords, one apiece");
}

#[test]
fn the_england_map_has_fourteen_counties_and_county_one_is_the_dead_end() {
    let save = england!();
    let counties = save.counties().unwrap();
    let real: Vec<_> = counties.iter().filter(|c| c.is_county()).collect();
    assert_eq!(real.len(), 14);

    let directed: usize = real.iter().map(|c| c.neighbours().len()).sum();
    assert_eq!(directed % 2, 0);
    eprintln!("England: {} borders", directed / 2);

    assert_eq!(counties[1].neighbours(), &[2]);
    assert_eq!(counties[1].neighbour_count, 1);
    assert_eq!(directed / 2, 27, "twenty-seven borders on the England map");
}

#[test]
fn realm_fives_starting_county_is_the_one_that_begins_on_half_rations() {
    let save = england!();
    let counties = save.counties().unwrap();

    for c in counties.iter().filter(|c| c.is_county() && !c.is_owned()) {
        assert_eq!((c.grain, c.herd, c.ration_split), (100, 67, 100), "county {}", c.index);
        assert_eq!((c.grain_eaten, c.herd_eaten), (0, 13), "county {}", c.index);
        assert_eq!(c.ration_achieved, 3, "county {}", c.index);
        assert_eq!(c.d_hap_ration, 1, "county {}", c.index);
    }

    let hungry = england_county_of_realm(&save, 5);
    let short: Vec<usize> = counties
        .iter()
        .filter(|c| c.is_county() && c.ration_achieved != 3)
        .map(|c| c.index)
        .collect();
    assert_eq!(short, vec![hungry], "exactly one county starts short, and it is realm 5's");

    for c in counties.iter().filter(|c| c.is_owned() && c.index != hungry) {
        assert_eq!(c.grain, 0, "owned county {} holds no grain", c.index);
        assert!(c.herd * 5 >= c.population, "owned county {} lives on cheese", c.index);
        assert_eq!((c.grain_eaten, c.herd_eaten), (0, 0), "county {}", c.index);
        assert_eq!(c.ration_achieved, 3, "county {}", c.index);
        assert_eq!(c.d_hap_ration, 1, "county {}", c.index);
    }

    let one = counties[hungry];
    assert_eq!(one.owner, 5, "by construction");
    assert_eq!((one.grain, one.herd, one.ration_split), (0, 74, 0));
    assert_eq!(one.ration_wanted, 3, "it asked for Normal");
    assert_eq!(one.ration_achieved, 2, "and the preview says it will get Half");
    assert_eq!(one.d_hap_ration, -2, "3L - 8 at L = 2");
    assert_eq!(one.shown_ration, 1, "but the happiness it stores was built on +1");
    assert_eq!(one.happiness, 72, "65 + 5 + 1 + 1");
}

/// `+0x180` and `+0x184` equal the stores themselves, in every county. The pass
/// that wrote them therefore spent nothing — which is the ration *preview*, and
/// is the file's own evidence for the two-call reading of `Ration_Apply`.
#[test]
fn the_recorded_available_food_is_the_food_still_in_store() {
    let save = england!();
    for c in save.counties().unwrap().iter().filter(|c| c.is_county()) {
        assert_eq!(c.grain_available, c.grain, "county {}", c.index);
        assert_eq!(c.herd_available, c.herd, "county {}", c.index);
    }
}

