
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

#[test]
fn the_core_ruleset_contains_no_floating_point_at_all() {
    if regenerating() {
        return;
    }
    let rs = Ruleset::core();
    assert!(rs.float_rules().is_empty(), "{:?}", rs.float_rules());
}

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
