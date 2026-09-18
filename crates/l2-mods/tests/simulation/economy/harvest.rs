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

    for slot in 0..5usize {
        let tile = slot + 1;
        k.counties[1].field_tiles[slot] = tile as u16;
        k.campaign.map.terrain[tile] = l2_kingdom::field::terrain::GRAIN;
    }

    k.start_new_game(); // Winter 1268
    k.advance_season(); // Spring: sow
    let unsown = k.counties[1].grain;
    for _ in 0..3 {
        k.advance_season(); // Summer, Autumn, Winter
    }
    k.counties[1].grain - unsown
}

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
    assert_eq!(after * 2, before, "{after} should be half of {before}");
}

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

    // Since C225 the weather pass ruins one field a season in this seed
    // (`FUN_00469A9C`), so the barn holds 576 and 240, not 600 and 300.
    assert_eq!((high, low), (576, 240));
}

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

// The one that is *not* here is `kingdom.ai.personality.*.farm_style`. It
// loads and validates, and nothing in `l2-kingdom` reads it, because the three
// labour allocators `AI_ManageFields` dispatches into were never traced. There
// is no observable effect to assert,
// `docs/decisions.md` C12.

