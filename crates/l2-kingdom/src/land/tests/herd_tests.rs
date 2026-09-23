#![allow(unused_imports)]
use super::*;
use super::fertility_tests::*;
use super::grain_tests::*;
use super::crowding::*;
use super::super::*;

pub(crate) fn grazing(herd: i32, fields_cattle: i32, labour: i32) -> County {
    let mut c = County::new();
    c.herd = herd;
    c.fields_cattle = fields_cattle;
    c.labour[T.job.cattle_farming] = labour;
    c.herd_crowding = herd_crowding(T, herd, fields_cattle);
    c.weather = Weather::Cloudy;
    c
}

pub(crate) const SPRING: u8 = Season::Spring as u8;
pub(crate) const SUMMER: u8 = Season::Summer as u8;
pub(crate) const AUTUMN: u8 = Season::Autumn as u8;
pub(crate) const WINTER: u8 = Season::Winter as u8;

#[test]
fn the_herd_follows_the_weather_table_exactly() {
    for w in Weather::ALL {
        let mut c = grazing(1000, 100, 3000);
        c.weather = w;
        let farming = herd_growth(T, 1000, 100, 3000, c.herd_crowding, SUMMER);
        herd_season_tick(T, &mut c, SUMMER, AUTUMN);
        let weather = pct(1000, T.weather[w.index() as usize].herd_pct);
        assert_eq!(c.herd, 1000 + farming.net() + weather, "{}", w.name());
    }
    //... and the ordering the table encodes is still the ordering.
    let after = |w: Weather| {
        let mut c = grazing(1000, 100, 3000);
        c.weather = w;
        herd_season_tick(T, &mut c, SUMMER, AUTUMN);
        c.herd
    };
    assert!(after(Weather::Sunny) > after(Weather::Cloudy));
    assert!(after(Weather::Cloudy) > after(Weather::Frost));
}

#[test]
fn the_herd_tick_records_what_the_event_and_the_weather_did() {
    for w in Weather::ALL {
        let mut c = grazing(1000, 100, 3000);
        c.weather = w;
        herd_season_tick(T, &mut c, SUMMER, AUTUMN);
        assert_eq!(c.herd_weather_change, pct(1000, T.weather[w.index() as usize].herd_pct), "{}", w.name());
        assert_eq!(c.herd_event_change, 0, "{}: no event", w.name());
    }
    let mut c = grazing(200, 20, 600);
    c.event_herd_pct = -25; // wolves
    herd_season_tick(T, &mut c, SUMMER, AUTUMN);
    assert_eq!(c.herd_event_change, 50, "a magnitude, not -50");

    let mut c = grazing(1000, 100, 3000);
    c.weather = Weather::Sunny;
    c.event_herd_pct = crate::event::HERD_NO_GROWTH;
    herd_season_tick(T, &mut c, SUMMER, AUTUMN);
    assert_eq!(c.herd_weather_change, 0, "No Bull cancels the sun, and the figure says so");
    assert_eq!(c.herd_event_change, 0, "and has no figure of its own");
}

#[test]
fn wolves_take_their_percentage_and_the_modifier_is_consumed() {
    let mut c = grazing(200, 20, 600);
    c.event_herd_pct = -25; // "taken by wolves"
    let farming = herd_growth(T, 200, 20, 600, c.herd_crowding, SUMMER);
    herd_season_tick(T, &mut c, SUMMER, AUTUMN);
    assert_eq!(c.herd, 200 + farming.net() - 50, "a quarter of the herd, on top of farming");
    assert_eq!(c.event_herd_pct, 0);
}

#[test]
fn three_labourers_a_head_is_full_staffing_and_below_it_cattle_die() {
    let herd = 100;
    let fields = 10; // density 10, the mildest crowding band
    let crowding = herd_crowding(T, herd, fields);
    assert_eq!(crowding, 10);

    let full = herd * T.herd.labour_per_head;
    assert_eq!(full, 300, "three a head");

    let mut previous_deaths = i32::MAX;
    let mut previous_births = -1;
    for labour in 0..=(full * 3) {
        let g = herd_growth(T, herd, fields, labour, crowding, SUMMER);
        assert!(g.deaths <= previous_deaths, "labour {labour} killed more than {}", labour - 1);
        assert!(g.births >= previous_births, "labour {labour} bred less than {}", labour - 1);
        previous_deaths = g.deaths;
        previous_births = g.births;
    }

    let staffed = herd_growth(T, herd, fields, full, crowding, SUMMER);
    let abandoned = herd_growth(T, herd, fields, 0, crowding, SUMMER);
    assert_eq!(staffed.deaths, 1, "1 per 10,000 of 100 head");
    assert_eq!(abandoned.deaths, 34, "1 + (100 - 0) / 3 per 10,000, which is a third of it");
    assert_eq!(staffed.births, 14, "1,400 per 10,000");
    assert_eq!(abandoned.births, 0, "and a staffing of zero is a birth rate of zero");
    assert!(staffed.net() > 0 && abandoned.net() < 0, "the sign of the season flips");

    assert_eq!(herd_growth(T, herd, fields, full - 1, crowding, SUMMER).deaths, 1);
    assert_eq!(herd_growth(T, herd, fields, full / 3, crowding, SUMMER).deaths, 23);
}

#[test]
fn the_staffing_benefit_stops_at_twice_the_workers() {
    let (herd, fields) = (10_000, 1_000);
    let crowding = herd_crowding(T, herd, fields);
    let at = |labour: i32| herd_growth(T, herd, fields, labour, crowding, SUMMER).births;
    let full = herd * T.herd.labour_per_head;
    assert!(at(2 * full) > at(full), "twice the workers is worth having");
    assert_eq!(at(2 * full), at(10 * full), "ten times over is not");
    let ninety_nine = full * 199 / 100;
    assert!(at(ninety_nine) < at(2 * full));
}

#[test]
fn a_small_herd_breeds_faster_but_only_if_somebody_is_tending_it() {
    for &(below, bonus) in T.herd.small_bonus.iter() {
        let herd = below - 1;
        let fields = 10;
        let crowding = herd_crowding(T, herd, fields);
        let staffed = herd_growth(T, herd, fields, herd * 3, crowding, SUMMER);
        let base = pct(T.herd.crowding[0].birth_rate, 100);
        assert_eq!(
            staffed.births,
            per_myriad(herd, base + bonus).max(1),
            "a herd of {herd} should get the {bonus} bonus"
        );
        let idle = herd_growth(T, herd, fields, 0, crowding, SUMMER);
        assert!(idle.births <= 1, "and an unstaffed herd of {herd} gets none of it");
    }
    let crowding = herd_crowding(T, 25, 10);
    let plain = herd_growth(T, 25, 10, 75, crowding, SUMMER);
    assert_eq!(plain.births, per_myriad(25, 1400).max(1));
}

#[test]
fn a_county_with_no_pasture_loses_half_its_herd_or_all_of_it() {
    for herd in 0..=200 {
        for labour in [0, herd * 3, herd * 30] {
            let g = herd_growth(T, herd, 0, labour, 40, SUMMER);
            let expect = if herd == 0 {
                0
            } else if herd < T.herd.no_pasture_kill_all_below {
                herd
            } else {
                herd / 2
            };
            assert_eq!(g.deaths, expect, "herd {herd}, labour {labour}");
            assert_eq!(g.births, 0, "and nothing is born");
        }
    }
    assert_eq!(herd_growth(T, 5, 0, 999, 10, SPRING).deaths, 5, "five head, all of them");
    assert_eq!(herd_growth(T, 6, 0, 999, 10, SPRING).deaths, 3, "six head, half of them");
}

