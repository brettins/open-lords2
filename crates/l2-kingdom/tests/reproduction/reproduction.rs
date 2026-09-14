#![allow(unused_imports)]
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

/// **`docs/kingdom.md` §9 point 3**, checked against the file
/// against §7.2: with Advanced Farming off, every stored weather byte is 3 and
/// every stored fertility is 0 — and the pipeline leaves them there.
#[test]
fn basic_farming_leaves_every_county_cloudy_with_zero_fertility() {
    let s = england!();
    let file = s.kingdom(SEED);
    let mut k = s.starting_kingdom(SEED);
    k.start_new_game();
    for id in s.county_ids() {
        assert_eq!(file.counties[id].weather, Weather::Cloudy, "the file, county {id}");
        assert_eq!(file.counties[id].fertility, 0, "the file, county {id}");
        assert_eq!(k.counties[id].weather.index(), 3, "county {id}: the stored byte is 3");
        assert_eq!(k.counties[id].fertility, 0, "county {id}");
    }
}

// ---------------------------------------------------------------------------
// The reproduction
// ---------------------------------------------------------------------------

/// One `Season_Advance` from the file's own starting position lands on the
/// file's own clock, having run every documented pass in order.
#[test]
fn the_pipeline_reaches_the_files_clock() {
    let s = england!();
    let mut k = s.starting_kingdom(SEED);
    let report = k.start_new_game();
    assert_eq!(k.season, s.clock.season);
    assert_eq!(k.season_next, s.clock.season_next);
    assert_eq!(k.year, s.clock.year);
    assert_eq!(k.turn_count, s.clock.turn_count);
    // **Every pass but phase 7's three**, none of which `Game_NewGame`
    // (`0x00497CED`) calls — `Mercenary_AdvanceAll`, `Units_ResetMoves`
    // (`0x004651B9`) and `Diplo_ReconcileAlliances` (`0x004A1847`) are
    // `Turn_Tick`'s phase-7 arm. The file agrees: its twelve bands are still at
    // `Mercenary_Init`'s state, and the clock and stored fields below land
    // without the other two.
    use l2_kingdom::phase::Pass;
    let expected: Vec<_> = SEASON_PIPELINE
        .iter()
        .copied()
        .filter(|p| {
            !matches!(p, Pass::MercenaryAdvance | Pass::UnitsResetMoves | Pass::ReconcileAlliances)
        })
        .collect();
    assert_eq!(report.passes, expected, "in the documented order");
    // The weather letters are the exception: `Weather_UpdateAll` posts `0x8F`
    // or `0x90` for a county that bands Drought or Flooding, and a new game
    // runs that pass like any other season.
    let unexpected: Vec<_> = report
        .messages
        .iter()
        .filter(|m| {
            !matches!(
                m,
                l2_kingdom::report::Message::Drought { .. }
                    | l2_kingdom::report::Message::Flooding { .. }
            )
        })
        .collect();
    assert!(unexpected.is_empty(), "a happy kingdom raises nothing but weather: {unexpected:?}");
    assert!(report.revolts.is_empty());
}

/// **Thirteen of the fourteen counties reproduce every stored field** from the
/// starting position the save itself supplies — no reconstruction, no
/// adjustment. The fourteenth is county 1.
///
/// The herd is skipped and only the herd: the save records no earlier balance
/// to spend from. Everything the herd *feeds* still lands, which is the part
/// that matters.
#[test]
fn every_county_the_save_can_feed_reproduces_every_stored_field() {
    let s = england!();
    let file = s.kingdom(SEED);
    let mut k = s.starting_kingdom(SEED);
    k.start_new_game();

    let hungry = hungry_county(&s);
    let mut checked = 0;
    for id in s.county_ids() {
        if id == hungry {
            continue;
        }
        for (name, ours, theirs) in comparison(&k.counties[id], &file.counties[id]) {
            if name == "herd" || name == "herd_eaten" {
                continue;
            }
            assert_eq!(ours, theirs, "county {id} {name}");
            checked += 1;
        }
    }
    assert_eq!(checked, 13 * 24, "thirteen counties, twenty-four fields each");
}

/// **Start every county from the stores the season found, and the whole map
/// reproduces — fourteen counties, twenty-five fields, no inversion.**
///
/// `+0x228` and `+0x254` are written by `Game_SetupRealmsAndCounties`
/// (`0x0049BD99`) with the new-game starting grain and herd, and again by
/// `Grain_SeasonTick` and `Herd_SeasonTick` as their first statement, *before*
/// the season's food is taken out. In a turn-one save that is the same number
/// twice, and it is the store the ration pass saw. The twenty-four fields of
/// [`comparison`] plus the ration split all land, for every county — including
/// realm 5's, which [`every_county_the_save_can_feed_reproduces_every_stored_field`]
/// has to skip and [`solve_opening`] had to invent eight sacks for.
///
/// **What this does not prove**: it passes with
/// `Pass::AiManageFarms` skipped as well. On this fixture the AI's season-head
/// pass leaves every one of these fields where the rest of the season puts
/// them, once the stores are right — so this test is evidence about the
/// *rewind*, and none at all about the pass. The pass is pinned by
/// [`realm_fives_county_is_fed_on_slaughter_by_its_lords_sweep_from_the_rewound_stores`].
///
/// *Ablation*: drop the `+0x254` read, so every county starts on the herd the
/// season ended with, and realm 5's county — only that one — goes red on three
/// fields: the split (100 against 0), the achieved level (3 against 2) and the
/// closing preview's ration term (+1 against −2). Measured with a scratch
/// probe, not reasoned.
#[test]
fn every_county_reproduces_from_the_stores_the_season_found() {
    let save = l2_testkit::england!();
    let s = Scenario::from_save(&save).expect("the England turn-one fixture must import");
    let file = s.kingdom(SEED);
    let mut k = s.starting_kingdom(SEED);
    for id in s.county_ids() {
        k.counties[id].grain = county_i32(&save, id, 0x228);
        k.counties[id].herd = county_i32(&save, id, 0x254);
    }
    k.start_new_game();

    let mut checked = 0;
    for id in s.county_ids() {
        for (name, ours, theirs) in comparison(&k.counties[id], &file.counties[id]) {
            assert_eq!(ours, theirs, "county {id} {name}");
            checked += 1;
        }
        assert_eq!(k.counties[id].ration_split, file.counties[id].ration_split, "county {id} split");
        checked += 1;
    }
    assert_eq!(checked, 14 * 25);
}

/// **The whole map, every stored field.** Put back the food the season ate — the
/// openings the previous test proves unique — and run `Season_Advance` once.
/// Fourteen counties, twenty-six fields each: three hundred and sixty-four
/// numbers, every one of them read out of `lastturn.sav`.
///
/// This is what the file at the top of this module claimed to be doing and was
/// not.
#[test]
fn every_county_reproduces_every_stored_field() {
    let s = england!();
    let file = s.kingdom(SEED);
    let k = kingdom_after_the_first_season(&s);

    let mut checked = 0;
    for id in s.county_ids() {
        for (name, ours, theirs) in comparison(&k.counties[id], &file.counties[id]) {
            assert_eq!(ours, theirs, "county {id} {name}");
            checked += 1;
        }
    }
    assert_eq!(checked, 14 * 24);
}

// ---------------------------------------------------------------------------
// The herd — `docs/kingdom.md` §13 and §13.1
// ---------------------------------------------------------------------------

/// **The staffing and crowding rules, against the file, with no inversion.**
///
/// `Herd_SeasonTick` ends by writing next season's forecast into `+0x268`,
/// `+0x26C` and `+0x258` — `L2.eng` group 77's *"Calf births expected"*, *"Cow
/// deaths expected"* and *"Change due to farming"* — from state the save also
/// holds: the herd, what the people ate, the pasture, the cattle labour and the
/// crowding. So the whole of `FUN_0044DA99` can be run against fourteen
/// counties' worth of stored answers without recovering anything.
///
/// Fifty-six numbers, and they are not a soft test: they exercise the
/// understaffed arm (county 1 at 98% staffing), the capped arm (county 2 at
/// 199%), three of the four crowding bands, the Spring birth bonus, and the
/// double subtraction of `herdEaten` that makes "change due to farming" what it
/// is. Get the `/ 3` truncation, the `199 <` comparison or the `x 3 / 2`
/// rounding wrong anywhere and a column moves.
#[test]
fn the_herds_own_forecast_reproduces_for_every_county() {
    let save = l2_testkit::england!();
    let s = Scenario::from_save(&save).expect("the England turn-one fixture must import");
    let mut k = s.kingdom(SEED);
    let next = s.clock.season_next;
    assert_eq!(next, 1, "the save's g_seasonNext is Spring, which is what calves");

    let mut checked = 0;
    for id in s.county_ids() {
        // The crowding the importer derived, against the byte the game stored.
        assert_eq!(
            k.counties[id].herd_crowding,
            county_i32(&save, id, 0x25C),
            "county {id} crowding"
        );
        checked += 1;

        l2_kingdom::land::herd_preview(&Tables::DEFAULT, &mut k.counties[id], next);
        for (name, ours, offset) in [
            ("births expected", k.counties[id].herd_births_expected, 0x268u32),
            ("deaths expected", k.counties[id].herd_deaths_expected, 0x26C),
            ("change due to farming", k.counties[id].herd_change_expected, 0x258),
        ] {
            assert_eq!(ours, county_i32(&save, id, offset), "county {id} {name}");
            checked += 1;
        }
    }
    assert_eq!(checked, 14 * 4);

    // And the two bands the map actually visits are not the same band, so the
    // check is not fourteen copies of one arithmetic.
    let crowdings: std::collections::BTreeSet<i32> =
        s.county_ids().map(|id| k.counties[id].herd_crowding).collect();
    assert_eq!(crowdings.into_iter().collect::<Vec<_>>(), vec![10, 20, 40]);
}

