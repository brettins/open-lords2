//! The engine's own ruleset, and the two directions it must not drift in.
//!
//! `crates/l2-mods/rulesets/core/rules/*.toml` are *rendered* from the tables
//! they describe. That makes drift impossible in a way review never does:
//!
//! * **file → code.** Loading the shipped documents must reproduce
//!   `l2_sim::TroopTable::DEFAULT` and `l2_kingdom::tables::Tables::DEFAULT`
//!   exactly. If someone edits a number in the `.toml`, this fails.
//! * **code → file.** The shipped text must be byte-identical to what the
//!   renderer produces now. If someone edits a constant in Rust, this fails.
//!
//! Neither half alone is enough. The first would let a Rust constant change
//! with nobody noticing the document had gone stale; the second would let the
//! document and the renderer agree on something the loader could not read.
//!
//! To regenerate after a deliberate change to either table:
//!
//! ```text
//! L2_MODS_REGENERATE=1 cargo test -p l2-mods --test core
//! ```
//!
//! and then run the tests again with the variable unset, which is what CI and
//! everyone else does.

use l2_kingdom::tables::Tables;
use l2_mods::{core, kingdom, units, Ruleset};
use l2_sim::{TroopTable, ALL_TROOPS};

fn regenerating() -> bool {
    std::env::var("L2_MODS_REGENERATE").is_ok_and(|v| v != "0" && !v.is_empty())
}

#[test]
fn the_shipped_core_documents_are_exactly_what_the_renderer_produces() {
    let dir = core::source_dir();
    let mut stale = Vec::new();
    for (name, rendered) in core::render() {
        let path = dir.join(name);
        let on_disk = std::fs::read_to_string(&path).unwrap_or_default();
        if on_disk == rendered {
            continue;
        }
        if regenerating() {
            std::fs::write(&path, &rendered).expect("write regenerated ruleset");
            eprintln!("regenerated {}", path.display());
        } else {
            stale.push(name);
        }
    }
    assert!(
        stale.is_empty(),
        "{stale:?} no longer match the tables they were rendered from.\n\
         Regenerate with: L2_MODS_REGENERATE=1 cargo test -p l2-mods --test core"
    );
}

#[test]
fn the_core_ruleset_parses_and_every_document_is_accounted_for() {
    if regenerating() {
        return; // the files on disk are mid-rewrite
    }
    let rs = Ruleset::core();
    assert_eq!(rs.documents.len(), core::DOCUMENTS.len());
    // Nothing in the core layer may override anything else in it: the two
    // documents are disjoint, and if they ever stop being, the order they
    // apply in starts mattering and someone has to decide what it should be.
    assert!(
        rs.log.overrides.is_empty(),
        "the core documents overlap: {:?}",
        rs.log.overrides
    );
    assert!(rs.log.deletions.is_empty());
    assert!(rs.log.dangling_deletes.is_empty());
}

#[test]
fn loading_the_shipped_units_document_reproduces_the_simulation_table() {
    if regenerating() {
        return;
    }
    let rs = Ruleset::core();
    let loaded = units::troop_table(&rs).expect("core unit rules load");
    assert_eq!(loaded, TroopTable::DEFAULT);
}

#[test]
fn loading_the_shipped_kingdom_document_reproduces_the_economy_table() {
    if regenerating() {
        return;
    }
    let rs = Ruleset::core();
    let loaded = kingdom::tables(&rs).expect("core kingdom rules load");
    assert_eq!(loaded, Tables::DEFAULT);
}

/// The mod-facing format writes one `workforce` per castle, because an author
/// should not have to reproduce an oddity of the 1996 binary's memory layout.
/// That is only lossless while both columns hold the same number, which
/// `docs/kingdom.md` says they do in all five rows — and which is exactly the
/// kind of fact that stops being true when someone learns what the second
/// column is for.
#[test]
fn the_castle_workforce_pair_is_still_two_copies_of_one_number() {
    for (i, (a, b)) in Tables::DEFAULT.castle.workforce.iter().enumerate() {
        assert_eq!(
            a, b,
            "castle {} workforce columns have diverged: the ruleset format holds one \
             number and can no longer represent this",
            i + 1
        );
    }
}

/// `troop.<id>` and `unit.<id>` describe the same eleven soldiers through two
/// namespaces with different authorities. The ids are what links them, and a
/// mod author typing one has already met the other.
#[test]
fn the_unit_ids_are_the_same_ids_the_seeded_army_table_uses() {
    let seeded: Vec<&str> =
        l2_mods::seed::TROOP_COLUMNS.iter().map(|(id, _, _, _)| *id).collect();
    assert_eq!(units::UNIT_IDS.to_vec(), seeded);
}

#[test]
fn every_unit_id_round_trips_to_its_troop_and_back() {
    for troop in ALL_TROOPS {
        let id = units::unit_id(troop);
        assert_eq!(units::troop_for_id(id), Some(troop), "{id}");
    }
    assert_eq!(units::troop_for_id("no-such-unit"), None);
}

/// The shipped documents must contain no decimals at all. The simulation is
/// integer-only (`docs/netcode.md`), so a decimal in the *engine's own* rules
/// would be a value that cannot reach it — and would put a float into the
/// digest two peers compare.
#[test]
fn the_core_ruleset_contains_no_floating_point_at_all() {
    if regenerating() {
        return;
    }
    let rs = Ruleset::core();
    assert!(rs.float_rules().is_empty(), "{:?}", rs.float_rules());
}

/// A copy written for a player to read is the same text the engine runs on.
#[test]
fn writing_the_core_ruleset_out_gives_back_the_same_documents() {
    let dir = tempdir("corewrite");
    let written = core::write_to(&dir).expect("write core rules");
    assert_eq!(written.len(), core::DOCUMENTS.len());
    for (source, text) in core::DOCUMENTS {
        let name = source.rsplit('/').next().unwrap();
        let on_disk = std::fs::read_to_string(dir.join("rules").join(name)).unwrap();
        assert_eq!(on_disk, text);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

fn tempdir(tag: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let p = std::env::temp_dir().join(format!("l2mods-{tag}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&p).unwrap();
    p
}
