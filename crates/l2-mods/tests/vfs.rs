//! The overlay filesystem.

mod common;

use common::TempDir;
use l2_mods::Vfs;

#[test]
fn the_top_layer_wins_and_the_others_are_still_visible() {
    let base = TempDir::new("base");
    let modd = TempDir::new("mod");
    base.write("Base1a.pl8", "base sprite");
    base.write("Title.pl8", "base title");
    modd.write("Base1a.pl8", "modded sprite");

    let mut vfs = Vfs::new();
    vfs.push_layer("base", base.path()).unwrap();
    vfs.push_layer("prettier", modd.path()).unwrap();

    assert_eq!(vfs.read_to_string("Base1a.pl8").unwrap(), "modded sprite");
    assert_eq!(vfs.read_to_string("Title.pl8").unwrap(), "base title");
    assert_eq!(vfs.providers("Base1a.pl8").len(), 2);
    assert_eq!(vfs.providers("Base1a.pl8")[0].layer, 0);
    assert_eq!(vfs.shadowed(), vec![("base1a.pl8", vec!["base", "prettier"])]);
}

#[test]
fn lookup_ignores_case_in_every_direction() {
    let base = TempDir::new("case");
    // The real install's actual spelling.
    base.write("AXMEN.SMK", "video");
    base.write("Bat_los4.smk", "video");

    let mut vfs = Vfs::new();
    vfs.push_layer("base", base.path()).unwrap();

    // The executable asks for lowercase; the disk has neither casing.
    for asked in ["axmen.smk", "AXMEN.SMK", "Axmen.Smk", "aXmEn.sMk"] {
        assert!(vfs.exists(asked), "{asked} should resolve");
    }
    for asked in ["BAT_LOS4.SMK", "bat_los4.smk", "Bat_Los4.smk"] {
        assert!(vfs.exists(asked), "{asked} should resolve");
    }
    // And a name that genuinely is not there stays not there.
    assert!(!vfs.exists("axemen.smk"));
}

#[test]
fn separators_and_leading_dots_are_normalised() {
    let base = TempDir::new("sep");
    base.write("rules/troops.toml", "x = 1\n");

    let mut vfs = Vfs::new();
    vfs.push_layer("base", base.path()).unwrap();

    for asked in ["rules/troops.toml", r"rules\troops.toml", "./rules/troops.toml", "/rules/TROOPS.toml"] {
        assert!(vfs.exists(asked), "{asked} should resolve");
    }
}

#[test]
fn subdirectories_are_indexed_and_can_be_listed() {
    let base = TempDir::new("sub");
    base.write("rules/troops.toml", "");
    base.write("rules/deep/more.toml", "");
    base.write("art/Base1a.pl8", "");
    base.write("Top.pl8", "");

    let mut vfs = Vfs::new();
    vfs.push_layer("base", base.path()).unwrap();

    assert_eq!(vfs.entries_under("rules"), vec!["rules/deep/more.toml", "rules/troops.toml"]);
    assert_eq!(vfs.entries_with_extension("pl8"), vec!["art/base1a.pl8", "top.pl8"]);
    assert_eq!(vfs.entries().count(), 4);
}

#[test]
fn rules_are_enumerated_per_layer_because_they_merge_rather_than_shadow() {
    let base = TempDir::new("rbase");
    let modd = TempDir::new("rmod");
    base.write("rules/troops.toml", "x = 1\n");
    modd.write("rules/troops.toml", "x = 2\n");

    let mut vfs = Vfs::new();
    vfs.push_layer("base", base.path()).unwrap();
    vfs.push_layer("m", modd.path()).unwrap();

    // resolve() gives one winner - correct for a sprite, wrong for a rule.
    assert_eq!(vfs.providers("rules/troops.toml").len(), 2);
    assert_eq!(vfs.layer_entries_under(0, "rules").len(), 1);
    assert_eq!(vfs.layer_entries_under(1, "rules").len(), 1);
    assert_ne!(vfs.layer_entries_under(0, "rules")[0].1, vfs.layer_entries_under(1, "rules")[0].1);
}

#[test]
fn a_missing_file_is_an_error_that_names_it() {
    let base = TempDir::new("missing");
    let mut vfs = Vfs::new();
    vfs.push_layer("base", base.path()).unwrap();
    let e = vfs.read("Nope.pl8").unwrap_err().to_string();
    assert!(e.contains("Nope.pl8"), "{e}");
    assert!(vfs.resolve("Nope.pl8").is_none());
}

#[test]
fn a_layer_root_that_is_not_a_directory_is_rejected() {
    let mut vfs = Vfs::new();
    let e = vfs.push_layer("ghost", "definitely/not/here").unwrap_err().to_string();
    assert!(e.contains("ghost"), "{e}");
}

#[test]
fn the_same_layer_id_cannot_be_mounted_twice() {
    let base = TempDir::new("dupe");
    let mut vfs = Vfs::new();
    vfs.push_layer("base", base.path()).unwrap();
    assert!(vfs.push_layer("base", base.path()).is_err());
}

#[test]
fn nothing_in_the_api_can_write_to_a_layer() {
    // A compile-time property rather than a runtime one: Vfs exposes read,
    // read_to_string, resolve and the listing methods, and no counterpart that
    // takes bytes. This test exists so that adding one is a visible decision.
    let base = TempDir::new("ro");
    base.write("a.txt", "before");
    let mut vfs = Vfs::new();
    vfs.push_layer("base", base.path()).unwrap();
    assert_eq!(vfs.read_to_string("a.txt").unwrap(), "before");
    assert_eq!(std::fs::read_to_string(base.path().join("a.txt")).unwrap(), "before");
}

#[cfg(unix)]
#[test]
fn a_case_collision_within_one_layer_resolves_deterministically_and_is_reported() {
    // Only reachable on a case-sensitive filesystem. NTFS cannot hold both.
    let base = TempDir::new("collide");
    base.write("Sprite.pl8", "mixed");
    base.write("SPRITE.PL8", "upper");

    let mut vfs = Vfs::new();
    vfs.push_layer("base", base.path()).unwrap();

    assert_eq!(vfs.case_collisions().len(), 1);
    let c = &vfs.case_collisions()[0];
    assert_eq!(c.key, "sprite.pl8");
    assert_eq!(c.names, vec!["SPRITE.PL8", "Sprite.pl8"]);
    // Lowest raw name wins, every time, on every machine.
    assert_eq!(vfs.read_to_string("sprite.pl8").unwrap(), "upper");
}
