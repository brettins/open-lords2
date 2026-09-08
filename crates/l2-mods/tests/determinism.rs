//! Order-determinism, which is a correctness property here rather than a
//! preference.
//!
//! `docs/netcode.md` makes deterministic lockstep the architecture: two peers
//! execute the same commands and must reach bit-identical state. That
//! guarantee assumes they are running the same *rules*, and once rules arrive
//! by merging mod directories that assumption stops being free. `l2-net`'s
//! lobby now hashes the resolved ruleset and refuses a peer whose hash differs,
//! so a merge that is even slightly order-dependent turns into a refused
//! session — or worse, a session that starts and desyncs later.
//!
//! Three things could break it, and there is a test here for each:
//!
//! * **Iteration in hash order.** The value tree is `BTreeMap` throughout, and
//!   a source-level test makes adding a `HashMap` a visible decision rather
//!   than an accident.
//! * **Filesystem enumeration order.** `read_dir` gives no ordering guarantee
//!   and differs between filesystems. Every layer's documents are sorted by
//!   name before they apply.
//! * **Floats.** Not forbidden by the reader — a mod may carry one for
//!   something the simulation never reads — but they are reported, and the
//!   engine's own rules contain none.
//!
//! And one thing that must *not* affect the answer: where anything is
//! installed. Two players with the same mods at different paths are playing
//! the same game.

mod common;

use common::TempDir;
use l2_mods::{digest, Platform, Ruleset};

fn install(dir: &TempDir) {
    dir.write("Base1a.pl8", "sprite");
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
"#,
    );
}

fn write_mod(dir: &TempDir, id: &str, rules: &str) {
    dir.write(&format!("{id}/mod.toml"), &format!("[mod]\nid = \"{id}\"\n"));
    dir.write(&format!("{id}/rules/{id}.toml"), rules);
}

/// The load must be reproducible: same inputs, same bytes, every time.
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

/// The digest identifies the rules, not the installation. A player whose game
/// lives on `D:` and one whose lives on `/opt` must be allowed into the same
/// session.
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

    // Two separate reasons this holds, and both are load-bearing.
    //
    // An origin names the *layer* and the document's path *relative to that
    // layer* — never an absolute path. So a diagnostic printed on one machine
    // reads identically on another, and a bug report about
    // `longbows:rules/longbows.toml:12:11` means something to whoever wrote
    // the mod. That is a property of the origin, and it is asserted here
    // rather than only in the type's own tests, because the digest's
    // path-independence would otherwise look like a coincidence.
    let origin = |p: &Platform| {
        p.rules.origin("battle.three_bridges.attacker.archers").unwrap().to_string()
    };
    assert_eq!(origin(&p1), "aaa:rules/aaa.toml:2:11");
    assert_eq!(origin(&p1), origin(&p2));

    // And separately, origins are not in the hashed stream at all — see
    // `digest`'s module documentation. So even a layer id that legitimately
    // differed between two installs could not split a session.
}

/// Load order is the conflict-resolution policy, so reversing it where two
/// mods disagree must change the answer — and must be visible in the digest,
/// because the two players are no longer running the same rules.
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

/// Two orders that produce the same numbers are the same rules. The digest
/// hashes what survived, not the history of how it got there, so two players
/// who reordered mods that never touch each other are still compatible.
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

/// Documents inside one layer apply in sorted name order, whatever order the
/// filesystem hands them back in. `read_dir` promises nothing, and the two
/// filesystems this project runs on do not agree.
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

/// Building the same document set twice in different insertion orders must
/// give identical bytes, because the tree is ordered by key rather than by
/// arrival.
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

/// A known answer for the encoder itself, over a document that will never
/// change. If this ever moves, the byte stream moved — which means every
/// previously recorded digest is now wrong, and a peer on an older build will
/// be refused for no reason. That is why it is pinned rather than computed.
///
/// This is the same argument `l2-net` makes for freezing `hash.rs` and
/// `rng.rs`: the value stream is the product, not the code.
#[test]
fn the_encoder_produces_a_known_byte_stream() {
    let mut rs = Ruleset::new();
    rs.apply_str(
        "flag = true\nname = \"a\"\nn = 7\nlist = [1, 2]\n\n[t]\ninner = -3\n",
        "frozen",
    )
    .unwrap();
    assert_eq!(digest::digest(&rs), 0xc94e_b851_8cac_8674);
    assert_eq!(digest::digest_hex(&rs), "c94eb8518cac8674");
    assert_eq!(digest::encode(&rs).len(), 102);
}

/// Two values of different types that render the same must not hash the same.
#[test]
fn a_string_and_an_integer_that_look_alike_hash_differently() {
    let mut as_int = Ruleset::new();
    as_int.apply_str("x = 1\n", "a").unwrap();
    let mut as_str = Ruleset::new();
    as_str.apply_str("x = \"1\"\n", "a").unwrap();
    assert_ne!(digest::digest(&as_int), digest::digest(&as_str));

    // And field boundaries cannot move without the hash noticing, which is
    // what the length prefixes are for.
    let mut split_one = Ruleset::new();
    split_one.apply_str("a = \"xy\"\nb = \"z\"\n", "a").unwrap();
    let mut split_two = Ruleset::new();
    split_two.apply_str("a = \"x\"\nb = \"yz\"\n", "a").unwrap();
    assert_ne!(digest::digest(&split_one), digest::digest(&split_two));
}

/// The engine's own rules must contain no decimals, and a mod that introduces
/// one is reported rather than silently accepted.
#[test]
fn a_decimal_in_a_mod_is_reported_even_though_it_is_not_refused() {
    let base = TempDir::new("det-float-install");
    let mods = TempDir::new("det-float-mods");
    install(&base);
    write_mod(&mods, "floaty", "[battle.three_bridges]\nsome_ratio = 0.5\n");

    let p = Platform::builder()
        .base(base.path())
        .mods_dir(mods.path())
        .enable(["floaty"])
        .build()
        .unwrap();

    let report = p.report();
    assert_eq!(report.float_rules.len(), 1);
    assert_eq!(report.float_rules[0].0, "battle.three_bridges.some_ratio");
    assert!(format!("{report}").contains("cannot use"), "{report}");

    // Loading with no mods at all leaves no decimals anywhere.
    let plain = Platform::builder().base(base.path()).build().unwrap();
    assert!(plain.rules.float_rules().is_empty());
}

/// The guard that keeps the rest of this file true. `docs/netcode.md` bans
/// iteration whose order depends on hashing, and the cheapest way to keep that
/// rule is to make breaking it visible: this fails the moment a hash container
/// appears in the crate, so adding one has to be a decision someone takes on
/// purpose.
#[test]
fn no_hash_ordered_container_appears_anywhere_in_this_crate() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();
    let mut stack = vec![src];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("read src").flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("read source");
            for (n, line) in text.lines().enumerate() {
                // Prose about the rule is fine; a use of the type is not.
                let code = line.trim_start();
                if code.starts_with("//") || code.starts_with("*") {
                    continue;
                }
                if code.contains("HashMap") || code.contains("HashSet") {
                    offenders.push(format!("{}:{}: {}", path.display(), n + 1, line.trim()));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "hash-ordered containers make the merge order-dependent (docs/netcode.md):\n{}",
        offenders.join("\n")
    );
}
