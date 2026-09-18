#![allow(unused_imports)]
use super::*;
use super::hashing_and_types::*;
use common::TempDir;
use l2_mods::{digest, Platform, Ruleset};

#[test]
fn the_same_mods_in_the_same_order_give_byte_identical_rules() {
    let base = TempDir::new("det-install");
    let mods = TempDir::new("det-mods");
    install(&base);
    write_mod(&mods, "aaa", "[battle.three_bridges.attacker]\narchers = 300\n");
    write_mod(&mods, "zzz", "[unit.archers]\narmour = 7\n");

    let build = || {
        Platform::builder()
            .base(base.path())
            .mods_dir(mods.path())
            .enable(["aaa", "zzz"])
            .build()
            .unwrap()
    };
    let a = build();
    let b = build();
    assert_eq!(digest::encode(&a.rules), digest::encode(&b.rules));
    assert_eq!(a.digest(), b.digest());
    assert_eq!(a.rules.leaves(), b.rules.leaves());
}

#[test]
fn where_the_files_live_does_not_change_the_digest() {
    let one_base = TempDir::new("det-p1-install");
    let one_mods = TempDir::new("det-p1-mods");
    let two_base = TempDir::new("det-p2-install");
    let two_mods = TempDir::new("det-p2-mods");
    for (b, m) in [(&one_base, &one_mods), (&two_base, &two_mods)] {
        install(b);
        write_mod(m, "aaa", "[battle.three_bridges.attacker]\narchers = 300\n");
    }

    let load = |b: &TempDir, m: &TempDir| {
        Platform::builder().base(b.path()).mods_dir(m.path()).enable(["aaa"]).build().unwrap()
    };
    let p1 = load(&one_base, &one_mods);
    let p2 = load(&two_base, &two_mods);

    assert_ne!(one_base.path(), two_base.path(), "the fixture is only useful if they differ");
    assert_eq!(p1.digest(), p2.digest());

    let origin = |p: &Platform| {
        p.rules.origin("battle.three_bridges.attacker.archers").unwrap().to_string()
    };
    assert_eq!(origin(&p1), "aaa:rules/aaa.toml:2:11");
    assert_eq!(origin(&p1), origin(&p2));

}

#[test]
fn reversing_the_order_of_two_mods_that_disagree_changes_the_digest() {
    let base = TempDir::new("det-ord-install");
    let mods = TempDir::new("det-ord-mods");
    install(&base);
    write_mod(&mods, "aaa", "[battle.three_bridges.attacker]\narchers = 300\n");
    write_mod(&mods, "zzz", "[battle.three_bridges.attacker]\narchers = 400\n");

    let load = |order: [&str; 2]| {
        Platform::builder()
            .base(base.path())
            .mods_dir(mods.path())
            .enable(order)
            .build()
            .unwrap()
    };
    let forward = load(["aaa", "zzz"]);
    let backward = load(["zzz", "aaa"]);
    assert_eq!(forward.rules.integer("battle.three_bridges.attacker.archers").unwrap(), 400);
    assert_eq!(backward.rules.integer("battle.three_bridges.attacker.archers").unwrap(), 300);
    assert_ne!(forward.digest(), backward.digest());
}

#[test]
fn reordering_mods_that_do_not_overlap_leaves_the_digest_alone() {
    let base = TempDir::new("det-noov-install");
    let mods = TempDir::new("det-noov-mods");
    install(&base);
    write_mod(&mods, "aaa", "[battle.three_bridges.attacker]\narchers = 300\n");
    write_mod(&mods, "zzz", "[unit.archers]\narmour = 7\n");

    let load = |order: [&str; 2]| {
        Platform::builder()
            .base(base.path())
            .mods_dir(mods.path())
            .enable(order)
            .build()
            .unwrap()
    };
    assert_eq!(load(["aaa", "zzz"]).digest(), load(["zzz", "aaa"]).digest());
}

#[test]
fn documents_within_a_layer_apply_in_sorted_name_order_not_creation_order() {
    let base = TempDir::new("det-docs");
    base.write("rules/zz-last.toml", "[unit.archers]\narmour = 99\n");
    base.write("rules/aa-first.toml", "[unit.archers]\narmour = 1\n");

    let p = Platform::builder().base(base.path()).build().unwrap();
    assert_eq!(
        p.rules.integer("unit.archers.armour").unwrap(),
        99,
        "zz-last.toml sorts last and therefore wins, regardless of when it was written"
    );
    let applied: Vec<&str> =
        p.rules.documents.iter().map(|d| d.source.as_str()).collect();
    let base_docs: Vec<&&str> = applied.iter().filter(|s| s.starts_with("base:")).collect();
    assert_eq!(base_docs, vec![&"base:rules/aa-first.toml", &"base:rules/zz-last.toml"]);
}

#[test]
fn the_merged_tree_is_ordered_by_key_and_not_by_arrival() {
    let mut a = Ruleset::new();
    a.apply_str("[z.b]\nx = 1\n", "one").unwrap();
    a.apply_str("[a.c]\ny = 2\n", "two").unwrap();

    let mut b = Ruleset::new();
    b.apply_str("[a.c]\ny = 2\n", "two").unwrap();
    b.apply_str("[z.b]\nx = 1\n", "one").unwrap();

    assert_eq!(digest::encode(&a), digest::encode(&b));
    assert_eq!(a.leaves(), vec!["a.c.y".to_string(), "z.b.x".to_string()]);
    assert_eq!(a.leaves(), b.leaves());
}

