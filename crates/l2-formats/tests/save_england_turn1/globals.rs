#![allow(unused_imports)]
use super::*;
use super::counties::*;
use super::merchants::*;
use l2_formats::save::COUNTY_RECORDS;
use l2_testkit::{england, england_county_of_realm, ENGLAND_TURN1_COUNTIES};

/// The clock, the options and who is playing — the scalars outside the two
/// arrays.
///
/// **Corrected.** This used to assert `g_weatherCounty == 2` and that the
/// person holds county 8. Both are per-game rolls: the second England save
/// gives weather county 10 and puts the person on county 13. What survives is
/// that the weather county is a real county and that the person holds exactly
/// one of the five starting counties.
#[test]
fn the_globals_are_a_turn_one_winter_game_driven_by_realm_one() {
    let save = england!();
    let g = save.globals().unwrap();
    assert_eq!(g.county_count, 14);
    assert_eq!(g.scenario_index, 0, "the England map");
    assert_eq!(g.local_player, 1);
    assert_eq!((g.season, g.season_next, g.year, g.turn_count), (4, 1, 1268, 1));
    assert_eq!((g.turn_phase, g.turn_phase_step), (1, 0), "parked at the start of phase 1");
    assert_eq!((g.opt_difficulty, g.opt_advanced_farming, g.opt_armies_eat), (0, 0, 0));
    assert_eq!(g.merchant_count, 6);
    assert!(
        (1..=g.county_count).contains(&g.weather_county),
        "g_weatherCounty {} is a real county (which one is rolled per game)",
        g.weather_county
    );

    let mine = england_county_of_realm(&save, g.local_player as u8);
    assert!(
        ENGLAND_TURN1_COUNTIES.contains(&mine),
        "the person holds county {mine}, one of the five starting counties"
    );
    assert_eq!(save.counties().unwrap()[mine].owner as i32, g.local_player);
}

