#![allow(unused_imports)]
use super::*;
use super::order_determinism::*;
use common::TempDir;
use l2_mods::{digest, Platform, Ruleset};

/// A known answer for the encoder itself, over a document that will never
/// change. If this ever moves, the byte stream moved — which means every
/// previously recorded digest is now wrong, and a peer on an older build will
/// be refused for no reason. That is why it is pinned
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
/// one is reported
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
/// iteration whose order depends on hashing, and the difference
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

/// The handshake digest is stricter than the rules digest
/// is deliberate. `docs/netcode.md` D-12 says peers must agree on the mod set
/// and load order, and `l2_net::Hello::ruleset_hash` documents exactly that —
/// so `session_digest` is what fills it, not `digest`.
#[test]
fn the_handshake_digest_sees_a_mod_list_the_rules_digest_cannot() {
    let base = TempDir::new("det-sess-install");
    let mods = TempDir::new("det-sess-mods");
    install(&base);
    // Two mods that set no rules at all: only a file each. Nothing they do
    // reaches the merged rule tree.
    for id in ["aaa", "zzz"] {
        mods.write(&format!("{id}/mod.toml"), &format!("[mod]\nid = \"{id}\"\n"));
        mods.write(&format!("{id}/Base1a.pl8"), &format!("a sprite from {id}"));
    }

    let load = |enabled: &[&str]| {
        Platform::builder()
            .base(base.path())
            .mods_dir(mods.path())
            .enable(enabled.to_vec())
            .build()
            .unwrap()
    };
    let none = load(&[]);
    let one = load(&["aaa"]);
    let both = load(&["aaa", "zzz"]);
    let reversed = load(&["zzz", "aaa"]);

    // The rules are identical in all four: no mod set a rule.
    assert_eq!(none.digest(), one.digest());
    assert_eq!(none.digest(), both.digest());
    assert_eq!(none.digest(), reversed.digest());

    // The handshake still refuses every pairing, because an asset a mod
    // replaces can be simulation input — a .skr battlefield is terrain — and
    // the rule tree would never show it.
    assert_ne!(none.session_digest(), one.session_digest());
    assert_ne!(one.session_digest(), both.session_digest());
    assert_ne!(both.session_digest(), reversed.session_digest(), "order counts too");

    // And it is still reproducible, which is the whole point of it existing.
    assert_eq!(both.session_digest(), load(&["aaa", "zzz"]).session_digest());
}

/// The handshake digest must not move when nothing about the game moved.
#[test]
fn the_handshake_digest_still_ignores_where_the_files_live() {
    let one_base = TempDir::new("det-sess-p1");
    let one_mods = TempDir::new("det-sess-p1m");
    let two_base = TempDir::new("det-sess-p2");
    let two_mods = TempDir::new("det-sess-p2m");
    for (b, m) in [(&one_base, &one_mods), (&two_base, &two_mods)] {
        install(b);
        write_mod(m, "aaa", "[battle.three_bridges.attacker]\narchers = 300\n");
    }
    let load = |b: &TempDir, m: &TempDir| {
        Platform::builder().base(b.path()).mods_dir(m.path()).enable(["aaa"]).build().unwrap()
    };
    assert_eq!(
        load(&one_base, &one_mods).session_digest(),
        load(&two_base, &two_mods).session_digest()
    );
}

