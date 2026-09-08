//! Packaging: what a mod is on disk, and what inspecting one tells you.
//!
//! Inspection is deliberately not the same thing as loading. Loading asks what
//! the game will run on once every layer has had its turn, and stops at the
//! first error. Inspection asks what *this one mod* says with nothing else in
//! the picture, and reports everything it finds — which is the right shape for
//! a tool an author runs against their own work before shipping it.
//!
//! Every fixture is written by the test.

mod common;

use common::TempDir;
use l2_mods::package::{self, Warning};
use l2_mods::Platform;

#[test]
fn a_mod_is_a_directory_with_a_manifest_and_nothing_else_is() {
    let dir = TempDir::new("pkg-notamod");
    dir.write("almost/rules/x.toml", "[battle.a]\nindex = 0\n");
    match package::inspect(&dir.path().join("almost")) {
        Err(package::PackageError::NotAMod(_)) => {}
        other => panic!("expected NotAMod, got {other:?}"),
    }
}

#[test]
fn inspection_lists_the_documents_the_files_and_the_rules_a_mod_claims() {
    let dir = TempDir::new("pkg-full");
    dir.write(
        "full/mod.toml",
        "[mod]\nid = \"full\"\nname = \"Full Example\"\nversion = \"1.2.3\"\n\
         author = \"a test\"\nrequires = [\"core >= 1.0\"]\nafter = [\"other\"]\n\
         conflicts = [\"rival\"]\n",
    );
    dir.write("full/rules/a.toml", "[unit.archers]\narmour = 4\n");
    dir.write("full/rules/b.toml", "[unit.knights]\narmour = 30\nexchange = 100\n");
    dir.write("full/Base1a.pl8", "a sprite");
    dir.write("full/art/Base1b.pl8", "another sprite");

    let p = package::inspect(&dir.path().join("full")).expect("inspects");
    assert_eq!(p.meta.id, "full");
    assert_eq!(p.meta.version.to_string(), "1.2.3");
    assert_eq!(p.rule_documents, vec!["rules/a.toml", "rules/b.toml"]);
    assert_eq!(p.assets, vec!["Base1a.pl8", "art/Base1b.pl8"]);
    assert_eq!(
        p.rule_paths,
        vec!["unit.archers.armour", "unit.knights.armour", "unit.knights.exchange"]
    );
    assert!(p.warnings.is_empty(), "{:?}", p.warnings);

    // The manifest's own fields survive into the printed form, because the
    // ordering constraints are the part a user most often needs to check.
    let text = p.to_string();
    assert!(text.contains("full 1.2.3"), "{text}");
    assert!(text.contains("requires  core >= 1.0.0"), "{text}");
    assert!(text.contains("conflicts rival"), "{text}");
}

/// The most useful warning in the set. A rule file in the wrong directory
/// loads with no error at all — it becomes an asset nothing asks for — and the
/// mod simply does nothing.
#[test]
fn a_rule_file_outside_the_rules_directory_is_warned_about() {
    let dir = TempDir::new("pkg-misplaced");
    dir.write("oops/mod.toml", "[mod]\nid = \"oops\"\n");
    dir.write("oops/longbows.toml", "[unit.archers]\narmour = 4\n");

    let p = package::inspect(&dir.path().join("oops")).expect("inspects");
    assert!(p.rule_paths.is_empty(), "nothing in rules/, so nothing is a rule");
    assert_eq!(p.warnings, vec![Warning::TomlOutsideRules("longbows.toml".into())]);
    assert!(p.to_string().contains("treated as a file to shadow"));
}

#[test]
fn a_non_toml_in_the_rules_directory_is_warned_about_because_it_is_skipped() {
    let dir = TempDir::new("pkg-nontoml");
    dir.write("notes/mod.toml", "[mod]\nid = \"notes\"\n");
    dir.write("notes/rules/README.txt", "hello");

    let p = package::inspect(&dir.path().join("notes")).expect("inspects");
    assert_eq!(p.warnings, vec![Warning::NonTomlInRules("rules/README.txt".into())]);
}

#[test]
fn a_mod_with_only_a_manifest_says_so() {
    let dir = TempDir::new("pkg-empty");
    dir.write("hollow/mod.toml", "[mod]\nid = \"hollow\"\n");
    let p = package::inspect(&dir.path().join("hollow")).expect("inspects");
    assert_eq!(p.warnings, vec![Warning::Empty]);
}

/// `docs/netcode.md` is categorical: the simulation is integer-only. A decimal
/// in a rule file cannot reach it, so it is either dead or a mistake, and
/// either way the author should hear about it before a player does.
#[test]
fn a_decimal_rule_is_warned_about_with_its_line() {
    let dir = TempDir::new("pkg-float");
    dir.write("floaty/mod.toml", "[mod]\nid = \"floaty\"\n");
    dir.write("floaty/rules/f.toml", "[unit.archers]\narmour = 4.5\n");

    let p = package::inspect(&dir.path().join("floaty")).expect("inspects");
    match &p.warnings[..] {
        [Warning::FloatRule { path, at }] => {
            assert_eq!(path, "unit.archers.armour");
            assert!(at.contains("floaty:rules/f.toml:2:"), "{at}");
        }
        other => panic!("expected one float warning, got {other:?}"),
    }
}

#[test]
fn a_syntax_error_is_found_at_inspection_rather_than_at_a_players_load() {
    let dir = TempDir::new("pkg-broken");
    dir.write("broken/mod.toml", "[mod]\nid = \"broken\"\n");
    dir.write("broken/rules/x.toml", "[unit.archers\narmour = 4\n");

    let err = package::inspect(&dir.path().join("broken")).unwrap_err();
    let text = err.to_string();
    assert!(text.contains("broken:rules/x.toml"), "{text}");
}

/// The digest identifies the *rules*, not the installation. Two copies of the
/// same mod at different paths are the same mod; a mod whose rules changed is
/// not.
#[test]
fn the_rules_digest_follows_the_rules_and_not_the_path() {
    let a = TempDir::new("pkg-diga");
    let b = TempDir::new("pkg-digb");
    let c = TempDir::new("pkg-digc");
    for (dir, armour) in [(&a, 4), (&b, 4), (&c, 5)] {
        dir.write("m/mod.toml", "[mod]\nid = \"m\"\n");
        dir.write("m/rules/r.toml", &format!("[unit.archers]\narmour = {armour}\n"));
    }
    let da = package::inspect(&a.path().join("m")).unwrap().rules_digest;
    let db = package::inspect(&b.path().join("m")).unwrap().rules_digest;
    let dc = package::inspect(&c.path().join("m")).unwrap().rules_digest;
    assert_eq!(da, db, "same rules in a different directory is the same mod");
    assert_ne!(da, dc, "a changed rule is a different mod");
}

/// Comments and whitespace are not rules. Two files that say the same thing
/// differently must have the same digest, or "have we got the same mod" gets
/// answered by a diff of formatting.
#[test]
fn the_rules_digest_ignores_layout_and_comments() {
    let a = TempDir::new("pkg-fmt-a");
    let b = TempDir::new("pkg-fmt-b");
    a.write("m/mod.toml", "[mod]\nid = \"m\"\n");
    a.write("m/rules/r.toml", "[unit.archers]\narmour = 4\nrecovery = 6\n");
    b.write("m/mod.toml", "[mod]\nid = \"m\"\n");
    b.write(
        "m/rules/r.toml",
        "# a rebalance\n\n[unit.archers]\nrecovery   =   6   # slower\narmour = 0x04\n",
    );
    assert_eq!(
        package::inspect(&a.path().join("m")).unwrap().rules_digest,
        package::inspect(&b.path().join("m")).unwrap().rules_digest
    );
}

#[test]
fn inspecting_a_directory_of_mods_returns_them_sorted_by_id() {
    let dir = TempDir::new("pkg-all");
    for id in ["zebra", "alpha", "middle"] {
        dir.write(&format!("{id}/mod.toml"), &format!("[mod]\nid = \"{id}\"\n"));
        dir.write(&format!("{id}/rules/r.toml"), "[unit.archers]\narmour = 1\n");
    }
    dir.write("not-a-mod/readme.txt", "hello");

    let all = package::inspect_all(dir.path()).expect("inspects all");
    let ids: Vec<&str> = all.iter().map(|p| p.meta.id.as_str()).collect();
    assert_eq!(ids, vec!["alpha", "middle", "zebra"]);
}

/// The package warnings reach the platform's report, so a player who never
/// runs an inspection tool still finds out.
#[test]
fn a_loaded_platform_reports_its_mods_packaging_warnings() {
    let install = TempDir::new("pkgw-install");
    let mods = TempDir::new("pkgw-mods");
    install.write("Base1a.pl8", "sprite");
    mods.write("oops/mod.toml", "[mod]\nid = \"oops\"\n");
    mods.write("oops/longbows.toml", "[unit.archers]\narmour = 4\n");

    let p = Platform::builder()
        .base(install.path())
        .mods_dir(mods.path())
        .enable(["oops"])
        .build()
        .unwrap();

    assert_eq!(p.package("oops").map(|p| p.meta.id.as_str()), Some("oops"));
    let report = p.report();
    assert_eq!(report.package_warnings.len(), 1);
    assert_eq!(report.package_warnings[0].0, "oops");
    assert!(format!("{report}").contains("mods worth a second look"), "{report}");
    // And the mod really did nothing: the engine's own archer armour stands.
    assert_eq!(p.rules.integer("unit.archers.armour").unwrap(), 0);
}

/// `is_platform_metadata` is the one place that decides which files in a mod
/// belong to the platform rather than to the game. Both the report and the
/// per-mod effect analysis go through it, so they cannot disagree.
#[test]
fn the_manifest_and_the_rule_documents_are_the_platforms_files() {
    assert!(package::is_platform_metadata("mod.toml"));
    assert!(package::is_platform_metadata("rules/troops.toml"));
    assert!(package::is_platform_metadata("rules/nested/deep.toml"));
    assert!(!package::is_platform_metadata("rules/README.txt"));
    assert!(!package::is_platform_metadata("rulesets/troops.toml"));
    assert!(!package::is_platform_metadata("rules.toml"));
    assert!(!package::is_platform_metadata("art/mod.toml"));
    assert!(!package::is_platform_metadata("Base1a.pl8"));
}
