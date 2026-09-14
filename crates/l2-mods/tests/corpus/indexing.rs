#![allow(unused_imports)]
use super::*;
use super::seeding::*;
use super::difficulty::*;
use common::TempDir;
use l2_mods::seed::{battle_names, parse_troops_eng, to_rules_toml, ROWS};
use l2_mods::{Platform, Ruleset, Side, TroopRules, Vfs};
use std::env;

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
    // through the overlay is the point: the
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
        l2_testkit::skip!("no .smk in this install - skipping the casing check");
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

