#![allow(unused_imports)]
use super::*;
use super::deal_tests::*;
use super::validation_tests::*;
use super::*;
use super::diff::*;
use l2_formats::maps::{MapSet, Plane, PLANE_DIM, SLOT_LEN};
use l2_kingdom::map::MAP_TILES;
use l2_scenario::newgame::{self, MapError, NewGame};
use l2_scenario::{CountyState, Scenario};

/// **Every shipped map builds a world**, and each one closes on itself.
///
/// Running all 44 is `docs/plan.md`'s C26: a rule can
/// be wrong at 45 of its 51 inputs and stay invisible behind a fixture that
/// exercises one. Every claim here is a per-map invariant, so a map that breaks
/// one names itself.
#[test]
fn every_shipped_map_builds_a_world_that_closes_on_itself() {
    let bytes = maps!();
    let set = MapSet::parse(&bytes).expect("L2_maps.dat parses");
    let used = set.used_slots();
    assert!(!used.is_empty(), "the shipped file has used slots");
    let mut checked = 0;
    let mut counties_seen = 0;
    let mut smithless = Vec::new();
    for slot_index in used {
        let slot = set.slot(slot_index).expect("a used slot");
        let setup = NewGame { slot: slot_index, lords: 1, ..NewGame::default() };
        let w = newgame::build(&slot, &setup)
            .unwrap_or_else(|e| panic!("slot {slot_index} does not build: {e}"));
        let label = format!("slot {slot_index}");

        assert_eq!(w.county_count, slot.county_count(), "{label}: county count");
        assert!(w.county_count >= 4, "{label}: every shipped map has at least four counties");

        for c in 1..=w.county_count {
            counties_seen += 1;
            assert_ne!(w.town_tile[c], 0, "{label} county {c}: no town tile");
            assert_ne!(w.castle_tile[c], 0, "{label} county {c}: no castle plot");
            assert_eq!(
                w.tiles.content[w.castle_tile[c]],
                0x14,
                "{label} county {c}: the castle plot is not stamped"
            );
            assert!(!w.neighbours[c].is_empty(), "{label} county {c}: an island county");
            if !w.has_resource[c][2] {
                smithless.push((slot_index, c));
            }
            // Exactly four `0x10` plots, which is what stops the dwelling array
            // running into `fieldProgress` — `maps-layers.md` §2.3 measured it
            // over the file and this measures it after the load.
            let plots = (0..MAP_TILES)
                .filter(|&t| w.tiles.county[t] as usize == c && w.tiles.flags[t] & 0x10 != 0)
                .count();
            assert_eq!(plots, 4, "{label} county {c}: {plots} dwelling plots");
            // The twenty-slot table is a bound, and the razing is what keeps it
            // one: after the load no county may have a farm tile with no slot.
            let fields = w.field_tiles[c].iter().filter(|&&t| t != 0).count();
            let farm_tiles = (0..MAP_TILES)
                .filter(|&t| w.tiles.county[t] as usize == c && w.tiles.flags[t] & 0x20 != 0)
                .count();
            assert_eq!(fields, farm_tiles, "{label} county {c}: fields vs surviving farm tiles");
            assert!(fields <= 20, "{label} county {c}: {fields} fields");
        }

        // The player-start table and `l2-formats`' own count of it agree — the
        // one counts writes and the other counts distinct markers, and no
        // shipped map repeats a marker.
        let starts: Vec<u8> = (1..6).map(|m| w.player_start[m]).filter(|&c| c != 0).collect();
        assert_eq!(
            w.player_start_count,
            slot.player_start_count(),
            "{label}: the counter and the table disagree, so a marker is repeated"
        );
        assert_eq!(starts.len(), w.player_start_count, "{label}: start table length");
        assert!(matches!(starts.len(), 2 | 4 | 5), "{label}: {} seats", starts.len());
        for (i, &c) in starts.iter().enumerate() {
            assert!(c as usize <= w.county_count, "{label}: start {i} names county {c}");
        }
        for row in 0..6 {
            for &c in w.routes.row(row).iter().filter(|&&c| c != 0) {
                assert!(c as usize <= w.county_count, "{label} route {row}: county {c}");
            }
        }
        checked += 1;
    }
    eprintln!("{checked} shipped maps built, {counties_seen} counties");
    assert_eq!(checked, 44, "the shipped file has 44 used slots");
    assert_eq!(counties_seen, 434, "434 counties over the 44 used slots — maps.md §2");

    // **A correction, and it is worth more than the assertion it replaces.**
    // `docs/symbols.json` says of `County_PlaceBlacksmith` that the weapons
    // site "is derived
    // one". 433 of 434 do. County 4 of slot 8 (Africa) has **no tile whose
    // flags byte is zero at all** — its 57 tiles are all road, boundary, rough,
    // plot, farmland, town or site — and the candidate test is `flags == 0`
    // exactly, so the original's `local_14` stays 0 and its guard refuses.
    // That county can never make a weapon.
    assert_eq!(smithless, vec![(8, 4)], "the set of counties with no blacksmith has changed");
}

/// **`PlayerStart_Compact`'s bubble is a slice, and this is why.**
///
/// The original drops table entries whose slot number is above the live realm
/// count by bubbling them down. `newgame`'s version takes the first `lords`
/// entries instead. The two agree exactly when every populated entry's slot
/// number is its own index and the populated entries are contiguous from 1 —
/// which is a property of the **maps**, and is what is checked here.
#[test]
fn the_player_start_markers_are_contiguous_from_one_on_every_shipped_map() {
    let bytes = maps!();
    let set = MapSet::parse(&bytes).expect("L2_maps.dat parses");
    for slot_index in set.used_slots() {
        let slot = set.slot(slot_index).expect("a used slot");
        let mut markers = Vec::new();
        for y in 0..PLANE_DIM {
            for x in 0..PLANE_DIM {
                let m = slot.at(Plane::Marker, x, y);
                // The `0x80` arm — the castle — is the player start.
                if m != 0 && slot.flags_at(x, y) & 0x80 != 0 {
                    markers.push(m);
                }
            }
        }
        markers.sort_unstable();
        let n = markers.len();
        assert_eq!(
            markers,
            (1..=n as u8).collect::<Vec<u8>>(),
            "slot {slot_index}: the start markers are not 1..{n}"
        );
    }
}

