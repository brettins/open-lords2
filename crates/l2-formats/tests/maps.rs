//! Validates the map container against a real game install.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-formats -- --nocapture
//! ```
//!
//! These re-derive the structural claims in `docs/formats/maps.md` from the
//! bytes rather than trusting them. Skips when `LORDS2_DIR` is unset; no game
//! data lives in this repository.

use l2_formats::maps::{flags, Plane, MapSet, PLANE_DIM, SLOT_LEN};
use std::fs;

fn maps_file() -> Option<Vec<u8>> {
    l2_testkit::read_install("L2_maps.dat")
}

#[test]
fn the_windows_release_holds_eighty_slots_of_which_fortyfour_are_used() {
    let Some(bytes) = maps_file() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    assert_eq!(bytes.len() % SLOT_LEN, 0, "file is not a whole number of slots");

    let set = MapSet::parse(&bytes).expect("parse map set");
    let used = set.used_slots();
    println!(
        "L2_maps.dat: {} bytes = {} slots, {} used, {} empty",
        bytes.len(),
        set.slot_count(),
        used.len(),
        set.slot_count() - used.len()
    );
    assert_eq!(set.slot_count(), 80);
    assert_eq!(used.len(), 44);
    // Used slots come in two runs: 0..23 and 40..59.
    assert!(used.contains(&0) && used.contains(&23) && used.contains(&40) && used.contains(&59));
    assert!(!used.contains(&24) && !used.contains(&79));
}

#[test]
fn the_no_county_flag_agrees_with_the_county_plane_exactly() {
    let Some(bytes) = maps_file() else { return };
    let set = MapSet::parse(&bytes).unwrap();

    let (mut tiles, mut agree) = (0usize, 0usize);
    for i in set.used_slots() {
        let slot = set.slot(i).unwrap();
        for y in 0..PLANE_DIM {
            for x in 0..PLANE_DIM {
                let no_county = slot.flags_at(x, y) & flags::NO_COUNTY != 0;
                if no_county == (slot.county_at(x, y) == 0) {
                    agree += 1;
                }
                tiles += 1;
            }
        }
    }
    println!("NO_COUNTY flag agrees with county id on {agree}/{tiles} tiles");
    // This is claimed to hold at exactly 100%, so anything less is a red flag
    // about our plane indexing, not a tolerance to widen.
    assert_eq!(agree, tiles);
}

#[test]
fn castle_tiles_form_complete_two_by_two_blocks() {
    let Some(bytes) = maps_file() else { return };
    let set = MapSet::parse(&bytes).unwrap();

    let mut total_blocks = 0usize;
    for i in set.used_slots() {
        let slot = set.slot(i).unwrap();
        let castle = |x: usize, y: usize| slot.flags_at(x, y) & flags::CASTLE != 0;

        let mut marked = vec![false; PLANE_DIM * PLANE_DIM];
        let mut blocks = 0usize;
        for y in 0..PLANE_DIM {
            for x in 0..PLANE_DIM {
                if !castle(x, y) || marked[y * PLANE_DIM + x] {
                    continue;
                }
                // Top-left of a block: the other three must also be castle.
                assert!(
                    x + 1 < PLANE_DIM && y + 1 < PLANE_DIM,
                    "slot {i}: castle block runs off the edge at ({x},{y})"
                );
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    assert!(
                        castle(x + dx, y + dy),
                        "slot {i}: incomplete castle block at ({x},{y})"
                    );
                    marked[(y + dy) * PLANE_DIM + x + dx] = true;
                }
                blocks += 1;
            }
        }
        // One castle per county.
        assert_eq!(
            blocks,
            slot.county_count(),
            "slot {i}: {blocks} castle blocks but {} counties",
            slot.county_count()
        );
        total_blocks += blocks;
    }
    println!("{total_blocks} complete 2x2 castle blocks across 44 maps, one per county");
    assert_eq!(total_blocks, 434);
}

#[test]
fn the_dos_release_is_the_windows_file_truncated() {
    let Some(bytes) = maps_file() else {
        l2_testkit::skip!("no L2_maps.dat reachable");
    };
    // Optional: only runs if the older DOS install is also present.
    let Some(dos_dir) = l2_testkit::dos_install_dir() else {
        l2_testkit::skip!("no DOS install ({} unset)", l2_testkit::DOS_INSTALL_VAR);
    };
    let Ok(dos) = fs::read(dos_dir.join("L2_MAPS.DAT")) else {
        l2_testkit::skip!("{} holds no L2_MAPS.DAT", dos_dir.display());
    };
    assert_eq!(dos.len(), 40 * SLOT_LEN);
    assert_eq!(
        &bytes[..dos.len()],
        &dos[..],
        "the Windows file's first 40 slots should be byte-identical to the DOS file"
    );
    println!("DOS file is byte-identical to the first {} slots", dos.len() / SLOT_LEN);
}

#[test]
fn county_ids_stay_in_the_documented_range() {
    let Some(bytes) = maps_file() else { return };
    let set = MapSet::parse(&bytes).unwrap();
    let mut seen = [false; 256];
    for i in set.used_slots() {
        for &c in set.slot(i).unwrap().plane(Plane::County) {
            seen[c as usize] = true;
        }
    }
    let ids: Vec<usize> = (0..256).filter(|&i| seen[i]).collect();
    println!("county ids present: {ids:?}");
    // Documented as 1..=16 plus 32, with 0 meaning "no county".
    assert!(ids.iter().all(|&c| c == 0 || (1..=16).contains(&c) || c == 32));
}
