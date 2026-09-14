#![allow(unused_imports)]
use super::*;
use super::simulation::*;
use super::*;
use super::helpers::*;
use super::food_and_ration::*;
use super::population_and_labour::*;
use super::simulation::*;
use l2_formats::save::Save;
use l2_kingdom::county::County;
use l2_kingdom::phase::SEASON_PIPELINE;
use l2_kingdom::tables::{health_band, Season, Tables, Weather};
use l2_scenario::{Scenario, STARTING_HEALTH_METER};

/// **The correction, asserted through the importer.** Five owned counties, at
/// indices 1, 4, 8, 11 and 13, one for each of realms 1 to 5 — not four owned
/// by one realm, which is what C12's version of this file invented.
///
/// **Corrected again.** It used to pin the mapping,
/// `[(1, 5), (4, 4), (8, 1), (11, 3), (13, 2)]`. The *set* is scenario; the
/// *assignment* is rolled per game, and a second England turn-one save gives
/// 1→4, 4→2, 8→5, 11→3, 13→1. The importer's job is to carry across whatever
/// the file says, so what is asserted is that it did: every owner it produced
/// is the owner byte the save holds.
#[test]
fn the_england_scenario_is_five_realms_with_one_county_each() {
    let s = england!();
    assert_eq!(s.county_count, 14, "fourteen counties on the England map");
    assert_eq!(s.local_player, 1);

    let owned: Vec<(usize, u8)> = s
        .county_ids()
        .filter_map(|id| s.counties[id].as_ref().map(|c| (id, c.owner)))
        .filter(|(_, owner)| *owner != 0)
        .collect();
    assert_eq!(
        owned.iter().map(|&(id, _)| id).collect::<Vec<usize>>(),
        l2_testkit::ENGLAND_TURN1_COUNTIES,
        "the five starting counties"
    );
    let mut realms: Vec<u8> = owned.iter().map(|&(_, r)| r).collect();
    realms.sort_unstable();
    assert_eq!(realms, [1, 2, 3, 4, 5], "one county each, in some order");
    assert_eq!(s.county_ids().count() - owned.len(), 9, "nine unowned");

    for id in 1..=5 {
        assert!(s.realms[id].in_play, "realm {id} is in play");
        assert_eq!(s.realms[id].county_count, 1, "realm {id} owns one county");
        assert_eq!(s.realms[id].gold, 1000);
    }
    assert!(s.realms[1].is_human, "realm 1 is the person");
    assert_eq!(s.realms[1].lord, 0, "row 0 of every lord-indexed table is the person's");
    for id in 2..=5 {
        assert!(!s.realms[id].is_human, "realm {id} is an AI");
    }
}

/// `g_season` 4, `g_year` 1268, `g_turnCount` 1 — read out of the file rather
/// than predicted from `Game_NewGame`'s constants. The prediction and the file
/// agree, which is the point: [`the_pipeline_reaches_the_files_clock`] runs the
/// clock forward and lands on these.
#[test]
fn the_file_is_a_turn_one_winter_1268_autosave() {
    let s = england!();
    assert_eq!(s.clock.season, 4);
    assert_eq!(Season::from_index(s.clock.season), Some(Season::Winter));
    assert_eq!(s.clock.season_next, 1, "Spring is next");
    assert_eq!(s.clock.year, 1268);
    assert_eq!(s.clock.turn_count, 1);
    assert_eq!(s.options.difficulty, 0);
    assert!(!s.options.advanced_farming);
    assert!(!s.options.armies_eat);
}

/// The seventeen-record array with an unused index 0, from the file: records 15
/// and 16 import as nothing at all, and 1 … 14 all import.
#[test]
fn the_array_holds_seventeen_records_and_only_fourteen_are_a_county() {
    let s = england!();
    let k = s.kingdom(SEED);
    assert_eq!(k.counties.len(), 17);
    assert_eq!(k.county_count, 14);

    let empty = County::new();
    assert_eq!(k.counties[0], empty, "record 0 is never a county");
    assert_eq!(k.counties[15], empty);
    assert_eq!(k.counties[16], empty);
    assert!(s.counties[15].is_none());
    assert!(s.counties[16].is_none());
    for id in s.county_ids() {
        assert!(s.counties[id].is_some(), "county {id} should have imported");
        assert_ne!(k.counties[id], empty, "county {id} should be populated");
    }
}

/// Adjacency comes from the file:
/// end with a single neighbour, county 10 is a hub with seven, and every border
/// is named from both sides.
#[test]
fn the_map_the_import_builds_is_the_map_in_the_file() {
    let s = england!();
    let k = s.kingdom(SEED);
    for id in s.county_ids() {
        let c = &k.counties[id];
        assert!(c.neighbour_count > 0, "county {id} borders somebody");
        for &n in c.neighbours() {
            assert!(k.counties[n as usize].neighbours().contains(&(id as u8)), "county {id}");
        }
    }
    assert_eq!(k.counties[MAP_DEAD_END].neighbours(), &[2], "county 1 is the dead end");
    assert_eq!(k.counties[10].neighbour_count, 7, "county 10 is the hub");
    assert_eq!(
        s.county_ids().map(|id| k.counties[id].neighbour_count as usize).sum::<usize>(),
        54,
        "twenty-seven borders, counted from both sides"
    );
}

// ---------------------------------------------------------------------------
// The starting position
// ---------------------------------------------------------------------------

/// The rewind is the file's own record of the previous season, not a guess:
/// `population` becomes `popLast` and `happiness` becomes `happinessLast`, and
/// every value the season *computes* starts at zero, so that a pass which does
/// nothing cannot pass by leaving them alone.
#[test]
fn the_starting_position_is_the_file_rewound_by_exactly_one_season() {
    let s = england!();
    let start = s.starting_kingdom(SEED);
    let file = s.kingdom(SEED);

    for id in s.county_ids() {
        let a = &start.counties[id];
        let b = &file.counties[id];
        assert_eq!(a.population, b.pop_last, "county {id}");
        assert_eq!(a.happiness, b.happiness_last, "county {id}");
        assert_eq!(a.health_meter, STARTING_HEALTH_METER, "county {id}");
        assert_eq!(a.health_band, health_band(STARTING_HEALTH_METER) as u8, "county {id}");
        assert_eq!(a.owner, b.owner, "county {id}");
        assert_eq!(a.castle_type, b.castle_type, "county {id}");

        assert_eq!(a.births, 0, "county {id} has not been born into yet");
        assert_eq!(a.deaths, 0, "county {id}");
        assert_eq!(a.pop_last, 0, "county {id}");
        assert_eq!(a.pop_band, 0, "county {id}");
        assert_eq!(a.shown_tax, 0, "county {id}");
        assert_eq!(a.shown_ration, 0, "county {id}");
        assert_eq!(a.shown_health, 0, "county {id}");
        assert_eq!(a.happiness_sum, 0, "county {id}");
    }

    // Every county in the England turn-one fixture started the season on the same two
    // numbers - the file's own claim, checked.
    let starts: Vec<(i32, i32)> = s
        .county_ids()
        .map(|id| (file.counties[id].pop_last, file.counties[id].happiness_last))
        .collect();
    assert!(starts.iter().all(|&v| v == (417, 65)), "one starting position, fourteen counties");

    assert_eq!(start.turn_count, 0, "no season has run");
    assert_eq!(start.season, 0);
}

