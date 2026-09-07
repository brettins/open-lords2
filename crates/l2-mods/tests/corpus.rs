//! Validates the platform against a real game install.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-mods -- --nocapture
//! ```
//!
//! Skips (rather than fails) when unset, matching `l2-formats`. No assets live
//! in this repository and none are written by these tests: the install is
//! mounted as a read-only layer and every file this test creates goes in a
//! temporary directory.

mod common;

use common::TempDir;
use l2_mods::seed::{battle_names, parse_troops_eng, to_rules_toml, ROWS};
use l2_mods::{Platform, Ruleset, Side, TroopRules, Vfs};
use std::env;
use std::path::Path;

fn install() -> Option<String> {
    env::var("LORDS2_DIR").ok().filter(|d| Path::new(d).is_dir())
}

macro_rules! skip_without_install {
    () => {
        match install() {
            Some(d) => d,
            None => {
                eprintln!("LORDS2_DIR not set or not a directory - skipping corpus test");
                return;
            }
        }
    };
}

#[test]
fn the_whole_install_indexes_and_every_pl8_decodes_through_the_overlay() {
    let dir = skip_without_install!();
    let mut vfs = Vfs::new();
    vfs.push_layer("base", &dir).unwrap();

    let pl8s = vfs.entries_with_extension("pl8");
    println!("indexed {} entries, {} of them .pl8", vfs.entries().count(), pl8s.len());
    assert!(pl8s.len() >= 200, "expected the sprite corpus, found {}", pl8s.len());

    let mut frames = 0usize;
    for name in &pl8s {
        let bytes = vfs.read(name).expect("read through vfs");
        let pl8 = l2_formats::Pl8::parse(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        pl8.validate().unwrap_or_else(|e| panic!("{name}: {e}"));
        frames += pl8.frames.len();
    }
    println!("{} frames decoded through the overlay", frames);

    // The counts docs/formats/pl8.md records, per release. Reaching them
    // through the overlay rather than straight off disk is the point: the
    // indirection must not lose or reorder a single file.
    match pl8s.len() {
        291 => assert_eq!(frames, 21_344, "Windows release"),
        222 => assert_eq!(frames, 14_648, "DOS release"),
        n => println!("{n} .pl8 files: not a release with a recorded frame count"),
    }
}

#[test]
fn the_installs_inconsistent_casing_resolves_the_way_the_executable_asks() {
    let dir = skip_without_install!();
    let mut vfs = Vfs::new();
    vfs.push_layer("base", &dir).unwrap();

    // On disk: AXMEN.SMK, Axemen.smk, Bat_los4.smk, BAT_LOS5.SMK. The
    // executable asks in lowercase. All four must resolve, and the two
    // similarly-named-but-different videos must stay distinct.
    let smks = vfs.entries_with_extension("smk");
    println!("{} videos indexed", smks.len());
    if smks.is_empty() {
        // The 1996 DOS release ships no video at all; eng.md records the same
        // split for BATTLES.ENG. Pointing LORDS2_DIR at it is legitimate.
        eprintln!("no .smk in this install - skipping the casing check");
        return;
    }
    for name in &smks {
        assert!(vfs.exists(&name.to_ascii_uppercase()), "{name} uppercased");
        assert!(vfs.exists(&name.to_ascii_lowercase()), "{name} lowercased");
    }
    if vfs.exists("axmen.smk") && vfs.exists("axemen.smk") {
        assert_ne!(
            vfs.resolve("axmen.smk"),
            vfs.resolve("axemen.smk"),
            "AXMEN.SMK and Axemen.smk are different files, not case variants"
        );
    }
    // Whatever the host filesystem does, one layer must not be ambiguous.
    assert!(vfs.case_collisions().is_empty(), "{:?}", vfs.case_collisions());
}

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

        // docs/formats/eng.md: the engine clamps the four siege columns to 9,
        // and no shipped file exceeds it.
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
        // Only the Windows release shipped these; see docs/formats/eng.md.
        eprintln!("no TROOPS*.ENG in {dir} - skipping");
    }
}

#[test]
fn a_mod_layered_on_the_real_install_changes_one_number_and_nothing_else() {
    let dir = skip_without_install!();

    // Seed a base ruleset into a temporary directory. Nothing is written to
    // the install, and nothing generated here is ever committed.
    let base = TempDir::new("seeded");
    let mut vfs = Vfs::new();
    vfs.push_layer("scan", &dir).unwrap();
    let Ok(troops) = vfs.read("TROOPS2.ENG").or_else(|_| vfs.read("TROOPS.ENG")) else {
        eprintln!("no TROOPS*.ENG in this install - skipping");
        return;
    };
    let names = vfs.read("BATTLES.ENG").map(|b| battle_names(&b)).unwrap_or_default();
    let table = parse_troops_eng(&troops).expect("seed");
    base.write("rules/troops.toml", &to_rules_toml(&table, &names, "TROOPS2.ENG"));

    // Read the seeded ruleset back through the platform, with no mods, to get
    // the "before" picture.
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

    // Unchanged difficulty behaviour on real numbers.
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
        eprintln!("no TROOPS2.ENG in this install - skipping");
        return;
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

    // Count the leaves the example sets, then insist that every one of them
    // *overrode* something. A leaf that merely got added means the example
    // named a battle, side or troop that does not exist - a typo the merge
    // would otherwise accept in silence.
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

    // And it still validates as a troop table.
    let rules = TroopRules::from_ruleset(&p.rules).expect("still valid");
    assert_eq!(rules.battle("three_bridges").unwrap().attacker[5], 120);
    assert_eq!(rules.difficulty("very_hard").unwrap().scale_percent, 65);
}

fn count_leaves(v: &l2_mods::Value) -> usize {
    match v {
        l2_mods::Value::Table(t) => t.values().map(|c| count_leaves(&c.value)).sum(),
        _ => 1,
    }
}
