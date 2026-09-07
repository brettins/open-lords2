//! Mod metadata, discovery and load order.

mod common;

use common::TempDir;
use l2_mods::{discover, resolve_load_order, LoadOrderError, ModMeta, Version, VersionReq};
use std::path::Path;

fn meta(text: &str) -> ModMeta {
    ModMeta::from_str(text, Path::new("test-mod")).unwrap()
}

fn ids(order: &[ModMeta]) -> Vec<&str> {
    order.iter().map(|m| m.id.as_str()).collect()
}

#[test]
fn a_manifest_reads_every_field() {
    let m = meta(
        r#"
        [mod]
        id = "longbows"
        name = "Longbow Rebalance"
        version = "1.2.3"
        author = "Someone"
        description = """
        Makes archers worth fielding.
        """
        requires = ["core >= 1.0"]
        after = ["ui-tweaks"]
        conflicts = ["shortbows"]
        "#,
    );
    assert_eq!(m.id, "longbows");
    assert_eq!(m.name, "Longbow Rebalance");
    assert_eq!(m.version, Version::new(1, 2, 3));
    assert_eq!(m.author.as_deref(), Some("Someone"));
    assert!(m.description.unwrap().contains("archers"));
    assert_eq!(m.requires[0].id, "core");
    assert_eq!(m.requires[0].req, VersionReq::AtLeast(Version::new(1, 0, 0)));
    assert_eq!(m.after, vec!["ui-tweaks"]);
    assert_eq!(m.conflicts, vec!["shortbows"]);
}

#[test]
fn a_minimal_manifest_is_one_line_of_content() {
    let m = meta("[mod]\nid = \"tiny\"\n");
    assert_eq!(m.id, "tiny");
    assert_eq!(m.name, "tiny");
    assert_eq!(m.version, Version::new(0, 0, 0));
    assert!(m.requires.is_empty());
}

#[test]
fn bad_manifests_say_what_is_wrong() {
    assert!(ModMeta::from_str("name = \"x\"\n", Path::new("m")).is_err());
    assert!(ModMeta::from_str("[mod]\nname = \"x\"\n", Path::new("m")).is_err());
    let e = ModMeta::from_str("[mod]\nid = \"has space\"\n", Path::new("m"))
        .unwrap_err()
        .to_string();
    assert!(e.contains("must be non-empty"), "{e}");
    let e = ModMeta::from_str("[mod]\nid = \"a\"\nversion = \"1.2.3.4\"\n", Path::new("m"))
        .unwrap_err()
        .to_string();
    assert!(e.contains("too many components"), "{e}");
}

#[test]
fn version_requirements_behave() {
    let v = Version::parse;
    assert_eq!(v("2").unwrap(), Version::new(2, 0, 0));
    assert_eq!(v("2.1").unwrap(), Version::new(2, 1, 0));
    assert!(v("2.x").is_err());

    assert!(VersionReq::AtLeast(v("1.2").unwrap()).matches(v("1.3").unwrap()));
    assert!(!VersionReq::AtLeast(v("1.2").unwrap()).matches(v("1.1.9").unwrap()));
    assert!(VersionReq::Exactly(v("1.2.0").unwrap()).matches(v("1.2").unwrap()));
    assert!(!VersionReq::Exactly(v("1.2.0").unwrap()).matches(v("1.2.1").unwrap()));
    assert!(VersionReq::Compatible(v("1.2").unwrap()).matches(v("1.9").unwrap()));
    assert!(!VersionReq::Compatible(v("1.2").unwrap()).matches(v("2.0").unwrap()));
    // Below 1.0 the minor is the breaking component, as Cargo treats it.
    assert!(VersionReq::Compatible(v("0.4.1").unwrap()).matches(v("0.4.9").unwrap()));
    assert!(!VersionReq::Compatible(v("0.4.1").unwrap()).matches(v("0.5.0").unwrap()));
}

#[test]
fn discovery_finds_one_level_of_subdirectories() {
    let dir = TempDir::new("discover");
    dir.write("longbows/mod.toml", "[mod]\nid = \"longbows\"\n");
    dir.write("sieges/mod.toml", "[mod]\nid = \"sieges\"\n");
    dir.write("notamod/readme.txt", "hello");
    dir.write("longbows/examples/inner/mod.toml", "[mod]\nid = \"inner\"\n");

    let found = discover(dir.path()).unwrap();
    assert_eq!(ids(&found), vec!["longbows", "sieges"]);
    assert_eq!(found[0].root, dir.path().join("longbows"));
}

#[test]
fn the_users_order_is_kept_when_nothing_forbids_it() {
    let mods = vec![meta("[mod]\nid = \"a\"\n"), meta("[mod]\nid = \"b\"\n"), meta("[mod]\nid = \"c\"\n")];
    let order = resolve_load_order(&mods, &["c".into(), "a".into(), "b".into()]).unwrap();
    assert_eq!(ids(&order), vec!["c", "a", "b"]);
}

#[test]
fn a_dependency_is_moved_before_its_dependent() {
    let mods = vec![
        meta("[mod]\nid = \"core\"\nversion = \"1.0\"\n"),
        meta("[mod]\nid = \"addon\"\nrequires = [\"core\"]\n"),
    ];
    // User asked for addon first; the constraint overrules that one edge.
    let order = resolve_load_order(&mods, &["addon".into(), "core".into()]).unwrap();
    assert_eq!(ids(&order), vec!["core", "addon"]);
}

#[test]
fn after_orders_without_requiring() {
    let mods = vec![
        meta("[mod]\nid = \"ui\"\n"),
        meta("[mod]\nid = \"patch\"\nafter = [\"ui\", \"absent\"]\n"),
    ];
    let order = resolve_load_order(&mods, &["patch".into(), "ui".into()]).unwrap();
    assert_eq!(ids(&order), vec!["ui", "patch"]);
    // 'absent' is not enabled, and that is not an error the way requires is.
    let order = resolve_load_order(&mods, &["patch".into()]).unwrap();
    assert_eq!(ids(&order), vec!["patch"]);
}

#[test]
fn the_order_is_the_same_every_time() {
    let mods = vec![
        meta("[mod]\nid = \"a\"\n"),
        meta("[mod]\nid = \"b\"\nrequires = [\"a\"]\n"),
        meta("[mod]\nid = \"c\"\nrequires = [\"a\"]\n"),
        meta("[mod]\nid = \"d\"\nrequires = [\"b\", \"c\"]\n"),
    ];
    let enabled: Vec<String> = ["d", "c", "b", "a"].iter().map(|s| s.to_string()).collect();
    let first = resolve_load_order(&mods, &enabled).unwrap();
    for _ in 0..20 {
        assert_eq!(ids(&resolve_load_order(&mods, &enabled).unwrap()), ids(&first));
    }
    assert_eq!(ids(&first), vec!["a", "c", "b", "d"]);
}

#[test]
fn a_missing_dependency_names_both_mods() {
    let mods = vec![meta("[mod]\nid = \"addon\"\nrequires = [\"core\"]\n")];
    let e = resolve_load_order(&mods, &["addon".into()]).unwrap_err();
    assert_eq!(
        e,
        LoadOrderError::MissingDependency { dependent: "addon".into(), needs: "core".into() }
    );
    assert!(e.to_string().contains("not enabled"));
}

#[test]
fn a_version_that_is_too_old_is_reported_with_both_versions() {
    let mods = vec![
        meta("[mod]\nid = \"core\"\nversion = \"1.0.0\"\n"),
        meta("[mod]\nid = \"addon\"\nrequires = [\"core >= 2.0\"]\n"),
    ];
    let e = resolve_load_order(&mods, &["core".into(), "addon".into()]).unwrap_err().to_string();
    assert!(e.contains(">= 2.0.0"), "{e}");
    assert!(e.contains("1.0.0"), "{e}");
}

#[test]
fn a_declared_conflict_stops_the_load() {
    let mods = vec![
        meta("[mod]\nid = \"a\"\nconflicts = [\"b\"]\n"),
        meta("[mod]\nid = \"b\"\n"),
    ];
    assert!(resolve_load_order(&mods, &["a".into(), "b".into()]).is_err());
    assert!(resolve_load_order(&mods, &["a".into()]).is_ok());
}

#[test]
fn a_cycle_is_reported_with_the_loop_spelled_out() {
    let mods = vec![
        meta("[mod]\nid = \"a\"\nafter = [\"c\"]\n"),
        meta("[mod]\nid = \"b\"\nafter = [\"a\"]\n"),
        meta("[mod]\nid = \"c\"\nafter = [\"b\"]\n"),
    ];
    let e = resolve_load_order(&mods, &["a".into(), "b".into(), "c".into()]).unwrap_err();
    match &e {
        LoadOrderError::Cycle(ids) => {
            assert!(ids.len() >= 3, "{ids:?}");
            assert_eq!(ids.first(), ids.last());
        }
        other => panic!("expected a cycle, got {other:?}"),
    }
    assert!(e.to_string().contains("->"));
}

#[test]
fn unknown_and_duplicate_ids_are_refused() {
    let mods = vec![meta("[mod]\nid = \"a\"\n")];
    assert_eq!(
        resolve_load_order(&mods, &["z".into()]).unwrap_err(),
        LoadOrderError::Unknown("z".into())
    );
    assert_eq!(
        resolve_load_order(&mods, &["a".into(), "a".into()]).unwrap_err(),
        LoadOrderError::Duplicate("a".into())
    );
}
