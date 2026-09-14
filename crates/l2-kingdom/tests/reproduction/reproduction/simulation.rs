#![allow(unused_imports)]
use super::*;
use super::scenarios::*;
use super::*;
use super::helpers::*;
use super::food_and_ration::*;
use super::population_and_labour::*;
use l2_formats::save::Save;
use l2_kingdom::county::County;
use l2_kingdom::phase::SEASON_PIPELINE;
use l2_kingdom::tables::{health_band, Season, Tables, Weather};
use l2_scenario::{Scenario, STARTING_HEALTH_METER};

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


