#![allow(unused_imports)]
use super::*;
use super::world_tests::*;
use super::deal_tests::*;
use super::*;
use super::diff::*;
use l2_formats::maps::{MapSet, Plane, PLANE_DIM, SLOT_LEN};
use l2_kingdom::map::MAP_TILES;
use l2_scenario::newgame::{self, MapError, NewGame};
use l2_scenario::{CountyState, Scenario};

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


