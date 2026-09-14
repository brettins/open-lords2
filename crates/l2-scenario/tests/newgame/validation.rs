#![allow(unused_imports)]
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

/// A map that seats two cannot be started with five lords, and the refusal is a
/// refusal.
#[test]
fn a_two_seat_map_refuses_five_lords() {
    let bytes = maps!();
    let set = MapSet::parse(&bytes).expect("L2_maps.dat parses");
    let two_seaters: Vec<usize> = set
        .used_slots()
        .into_iter()
        .filter(|&i| set.slot(i).unwrap().player_start_count() == 2)
        .collect();
    assert_eq!(two_seaters.len(), 8, "eight shipped maps seat two — maps-layers.md §5.1");
    let slot_index = two_seaters[0];
    let slot = set.slot(slot_index).expect("a used slot");
    let setup = NewGame { slot: slot_index, lords: 5, ..NewGame::default() };
    assert_eq!(
        Scenario::from_map(&slot, &setup),
        Err(MapError::TooManyLords { lords: 5, seats: 2 })
    );
}

/// **Which realm you play is rolled, and the seed is what rolls it.**
#[test]
fn the_start_table_is_dealt_and_the_deal_follows_the_seed() {
    let bytes = maps!();
    let set = MapSet::parse(&bytes).expect("L2_maps.dat parses");
    let slot = set.slot(ENGLAND).expect("slot 0");
    let owner_of = |seed: u64| {
        let setup = NewGame { slot: ENGLAND, lords: 5, local_player: 1, seed, ..NewGame::default() };
        let s = Scenario::from_map(&slot, &setup).expect("England builds");
        let mut v: Vec<(u8, u8)> = (1..=s.county_count)
            .filter_map(|id| s.counties[id].as_ref().map(|c| (id as u8, c.owner)))
            .filter(|&(_, o)| o != 0)
            .collect();
        v.sort_unstable();
        v
    };
    let a = owner_of(1);
    // The same seed twice is the same world — a lockstep peer's whole
    // requirement of this constructor.
    assert_eq!(a, owner_of(1), "the deal is a function of the seed");
    // The set of owned counties never moves; only who owns them.
    for seed in 0..24u64 {
        let deal = owner_of(seed);
        let counties: Vec<u8> = deal.iter().map(|&(c, _)| c).collect();
        assert_eq!(counties, ENGLAND_STARTS, "seed {seed}: the start counties moved");
        let mut realms: Vec<u8> = deal.iter().map(|&(_, r)| r).collect();
        realms.sort_unstable();
        assert_eq!(realms, vec![1, 2, 3, 4, 5], "seed {seed}: one county each");
    }
    let deals: std::collections::BTreeSet<Vec<(u8, u8)>> =
        (0..64u64).map(owner_of).collect();
    assert!(deals.len() > 1, "the deal never moves, so it is not a deal");
}

// ------------------------------------------------------------------ the diff

];

/// **Enumerate the fields; do not spot-check them.**
///
/// Reads `pub struct CountyState`'s field list out of
/// `crates/l2-scenario/src/mod.rs` and fails if a field is not in [`JUDGED`].
/// A hand-written list goes stale the day somebody adds a field; a list checked
/// against the definition cannot.
///
/// It needs no game, so it runs on CI — which is the point, because the diff
/// itself is fixture-gated and this is the half of it that is not.
#[test]
fn every_county_field_is_accounted_for() {
    let src = include_str!("../src/lib.rs");
    let start = src.find("pub struct CountyState {").expect("the struct is still called that");
    let body = &src[start..];
    let end = body.find("\n}").expect("the struct closes");
    let mut fields = Vec::new();
    for line in body[..end].lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("pub ") else { continue };
        let Some(name) = rest.split(':').next() else { continue };
        if name.is_empty() || name.contains(' ') {
            continue;
        }
        fields.push(name.to_string());
    }
    assert!(fields.len() > 20, "the parser found only {} fields", fields.len());
    let missing: Vec<&String> = fields.iter().filter(|f| !JUDGED.contains(&f.as_str())).collect();
    assert!(
        missing.is_empty(),
        "CountyState fields the two-constructor diff does not judge: {missing:?}.\n\
         Add each to JUDGED with a verdict, or say in the test why it cannot be compared."
    );
    let stale: Vec<&&str> = JUDGED.iter().filter(|c| !fields.iter().any(|f| f == *c)).collect();
    assert!(stale.is_empty(), "JUDGED names fields CountyState no longer has: {stale:?}");
}

// ------------------------------------------------------------------ ablation

/// **Break it on purpose and watch the checks go red.**
///
/// `docs/agents.md`: this project has twice shipped a check that passed with
/// the bug still in. The diff is worth exactly what its sensitivity is worth,
/// so this reproduces the three shapes of failure it exists to catch — a pass
/// that does not run, a pass that runs in the wrong order, and a table that
/// silently truncates — on a synthetic map, so it needs no install.
#[test]
fn the_checks_are_sensitive_to_the_three_mistakes_they_exist_for() {
    let slot_bytes = synthetic_slot();
    let set = MapSet::parse(&slot_bytes).unwrap();
    let good =
        newgame::build(&set.slot(0).unwrap(), &NewGame { lords: 1, ..NewGame::default() }).unwrap();

    // 1. The blacksmith pass not running: the county claims no weapons
    //    resource, which is what the corpus test's `has_resource[2]` catches.
    assert!(good.has_resource[1][2], "the blacksmith pass ran");
    // …and it went on a tile the file gave no flags at all.
    let smith = good.industry_site[1][2];
    assert_eq!(slot_bytes[Plane::Flags as usize * 4096 + smith], 0);

    // 2. The site passes in the wrong order: were the castle search to run
    //    before the resource sites, the mine — a `0x80` tile with no terrain —
    //    would be swallowed into the castle block and stamped `0x14`.
    assert_eq!(good.tiles.content[mine_tile()], 1, "the mine is iron, not a castle plot");
    assert_ne!(good.castle_tile[1], mine_tile(), "the castle did not eat the mine");

    // 3. The twenty-slot table dropping: a twenty-first
    // field would stay farmland with no entry, and the corpus test's
    //    `fields == surviving farm tiles` is what fails.
    let many = many_fields_slot();
    let set = MapSet::parse(&many).unwrap();
    let w = newgame::build(&set.slot(0).unwrap(), &NewGame { lords: 1, ..NewGame::default() })
        .unwrap();
    let left = (0..MAP_TILES).filter(|&t| w.tiles.flags[t] & 0x20 != 0).count();
    assert_eq!(left, 20, "a razed field stops being farmland");
    let listed = w.field_tiles[1].iter().filter(|&&t| t != 0).count();
    assert_eq!(listed, left, "every surviving field is in the table");
}

fn put(buf: &mut [u8], plane: Plane, x: usize, y: usize, v: u8) {
    buf[plane as usize * 4096 + y * PLANE_DIM + x] = v;
}

fn mine_tile() -> usize {
    30 * PLANE_DIM + 30
}

fn synthetic_slot() -> Vec<u8> {
    let mut buf = vec![0u8; SLOT_LEN];
    for y in 0..PLANE_DIM {
        for x in 0..PLANE_DIM {
            put(&mut buf, Plane::County, x, y, 1);
        }
    }
    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        put(&mut buf, Plane::Flags, 10 + dx, 10 + dy, 0x40);
        put(&mut buf, Plane::GfxBank, 10 + dx, 10 + dy, 0x0C);
        put(&mut buf, Plane::Flags, 20 + dx, 20 + dy, 0x80);
        put(&mut buf, Plane::GfxIndex, 20 + dx, 20 + dy, 6);
    }
    put(&mut buf, Plane::Marker, 20, 20, 1);
    // The mine: a Town-bank tile drawing frame 30, carrying the site bit.
    put(&mut buf, Plane::Flags, 30, 30, 0x80);
    put(&mut buf, Plane::GfxBank, 30, 30, 0x0C);
    put(&mut buf, Plane::GfxIndex, 30, 30, 30);
    for i in 0..4 {
        put(&mut buf, Plane::Flags, 40 + i, 40, 0x10);
    }
    for i in 0..2 {
        put(&mut buf, Plane::Flags, 12 + i, 12, 0x20);
        put(&mut buf, Plane::GfxBank, 12 + i, 12, 0x08);
        put(&mut buf, Plane::GfxIndex, 12 + i, 12, 80);
    }
    buf
}

fn many_fields_slot() -> Vec<u8> {
    let mut buf = synthetic_slot();
    for i in 0..25usize {
        let (x, y) = (i % 5 + 20, i / 5 + 40);
        put(&mut buf, Plane::Flags, x, y, 0x20);
        put(&mut buf, Plane::GfxBank, x, y, 0x08);
        put(&mut buf, Plane::GfxIndex, x, y, 80);
    }
    buf
}

