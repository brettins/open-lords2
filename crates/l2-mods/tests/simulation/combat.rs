#![allow(unused_imports)]
use super::*;
use super::economy::*;
use common::TempDir;
use l2_mods::Platform;
use l2_sim::{Battle, Troop, TroopTable, SIDE_A, SIDE_B};
use l2_kingdom::tables::{health_band, Tables};
use l2_kingdom::{Kingdom, Options};
use l2_kingdom::tables::{Commodity, JOB_COUNT};

pub(crate) fn empty_base(dir: &TempDir) {
    dir.write("placeholder.txt", "not a rule and not an asset the engine asks for");
}

fn duel(table: TroopTable, a: Troop, b: Troop, men: u16, ticks: u32) -> (u32, u32) {
    let mut battle = Battle::with_troops(table);
    battle.add(a, SIDE_A, men).unwrap();
    battle.add(b, SIDE_B, men).unwrap();
    battle.engage(0, 1);
    battle.run(ticks);
    (battle.men(SIDE_A), battle.men(SIDE_B))
}

#[test]
fn a_load_with_no_mods_reproduces_the_engines_own_combat_constants() {
    let base = TempDir::new("sim-plain");
    empty_base(&base);
    let p = Platform::builder().base(base.path()).build().unwrap();
    assert_eq!(p.troop_table().unwrap(), TroopTable::DEFAULT);
}

#[test]
fn a_mod_file_changes_the_outcome_of_a_battle() {
    let base = TempDir::new("sim-base");
    let mods = TempDir::new("sim-mods");
    empty_base(&base);
    mods.write("angry-peasants/mod.toml", "[mod]\nid = \"angry-peasants\"\n");
    mods.write(
        "angry-peasants/rules/peasants.toml",
        "[unit.peasants]\nmelee_attack = [40, 40, 40, 40]\n",
    );

    let plain = Platform::builder().base(base.path()).build().unwrap();
    let modded = Platform::builder()
        .base(base.path())
        .mods_dir(mods.path())
        .enable(["angry-peasants"])
        .build()
        .unwrap();

    let report = modded.report();
    assert_eq!(report.overrides.len(), 1, "{report}");
    assert_eq!(report.overrides[0].path, "unit.peasants.melee_attack");
    assert!(report.added_rules.is_empty(), "{report}");

    let before = duel(plain.troop_table().unwrap(), Troop::Peasants, Troop::Swordsmen, 20, 2000);
    let after = duel(modded.troop_table().unwrap(), Troop::Peasants, Troop::Swordsmen, 20, 2000);

    assert!(
        after.0 > before.0,
        "peasants swinging for 40 should fare better than the stock 5: {after:?} vs {before:?}"
    );
    assert!(after.1 < before.1, "and the swordsmen should suffer for it");
}

#[test]
fn the_last_mod_in_the_load_order_is_the_one_the_simulation_runs_on() {
    let base = TempDir::new("sim-order-base");
    let mods = TempDir::new("sim-order-mods");
    empty_base(&base);
    for (id, armour) in [("aaa", 10), ("zzz", 55)] {
        mods.write(&format!("{id}/mod.toml"), &format!("[mod]\nid = \"{id}\"\n"));
        mods.write(
            &format!("{id}/rules/{id}.toml"),
            &format!("[unit.archers]\narmour = {armour}\n"),
        );
    }

    let forward = Platform::builder()
        .base(base.path())
        .mods_dir(mods.path())
        .enable(["aaa", "zzz"])
        .build()
        .unwrap();
    let backward = Platform::builder()
        .base(base.path())
        .mods_dir(mods.path())
        .enable(["zzz", "aaa"])
        .build()
        .unwrap();

    let armour_of = |p: &Platform| p.troop_table().unwrap().stats(Troop::Archers).armour;
    assert_eq!(armour_of(&forward), 55);
    assert_eq!(armour_of(&backward), 10);

    let mut battle = Battle::with_troops(forward.troop_table().unwrap());
    let i = battle.add(Troop::Archers, SIDE_A, 4).unwrap();
    assert_eq!(battle.figures[i].armour(), 55);
}

#[test]
fn an_impossible_combat_constant_is_refused_with_the_line_that_wrote_it() {
    let base = TempDir::new("sim-bad-base");
    let mods = TempDir::new("sim-bad-mods");
    empty_base(&base);
    mods.write("bad/mod.toml", "[mod]\nid = \"bad\"\n");
    mods.write("bad/rules/bad.toml", "[unit.archers]\nrecovery = 0\n");

    let p = Platform::builder()
        .base(base.path())
        .mods_dir(mods.path())
        .enable(["bad"])
        .build()
        .expect("the document is well formed, so the load itself succeeds");

    let err = p.troop_table().unwrap_err().to_string();
    assert!(err.contains("bad:rules/bad.toml:2:12"), "{err}");
    assert!(err.contains("unit.archers.recovery"), "{err}");
    assert!(err.contains("0 is outside 1..=65535"), "{err}");
}

#[test]
fn a_rising_strength_band_is_refused_rather_than_sorted() {
    let base = TempDir::new("sim-band-base");
    let mods = TempDir::new("sim-band-mods");
    empty_base(&base);
    mods.write("rising/mod.toml", "[mod]\nid = \"rising\"\n");
    mods.write("rising/rules/r.toml", "[unit.knights]\nmelee_attack = [5, 9, 3, 1]\n");

    let p = Platform::builder()
        .base(base.path())
        .mods_dir(mods.path())
        .enable(["rising"])
        .build()
        .unwrap();

    let err = p.troop_table().unwrap_err().to_string();
    assert!(err.contains("the bands run best first"), "{err}");
    assert!(err.contains("rising:rules/r.toml"), "{err}");
}

#[test]
fn a_kingdom_mod_overrides_exactly_the_two_rules_it_names() {
    let base = TempDir::new("sim-king-base");
    let mods = TempDir::new("sim-king-mods");
    empty_base(&base);
    mods.write("famine/mod.toml", "[mod]\nid = \"famine\"\n");
    mods.write(
        "famine/rules/famine.toml",
        "[kingdom.grain]\nyield_per_sack = 6\n\n[kingdom.food]\nfood_per_sack = 3\n",
    );

    let plain = Platform::builder().base(base.path()).build().unwrap();
    assert_eq!(plain.kingdom_tables().unwrap(), l2_kingdom::tables::Tables::DEFAULT);

    let modded = Platform::builder()
        .base(base.path())
        .mods_dir(mods.path())
        .enable(["famine"])
        .build()
        .unwrap();
    let t = modded.kingdom_tables().unwrap();
    assert_eq!(t.grain.yield_per_sack, 6);
    assert_eq!(t.food.food_per_sack, 3);
    let stock = l2_kingdom::tables::Tables::DEFAULT;
    assert_eq!(t.grain.max_sacks_per_field, stock.grain.max_sacks_per_field);
    assert_eq!(t.food.food_per_head, stock.food.food_per_head);
    assert_eq!(modded.report().overrides.len(), 2, "{}", modded.report());
}

#[test]
fn the_documented_example_mod_reaches_the_simulation() {
    let base = TempDir::new("sim-example-base");
    empty_base(&base);
    let p = Platform::builder()
        .base(base.path())
        .mods_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("example-mods"))
        .enable(["longbows"])
        .build()
        .expect("the documented example loads");

    let archers = p.troop_table().unwrap().stats(Troop::Archers);
    assert_eq!(archers.melee_attack, [7, 5, 4, 2]);
    assert_eq!(archers.armour, 4);
    assert!(!p.report().added_rules.is_empty());
}


