
mod common;

use common::TempDir;
use l2_mods::{Fate, Platform};

fn fake_install(dir: &TempDir) {
    dir.write("Base1a.pl8", "base sprite a");
    dir.write("Base1b.pl8", "base sprite b");
    dir.write(
        "rules/troops.toml",
        r#"
[troop.archers]
column = 5
name = "Archers"
siege_engine = false

[difficulty.normal]
order = 2
scale_percent = 100

[battle.three_bridges]
index = 0
name = "Three Bridges"
defensive_advantage = 5

[battle.three_bridges.attacker]
archers = 200

[battle.three_bridges.defender]
archers = 100
"#,
    );
}

fn mod_with(mods: &TempDir, id: &str, rules: &str) {
    mods.write(&format!("{id}/mod.toml"), &format!("[mod]\nid = \"{id}\"\n"));
    mods.write(&format!("{id}/rules/{id}.toml"), rules);
}

fn effect_of<'a>(
    effects: &'a [l2_mods::LayerEffect],
    id: &str,
) -> &'a l2_mods::LayerEffect {
    effects.iter().find(|e| e.id == id).unwrap_or_else(|| panic!("no layer '{id}'"))
}

fn fate_of(effects: &[l2_mods::LayerEffect], id: &str, path: &str) -> Fate {
    effect_of(effects, id)
        .rules
        .iter()
        .find(|c| c.path == path)
        .unwrap_or_else(|| panic!("'{id}' never claimed '{path}'"))
        .fate
        .clone()
}

#[test]
fn a_rule_that_lost_to_a_later_mod_names_the_mod_that_won() {
    let install = TempDir::new("eff-install");
    let mods = TempDir::new("eff-mods");
    fake_install(&install);
    mod_with(&mods, "aaa", "[battle.three_bridges.attacker]\narchers = 300\n");
    mod_with(&mods, "zzz", "[battle.three_bridges.attacker]\narchers = 400\n");

    let p = Platform::builder()
        .base(install.path())
        .mods_dir(mods.path())
        .enable(["aaa", "zzz"])
        .build()
        .unwrap();
    let effects = p.effects();

    let path = "battle.three_bridges.attacker.archers";
    match fate_of(&effects, "aaa", path) {
        Fate::LostTo(who) => assert!(who.starts_with("zzz:"), "lost to {who}"),
        other => panic!("expected a loss, got {other}"),
    }
    match fate_of(&effects, "zzz", path) {
        Fate::Overrode(prev) => assert!(prev.starts_with("aaa:"), "overrode {prev}"),
        other => panic!("expected an override, got {other}"),
    }
    assert_eq!(p.rules.integer(path).unwrap(), 400);

    assert!(effect_of(&effects, "aaa").is_inert());
    assert!(!effect_of(&effects, "zzz").is_inert());
    assert_eq!(p.report().inert_layers, vec!["aaa".to_string()]);
    let text = p.effect_report();
    assert!(text.contains("aaa: HAS NO EFFECT"), "{text}");
}

#[test]
fn a_misspelt_rule_is_reported_as_added_rather_than_overriding() {
    let install = TempDir::new("typo-install");
    let mods = TempDir::new("typo-mods");
    fake_install(&install);
    mod_with(
        &mods,
        "typo",
        "[battle.three_brdiges.attacker]\narchers = 999\n",
    );

    let p = Platform::builder()
        .base(install.path())
        .mods_dir(mods.path())
        .enable(["typo"])
        .build()
        .unwrap();

    let effects = p.effects();
    let path = "battle.three_brdiges.attacker.archers";
    assert_eq!(fate_of(&effects, "typo", path), Fate::Added);

    assert!(p.report().overrides.is_empty());
    let report = p.report();
    assert_eq!(report.added_rules, vec![(path.to_string(), "typo".to_string())]);
    assert!(format!("{report}").contains("check the spelling"), "{report}");

    assert_eq!(p.rules.integer("battle.three_bridges.attacker.archers").unwrap(), 200);
}

#[test]
fn a_rule_removed_by_a_later_delete_says_who_removed_it() {
    let install = TempDir::new("del-install");
    let mods = TempDir::new("del-mods");
    fake_install(&install);
    mod_with(&mods, "aaa", "[battle.three_bridges.attacker]\narchers = 300\n");
    mod_with(
        &mods,
        "zzz",
        "[battle.three_bridges]\n\"$delete\" = [\"attacker\"]\n",
    );

    let p = Platform::builder()
        .base(install.path())
        .mods_dir(mods.path())
        .enable(["aaa", "zzz"])
        .build()
        .unwrap();

    assert!(p.rules.get("battle.three_bridges.attacker.archers").is_none());
    let effects = p.effects();
    match fate_of(&effects, "aaa", "battle.three_bridges.attacker.archers") {
        Fate::Deleted(_) => {}
        other => panic!("expected a deletion, got {other}"),
    }
}

#[test]
fn a_shadowed_sprite_names_the_layer_that_covered_it() {
    let install = TempDir::new("shadow-install");
    let mods = TempDir::new("shadow-mods");
    fake_install(&install);
    mods.write("aaa/mod.toml", "[mod]\nid = \"aaa\"\n");
    mods.write("aaa/Base1a.pl8", "aaa sprite");
    mods.write("aaa/Base1b.pl8", "aaa other sprite");
    mods.write("zzz/mod.toml", "[mod]\nid = \"zzz\"\n");
    mods.write("zzz/Base1a.pl8", "zzz sprite");

    let p = Platform::builder()
        .base(install.path())
        .mods_dir(mods.path())
        .enable(["aaa", "zzz"])
        .build()
        .unwrap();

    let effects = p.effects();
    let aaa = effect_of(&effects, "aaa");
    let shadowed: Vec<_> =
        aaa.assets.iter().filter(|a| a.shadowed_by.is_some()).map(|a| a.name.as_str()).collect();
    assert_eq!(shadowed, vec!["base1a.pl8"]);
    assert_eq!(aaa.assets_winning(), 1, "base1b.pl8 still wins");
    assert!(!aaa.is_inert(), "a mod that still lands one file is not inert");
    assert_eq!(p.vfs.read("Base1a.pl8").unwrap(), b"zzz sprite");
}

#[test]
fn two_mods_with_identically_named_rule_files_do_not_shadow_each_other() {
    let install = TempDir::new("dual-install");
    let mods = TempDir::new("dual-mods");
    fake_install(&install);
    mods.write("aaa/mod.toml", "[mod]\nid = \"aaa\"\n");
    mods.write("aaa/rules/rules.toml", "[battle.three_bridges.attacker]\narchers = 300\n");
    mods.write("zzz/mod.toml", "[mod]\nid = \"zzz\"\n");
    mods.write("zzz/rules/rules.toml", "[battle.three_bridges.defender]\narchers = 5\n");

    let p = Platform::builder()
        .base(install.path())
        .mods_dir(mods.path())
        .enable(["aaa", "zzz"])
        .build()
        .unwrap();

    assert_eq!(p.rules.integer("battle.three_bridges.attacker.archers").unwrap(), 300);
    assert_eq!(p.rules.integer("battle.three_bridges.defender.archers").unwrap(), 5);
    assert!(
        p.report().shadowed_assets.is_empty(),
        "rule documents are merged, not shadowed: {:?}",
        p.report().shadowed_assets
    );
    assert_eq!(p.vfs.providers("rules/rules.toml").len(), 2);
}

#[test]
fn an_empty_mod_is_reported_as_empty_rather_than_as_overridden() {
    let install = TempDir::new("empty-install");
    let mods = TempDir::new("empty-mods");
    fake_install(&install);
    mods.write("hollow/mod.toml", "[mod]\nid = \"hollow\"\n");

    let p = Platform::builder()
        .base(install.path())
        .mods_dir(mods.path())
        .enable(["hollow"])
        .build()
        .unwrap();

    let effects = p.effects();
    let hollow = effect_of(&effects, "hollow");
    assert!(hollow.is_empty());
    assert!(hollow.is_inert());
    assert!(p.report().inert_layers.is_empty());
    assert!(p.effect_report().contains("hollow: supplied nothing"));
}

#[test]
fn the_core_ruleset_is_the_bottom_layer_and_is_reported_as_one() {
    let install = TempDir::new("core-install");
    fake_install(&install);
    let p = Platform::builder().base(install.path()).build().unwrap();

    let effects = p.effects();
    assert_eq!(effects[0].id, l2_mods::CORE_LAYER);
    assert_eq!(effects[1].id, l2_mods::BASE_LAYER);
    assert!(effects[0].rules_winning() > 100, "the core ruleset is not small");
    assert!(effects[0].assets.is_empty(), "the core layer has no files; it is compiled in");
    assert!(p.report().overrides.is_empty(), "{}", p.report());
}
