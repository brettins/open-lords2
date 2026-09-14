#![allow(unused_imports)]
use super::*;
use super::rules::*;
use super::ai::*;
use super::validation::*;
use super::*;
use super::combat::*;
use common::TempDir;
use l2_mods::Platform;
use l2_sim::{Battle, Troop, TroopTable, SIDE_A, SIDE_B};
use l2_kingdom::tables::{health_band, Tables};
use l2_kingdom::{Kingdom, Options};
use l2_kingdom::tables::{Commodity, JOB_COUNT};
use l2_kingdom::tables::{health_band, Tables};
use l2_kingdom::{Kingdom, Options};
use l2_kingdom::tables::{Commodity, JOB_COUNT};

/// A one-county kingdom with five grain fields, a hundred sacks of seed and
/// more labour than sowing can use, run through a full year: Spring sows,
/// Summer and Autumn grow, Winter harvests.
///
/// Rations come entirely from the herd (`ration_split = 100`)
/// large, so nothing eats the grain and the store at the end of the year is the
/// harvest.
fn a_years_harvest(tables: Tables) -> i32 {
    let mut k = Kingdom::with_tables(0xF00D_1234, tables);
    k.options = Options { difficulty: 0, advanced_farming: false, ..Options::default() };
    assert!(k.set_county_count(1));
    k.realms[1].in_play = true;
    k.realms[1].is_human = true;
    k.realms[1].county_count = 1;

    let c = &mut k.counties[1];
    c.owner = 1;
    c.population = 400;
    c.pop_band = c.compute_pop_band();
    c.happiness = 65;
    c.health_meter = 65;
    c.health_band = health_band(65);
    c.herd = 400;
    c.grain = 100;
    c.ration_wanted = 3; // Normal
    c.ration_split = 100; // fed entirely from the herd
    c.labour = [100_000; l2_kingdom::tables::JOB_COUNT];

    // **Five grain fields, on the map.** Writing `fields_grain = 5` into the
    // record is not enough any more and never should have been:
    // `County_RecountFieldsAll` is a pass of the season now, and it rebuilds
    // all five counts from the terrain byte of the twenty tiles the county
    // names. A county with no field tiles has no fields, whatever its record
    // says. `docs/kingdom.md` §7.2.
    for slot in 0..5usize {
        let tile = slot + 1;
        k.counties[1].field_tiles[slot] = tile as u16;
        k.campaign.map.terrain[tile] = l2_kingdom::field::terrain::GRAIN;
    }

    k.start_new_game(); // Winter 1268
    k.advance_season(); // Spring: sow
    // Whatever is left in the barn after sowing is seed the county did not
    // plant, and it is still there in Winter. Subtracting it leaves the
    // harvest itself, which is the number the yield rule decides.
    let unsown = k.counties[1].grain;
    for _ in 0..3 {
        k.advance_season(); // Summer, Autumn, Winter
    }
    k.counties[1].grain - unsown
}

/// With no mods at all the kingdom runs on the engine's own numbers. The half
/// that is easy to forget: a `Kingdom` that quietly ignored its table would
/// pass every "the mod changed it" test ever written.
#[test]
fn a_load_with_no_mods_reproduces_the_engines_own_economy() {
    let base = TempDir::new("king-plain");
    empty_base(&base);
    let p = Platform::builder().base(base.path()).build().unwrap();
    let loaded = p.kingdom_tables().unwrap();
    assert_eq!(loaded, Tables::DEFAULT);
    assert_eq!(
        a_years_harvest(loaded),
        a_years_harvest(Tables::DEFAULT),
        "the loaded table and the engine's own must farm identically"
    );
    assert_eq!(Kingdom::new(1).tables, Tables::DEFAULT);
}

/// **Two lines of TOML**
///
/// This is the assertion `docs/modding.md` §11 used to say could not be made.
/// It starts at a text file in a mod directory and ends at a different number
/// of sacks in a barn, through the real `Season_Advance` pipeline.
#[test]
fn a_mod_file_changes_what_a_county_harvests() {
    let base = TempDir::new("king-base");
    let mods = TempDir::new("king-mods");
    empty_base(&base);
    mods.write("famine/mod.toml", "[mod]\nid = \"famine\"\n");
    mods.write(
        "famine/rules/famine.toml",
        "[kingdom.grain]\nyield_per_sack = 6\n",
    );

    let plain = Platform::builder().base(base.path()).build().unwrap();
    let modded = Platform::builder()
        .base(base.path())
        .mods_dir(mods.path())
        .enable(["famine"])
        .build()
        .unwrap();

// It overrode a rule the engine already had.
    let report = modded.report();
    assert_eq!(report.overrides.len(), 1, "{report}");
    assert_eq!(report.overrides[0].path, "kingdom.grain.yield_per_sack");
    assert!(report.added_rules.is_empty(), "{report}");

    let before = a_years_harvest(plain.kingdom_tables().unwrap());
    let after = a_years_harvest(modded.kingdom_tables().unwrap());

    assert!(before > 0, "the stock year should actually grow something: {before}");
    assert!(
        after < before,
        "halving the yield per sack should halve the harvest: {after} vs {before}"
    );
// Exactly half, and that is worth pinning.
    // inequality: the yield multiplies the crop once at sowing and nothing
    // downstream of it is a threshold at these quantities.
    assert_eq!(after * 2, before, "{after} should be half of {before}");
}

/// Load order decides the economy too, right through to the barn — the same
/// claim `the_last_mod_in_the_load_order_is_the_one_the_simulation_runs_on`
/// makes about a casualty count.
#[test]
fn the_last_kingdom_mod_in_the_load_order_is_the_one_the_economy_runs_on() {
    let base = TempDir::new("king-order-base");
    let mods = TempDir::new("king-order-mods");
    empty_base(&base);
    for (id, yield_per_sack) in [("aaa", 6), ("zzz", 24)] {
        mods.write(&format!("{id}/mod.toml"), &format!("[mod]\nid = \"{id}\"\n"));
        mods.write(
            &format!("{id}/rules/{id}.toml"),
            &format!("[kingdom.grain]\nyield_per_sack = {yield_per_sack}\n"),
        );
    }

    let build = |order: [&str; 2]| {
        Platform::builder()
            .base(base.path())
            .mods_dir(mods.path())
            .enable(order)
            .build()
            .unwrap()
            .kingdom_tables()
            .unwrap()
    };
    let forward = build(["aaa", "zzz"]);
    let backward = build(["zzz", "aaa"]);
    assert_eq!(forward.grain.yield_per_sack, 24);
    assert_eq!(backward.grain.yield_per_sack, 6);
    let high = a_years_harvest(forward);
    let low = a_years_harvest(backward);
    assert!(high > low, "the last mod loaded is the one that farms: {high} against {low}");

    // **And it is not four times, which is the labour cap talking.** The yield
    // per sack is in `Grain_Sow`'s own labour test — `yield * seed / divisor`
    // hands are needed to tend the seed —
    // four times as hungry for farmhands
    // less of it. `Grain_Grow` and `Grain_Harvest` then cap the standing crop
    // at `labour * multiplier` twice more. Quadrupling the number in the file
    // doubles what reaches the barn.
    // Since C225 the weather pass ruins one field a season in this seed
    // (`FUN_00469A9C`), so the barn holds 576 and 240, not 600 and 300.
    assert_eq!((high, low), (576, 240));
}

/// A rule with no arithmetic in it at all: the year random events start.
/// Pushing it forward means a modded game draws no events where the stock one
/// does, which is a behavioural difference.
#[test]
fn a_mod_can_postpone_the_first_year_random_events_are_drawn() {
    let base = TempDir::new("king-event-base");
    let mods = TempDir::new("king-event-mods");
    empty_base(&base);
    mods.write("quiet/mod.toml", "[mod]\nid = \"quiet\"\n");
    mods.write("quiet/rules/quiet.toml", "[kingdom.event]\nfirst_year = 1400\n");

    let modded = Platform::builder()
        .base(base.path())
        .mods_dir(mods.path())
        .enable(["quiet"])
        .build()
        .unwrap()
        .kingdom_tables()
        .unwrap();
    assert_eq!(modded.event.first_year, 1400);

    let mut county = l2_kingdom::county::County::new();
    county.owner = 1;
    assert!(
        l2_kingdom::event::eligible(&Tables::DEFAULT, &county, true, 1300),
        "the stock game is drawing events by 1300"
    );
    assert!(
        !l2_kingdom::event::eligible(&modded, &county, true, 1300),
        "and the modded one is not"
    );
}

// ---------------------------------------------------------------------------
// The five rules that had no field until now
// ---------------------------------------------------------------------------
//
// `docs/modding.md` §11 used to end with a list of rules a document could not
// reach at all: the ale ladder, the army-raising cost, the efficiency ramp's
// ceiling
// now, and each test below is the same shape as
// `a_mod_file_changes_what_a_county_harvests` — a `.toml` in a mod directory
// in, a different number out of the real simulation.
//
// The one that is *not* here is `kingdom.ai.personality.*.farm_style`. It
// loads and validates, and nothing in `l2-kingdom` reads it, because the three
// labour allocators `AI_ManageFields` dispatches into were never traced. There
// is no observable effect to assert,
// `docs/decisions.md` C12.

