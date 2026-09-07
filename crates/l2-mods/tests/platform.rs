//! End to end: an install, two mods, one loaded game.

mod common;

use common::TempDir;
use l2_mods::{Platform, Side, TroopRules};

/// A miniature "install": three sprites and a base ruleset.
fn fake_install(dir: &TempDir) {
    dir.write("Base1a.pl8", "base sprite a");
    dir.write("Base1b.pl8", "base sprite b");
    dir.write("AXMEN.SMK", "video");
    dir.write(
        "rules/troops.toml",
        r#"
[troop.peasants]
column = 0
name = "Peasants"
siege_engine = false

[troop.crossbows]
column = 1
name = "Crossbowmen"
siege_engine = false

[troop.catapults]
column = 7
name = "Catapults"
siege_engine = true

[difficulty.normal]
order = 2
scale_percent = 100

[difficulty.very_hard]
order = 4
scale_percent = 84

[battle.three_bridges]
index = 0
name = "Three Bridges"
defensive_advantage = 5

[battle.three_bridges.attacker]
peasants = 100
crossbows = 20
catapults = 2

[battle.three_bridges.defender]
peasants = 50
crossbows = 30
catapults = 1
"#,
    );
}

#[test]
fn a_mod_overrides_a_rule_and_a_sprite_at_once() {
    let install = TempDir::new("install");
    let mods = TempDir::new("mods");
    fake_install(&install);

    mods.write("longbows/mod.toml", "[mod]\nid = \"longbows\"\nname = \"Longbows\"\nversion = \"1.0\"\n");
    mods.write(
        "longbows/rules/longbows.toml",
        "[battle.three_bridges.attacker]\ncrossbows = 40\n",
    );
    mods.write("longbows/Base1a.pl8", "modded sprite a");

    let p = Platform::builder()
        .base(install.path())
        .mods_dir(mods.path())
        .enable(["longbows"])
        .build()
        .expect("loads");

    assert_eq!(p.load_order.len(), 1);
    assert_eq!(p.load_order[0].name, "Longbows");

    // Asset: shadowed.
    assert_eq!(p.vfs.read_to_string("Base1a.pl8").unwrap(), "modded sprite a");
    assert_eq!(p.vfs.read_to_string("base1b.pl8").unwrap(), "base sprite b");

    // Rule: merged, with the untouched siblings intact.
    assert_eq!(p.rules.integer("battle.three_bridges.attacker.crossbows").unwrap(), 40);
    assert_eq!(p.rules.integer("battle.three_bridges.attacker.peasants").unwrap(), 100);
    assert_eq!(p.rules.integer("battle.three_bridges.defender.crossbows").unwrap(), 30);

    // And the report says exactly what happened.
    let report = p.report();
    assert_eq!(report.shadowed_assets.len(), 1);
    assert_eq!(report.shadowed_assets[0].0, "base1a.pl8");
    assert_eq!(report.overrides.len(), 1);
    assert_eq!(report.overrides[0].path, "battle.three_bridges.attacker.crossbows");
    let text = report.to_string();
    assert!(text.contains("longbows:rules/longbows.toml"), "{text}");
}

#[test]
fn two_mods_touching_the_same_rule_are_both_named() {
    let install = TempDir::new("install2");
    let mods = TempDir::new("mods2");
    fake_install(&install);
    for (id, n) in [("a", 40), ("b", 60)] {
        mods.write(&format!("{id}/mod.toml"), &format!("[mod]\nid = \"{id}\"\n"));
        mods.write(
            &format!("{id}/rules/r.toml"),
            &format!("[battle.three_bridges.attacker]\ncrossbows = {n}\n"),
        );
    }

    let p = Platform::builder()
        .base(install.path())
        .mods_dir(mods.path())
        .enable(["a", "b"])
        .build()
        .unwrap();

    assert_eq!(p.rules.integer("battle.three_bridges.attacker.crossbows").unwrap(), 60);
    let report = p.report();
    assert_eq!(report.overrides.len(), 2);
    let text = report.to_string();
    assert!(text.contains("a:rules/r.toml"), "{text}");
    assert!(text.contains("b:rules/r.toml"), "{text}");
}

#[test]
fn a_clean_load_reports_nothing() {
    let install = TempDir::new("install3");
    fake_install(&install);
    let p = Platform::builder().base(install.path()).build().unwrap();
    assert!(p.report().is_quiet());
    assert_eq!(p.report().to_string().trim(), "no conflicts");
}

#[test]
fn the_typed_troop_rules_come_out_of_the_merged_document() {
    let install = TempDir::new("install4");
    let mods = TempDir::new("mods4");
    fake_install(&install);
    mods.write("harder/mod.toml", "[mod]\nid = \"harder\"\n");
    mods.write(
        "harder/rules/harder.toml",
        "[difficulty.very_hard]\nscale_percent = 50\n\n[battle.three_bridges.defender]\ncatapults = 4\n",
    );

    let p = Platform::builder()
        .base(install.path())
        .mods_dir(mods.path())
        .enable(["harder"])
        .build()
        .unwrap();

    let rules = TroopRules::from_ruleset(&p.rules).expect("typed view");
    assert_eq!(rules.troops.len(), 3);
    assert_eq!(rules.troops[0].id, "peasants");
    let battle = rules.battle("three_bridges").expect("battle");
    assert_eq!(battle.defensive_advantage, 5);

    // Normal is unscaled.
    let normal = rules.army(battle, "normal", Side::Attacker);
    assert_eq!(rules.count(&normal, "peasants"), Some(100));
    assert_eq!(rules.count(&normal, "crossbows"), Some(20));

    // The mod halved very_hard, and siege equipment is never scaled.
    let hard = rules.army(battle, "very_hard", Side::Attacker);
    assert_eq!(rules.count(&hard, "peasants"), Some(50));
    assert_eq!(rules.count(&hard, "catapults"), Some(2));

    // The mod also raised the defenders' catapults, which the cap allows.
    assert_eq!(battle.defender[7], 4);
}

#[test]
fn a_rule_outside_its_range_fails_the_load_with_the_mods_file_named() {
    let install = TempDir::new("install5");
    let mods = TempDir::new("mods5");
    fake_install(&install);
    mods.write("silly/mod.toml", "[mod]\nid = \"silly\"\n");
    // The original engine would silently clamp this to 9.
    mods.write("silly/rules/silly.toml", "[battle.three_bridges.attacker]\ncatapults = 40\n");

    let p = Platform::builder()
        .base(install.path())
        .mods_dir(mods.path())
        .enable(["silly"])
        .build()
        .unwrap();

    let e = TroopRules::from_ruleset(&p.rules).unwrap_err().to_string();
    assert!(e.contains("silly:rules/silly.toml"), "{e}");
    assert!(e.contains("outside 0..=9"), "{e}");
}

#[test]
fn enabling_nothing_gives_the_base_game() {
    let install = TempDir::new("install6");
    let mods = TempDir::new("mods6");
    fake_install(&install);
    mods.write("unused/mod.toml", "[mod]\nid = \"unused\"\n");
    mods.write("unused/rules/x.toml", "[battle.three_bridges.attacker]\ncrossbows = 999\n");

    let p = Platform::builder().base(install.path()).mods_dir(mods.path()).build().unwrap();
    assert!(p.load_order.is_empty());
    assert_eq!(p.rules.integer("battle.three_bridges.attacker.crossbows").unwrap(), 20);
    assert_eq!(p.vfs.layers().len(), 1);
}

#[test]
fn a_mod_can_be_supplied_directly_without_a_mods_directory() {
    let install = TempDir::new("install7");
    let modd = TempDir::new("standalone");
    fake_install(&install);
    modd.write("mod.toml", "[mod]\nid = \"dev\"\n");
    modd.write("rules/dev.toml", "[battle.three_bridges.attacker]\ncrossbows = 7\n");

    let meta = l2_mods::ModMeta::load(modd.path()).unwrap();
    let p = Platform::builder()
        .base(install.path())
        .add_mod(meta)
        .enable(["dev"])
        .build()
        .unwrap();
    assert_eq!(p.rules.integer("battle.three_bridges.attacker.crossbows").unwrap(), 7);
}

/// The example mod shipped in `example-mods/` is the one printed in
/// `docs/modding.md`. Loading it here is what stops the documentation drifting
/// away from the code.
#[test]
fn the_documented_example_mod_is_real_and_loads() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("example-mods").join("longbows");
    assert!(dir.is_dir(), "{} is missing", dir.display());

    let meta = l2_mods::ModMeta::load(&dir).expect("manifest parses");
    assert_eq!(meta.id, "longbows");
    assert_eq!(meta.version, l2_mods::Version::new(1, 0, 0));
    assert!(meta.description.unwrap().contains("Archers"));

    let mut rs = l2_mods::Ruleset::new();
    rs.apply_file(&dir.join("rules").join("longbows.toml"), "longbows:rules/longbows.toml")
        .expect("rules parse");
    assert_eq!(rs.integer("battle.three_bridges.attacker.archers").unwrap(), 120);
    assert_eq!(rs.integer("difficulty.very_hard.scale_percent").unwrap(), 65);
}
