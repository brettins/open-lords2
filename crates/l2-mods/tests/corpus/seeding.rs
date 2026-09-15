#![allow(unused_imports)]
use super::*;
use super::indexing::*;
use super::difficulty::*;
use common::TempDir;
use l2_mods::seed::{battle_names, parse_troops_eng, to_rules_toml, ROWS};
use l2_mods::{Platform, Ruleset, Side, TroopRules, Vfs};
use std::env;

#[test]
fn every_shipped_troops_file_seeds_a_ruleset_that_reads_back() {
    let dir = skip_without_install!();
    let mut vfs = Vfs::new();
    vfs.push_layer("base", &dir).unwrap();

    let names = match vfs.read("BATTLES.ENG") {
        Ok(bytes) => battle_names(&bytes),
        Err(_) => Vec::new(), // the DOS install has no BATTLES.ENG
    };
    println!("{} battle names", names.len());

    let mut seeded = 0;
    for file in ["TROOPS.ENG", "TROOPS2.ENG", "TROOPS3.ENG"] {
        let Ok(bytes) = vfs.read(file) else { continue };
        let table = parse_troops_eng(&bytes).unwrap_or_else(|e| panic!("{file}: {e}"));

        for row in 0..ROWS {
            for g in 0..5 {
                for s in 0..2 {
                    for c in 7..11 {
                        let v = table.counts[row][g][s][c];
                        assert!(v <= 9, "{file}: siege column {c} of row {row} is {v}");
                    }
                }
            }
        }

        let text = to_rules_toml(&table, &names, file);
        let mut rs = Ruleset::new();
        rs.apply_str(&text, &format!("base:rules/{file}.toml"))
            .unwrap_or_else(|e| panic!("{file}: generated rules do not parse: {e}"));
        let rules = TroopRules::from_ruleset(&rs)
            .unwrap_or_else(|e| panic!("{file}: generated rules do not validate: {e}"));
        assert_eq!(rules.battles.len(), ROWS);
        assert_eq!(rules.troops.len(), 11);
        seeded += 1;
        println!("{file}: {} battles seeded", rules.battles.len());
    }
    if seeded == 0 {
        l2_testkit::skip!("no TROOPS*.ENG in {dir} - skipping");
    }
}

#[test]
fn a_mod_layered_on_the_real_install_changes_one_number_and_nothing_else() {
    let dir = skip_without_install!();

    let base = TempDir::new("seeded");
    let mut vfs = Vfs::new();
    vfs.push_layer("scan", &dir).unwrap();
    let Ok(troops) = vfs.read("TROOPS2.ENG").or_else(|_| vfs.read("TROOPS.ENG")) else {
        l2_testkit::skip!("no TROOPS*.ENG in this install");
    };
    let names = vfs.read("BATTLES.ENG").map(|b| battle_names(&b)).unwrap_or_default();
    let table = parse_troops_eng(&troops).expect("seed");
    base.write("rules/troops.toml", &to_rules_toml(&table, &names, "TROOPS2.ENG"));

    let plain = Platform::builder().base(base.path()).build().expect("seeded rules load");
    let before_rules = TroopRules::from_ruleset(&plain.rules).expect("seeded rules validate");
    assert_eq!(before_rules.battles.len(), ROWS);
    let target = before_rules.battles[0].id.clone();
    let before = before_rules.battles[0].attacker;

    let mods = TempDir::new("realmods");
    mods.write("tweak/mod.toml", "[mod]\nid = \"tweak\"\n");
    mods.write(
        "tweak/rules/tweak.toml",
        &format!("[battle.{target}.attacker]\npeasants = 12345\n"),
    );

    let p = Platform::builder()
        .base(base.path())
        .mods_dir(mods.path())
        .enable(["tweak"])
        .build()
        .expect("loads");

    let after = TroopRules::from_ruleset(&p.rules).unwrap();
    let b = after.battle(&target).unwrap();
    assert_eq!(b.attacker[0], 12345);
    for (c, &was) in before.iter().enumerate().skip(1) {
        assert_eq!(b.attacker[c], was, "column {c} should not have moved");
    }
    assert_eq!(p.report().overrides.len(), 1);
    println!("{}", p.report());

    let normal = after.army(b, "normal", Side::Attacker);
    let easy = after.army(b, "very_easy", Side::Attacker);
    assert_eq!(easy[0], normal[0] * 116 / 100);
}

#[test]
fn the_example_mod_only_changes_rules_that_the_real_game_actually_has() {
    let dir = skip_without_install!();

    let base = TempDir::new("exbase");
    let mut scan = Vfs::new();
    scan.push_layer("scan", &dir).unwrap();
    let Ok(troops) = scan.read("TROOPS2.ENG") else {
        l2_testkit::skip!("no TROOPS2.ENG in this install - skipping");
    };
    let names = scan.read("BATTLES.ENG").map(|b| battle_names(&b)).unwrap_or_default();
    let table = parse_troops_eng(&troops).expect("seed");
    base.write("rules/troops.toml", &to_rules_toml(&table, &names, "TROOPS2.ENG"));

    let p = Platform::builder()
        .base(base.path())
        .mods_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("example-mods"))
        .enable(["longbows"])
        .build()
        .expect("the documented example loads against real data");

    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("example-mods/longbows/rules/longbows.toml"),
    )
    .unwrap();
    let doc = l2_mods::reader::parse(&text, "example").unwrap();
    let leaves = count_leaves(&doc.value);
    assert!(leaves > 0);
    assert_eq!(
        p.report().overrides.len(),
        leaves,
        "every rule the example sets should already exist; report:\n{}",
        p.report()
    );
    assert!(p.report().dangling_deletes.is_empty());
    assert!(p.report().type_changes.is_empty());

    let rules = TroopRules::from_ruleset(&p.rules).expect("still valid");
    assert_eq!(rules.battle("three_bridges").unwrap().attacker[5], 120);
    assert_eq!(rules.difficulty("very_hard").unwrap().scale_percent, 65);

    assert_eq!(p.troop_table().unwrap().stats(l2_sim::Troop::Archers).armour, 4);

    println!("--- report ---\n{}", p.report());
    println!("--- effects ---\n{}", p.effect_report());
    println!("--- digest --- {}", l2_mods::digest_hex(&p.rules));
}

fn count_leaves(v: &l2_mods::Value) -> usize {
    match v {
        l2_mods::Value::Table(t) => t.values().map(|c| count_leaves(&c.value)).sum(),
        _ => 1,
    }
}

