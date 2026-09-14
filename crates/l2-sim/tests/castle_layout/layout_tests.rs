#![allow(unused_imports)]
use super::*;
use super::siege_tests::*;
use l2_sim::castle::{self, CastleSheets};
use l2_sim::siege::{
    self, DOCK_WALL_ELEVATION, FLAG_DRAWBRIDGE, FLAG_KEEP, FLAG_WALL, SURFACE_BAILEY,
    SURFACE_RAMPART_WALK, SURFACE_WATER,
};
use l2_sim::terrain::DIM;
use l2_sim::{BattleRunner, Muster, Troop};

/// **The file is exactly its own directory plus ten layers.** The shipped
/// `Stnfield.pl8` is 64,168 bytes: 168 of PL8 header and directory, then
/// 10 × 6,400. Five castles, two layers each, and the builder's `castle * 0x20`
/// stride is two 16-byte PL8 records — a directory written for a
/// sprite sheet answers a question about castles.
#[test]
fn the_layout_file_holds_five_castles_of_two_layers_each() {
    let s = sheets!();
    assert_eq!(s.len(), 5, "g_castleLevel is 0..=4");
    for level in 0..=4u8 {
        let sheet = s.get(level).expect("a castle a level");
        assert_eq!(sheet.frames.len(), castle::LAYER_BYTES);
        assert_eq!(sheet.structures.len(), castle::LAYER_BYTES);
        assert_ne!(sheet.frames, sheet.structures, "level {level}: two layers, not one twice");
    }
}

/// **The measurement the whole branch is for: a real castle has walls two
/// cells high, and ours has none.**
///
/// [`l2_sim::siege::DOCK_WALL_ELEVATION`] is 2 exactly — `FUN_00491492` tests
/// `elevation == 2` on the cell two ahead of a siege tower — and
/// `Oil_FindPourTarget` refuses to look below 2. [`siege::our_castle`] puts its
/// wall at [`siege::WALL_ELEVATION`], **one**, so both of those are dead
/// against it whatever the rules say.
///
/// Ablation, run: build with `siege::our_castle(level)` instead — red at every
/// level, *"level 0: our stand-in has 0 cells at elevation 2"*.
#[test]
fn every_real_castle_has_ground_two_cells_high_and_the_stand_in_has_none() {
    let s = sheets!();
    for level in 0..=4u8 {
        let real = castle::build(level, s.get(level).unwrap());
        let two = real.cells.iter().filter(|c| c.elevation == DOCK_WALL_ELEVATION).count();
        assert!(two > 100, "level {level}: only {two} cells at elevation 2");

        let ours = siege::our_castle(level);
        let ours_two = ours.cells.iter().filter(|c| c.elevation == DOCK_WALL_ELEVATION).count();
        assert_eq!(ours_two, 0, "level {level}: our stand-in has cells at elevation 2");
    }
}

/// **A siege tower can dock somewhere on every real castle, and nowhere on
/// ours.** [`siege::tower_dock_site`] is `FUN_00491492`'s search run from a
/// cell; this asks it of every cell of every castle, which is the question
/// *"is there anywhere at all a tower could dock"*.
#[test]
fn a_tower_finds_a_dock_on_every_real_castle_and_none_on_the_stand_in() {
    let s = sheets!();
    for level in 0..=4u8 {
        let real = castle::build(level, s.get(level).unwrap());
        let docks = (0..DIM as i32)
            .flat_map(|y| (0..DIM as i32).map(move |x| (x, y)))
            .filter(|&(x, y)| siege::tower_dock_site(&real, x, y, 0).is_some())
            .count();
        assert!(docks > 0, "level {level}: nowhere on the real castle a tower may dock");

        let ours = siege::our_castle(level);
        let ours_docks = (0..DIM as i32)
            .flat_map(|y| (0..DIM as i32).map(move |x| (x, y)))
            .filter(|&(x, y)| siege::tower_dock_site(&ours, x, y, 0).is_some())
            .count();
        assert_eq!(ours_docks, 0, "level {level}: the stand-in offered a dock");
    }
}

/// **The moat is where the layout puts it, and it is not "level 2 and up".**
///
/// Our stand-in gave every castle from level 2 a ditch, which read as a
/// sensible ladder and is not what the file says: `0xEE` — the `Battlefield_PlaceMoatCell`
/// escape — appears in the layers of levels **1, 3 and 4** and in neither 0
/// nor 2. A test used to assert the ladder; its premise was our own ring.
#[test]
fn the_moat_is_at_levels_one_three_and_four() {
    let s = sheets!();
    let moated: Vec<bool> = (0..=4u8)
        .map(|level| {
            castle::build(level, s.get(level).unwrap())
                .cells
                .iter()
                .any(|c| c.surface == SURFACE_WATER)
        })
        .collect();
    assert_eq!(moated, vec![false, true, false, true, true]);
}

/// **The Readme, from inside the raster.** *"Only the Stone and Royal castles
/// have drawbridges"* — structure code 9 is in the layers of levels 3 and 4
/// and no other, four cells each, which is the same answer the stone/wooden
/// table split gives from the other side.
#[test]
fn only_the_two_largest_castles_carry_a_drawbridge() {
    let s = sheets!();
    for level in 0..=4u8 {
        let f = castle::build(level, s.get(level).unwrap());
        let bridge = f.cells.iter().filter(|c| c.flags & FLAG_DRAWBRIDGE != 0).count();
        assert_eq!(bridge, if level >= 3 { 4 } else { 0 }, "level {level}");
    }
}

/// **One way in, and it is a way in whatever else the castle has.** Structure
/// code 6 appears in every one of the five layers, and the keep
/// door's elevation is the one place the two castle families disagree: 4 for a
/// stone keep and 1 for a wooden one.
#[test]
fn every_castle_has_exactly_one_keep_door_at_its_familys_height() {
    let s = sheets!();
    for level in 0..=4u8 {
        let f = castle::build(level, s.get(level).unwrap());
        let doors: Vec<_> =
            f.cells.iter().filter(|c| c.flags & FLAG_KEEP != 0).collect();
        assert_eq!(doors.len(), 1, "level {level}");
        assert_eq!(doors[0].elevation, if level > 1 { 4 } else { 1 }, "level {level}");
    }
}

/// **Something a catapult and a ram can open, on every castle** — but not the
/// same thing on each. Levels 0, 1 and 2 carry a curtain block (code 8, flag
/// `0x20`) and no drawbridge; level 3 carries a drawbridge and **no** code-8
/// block at all; level 4 carries both. [`siege::smash_walls`] opens either, so
/// "the besieger has something to break" holds at every level — which is the
/// assertion our stand-in's `wall > 0` was reaching for and getting wrong.
#[test]
fn every_castle_offers_the_besieger_something_to_open() {
    let s = sheets!();
    let mut shape = Vec::new();
    for level in 0..=4u8 {
        let f = castle::build(level, s.get(level).unwrap());
        let wall = f.cells.iter().filter(|c| c.flags & FLAG_WALL != 0).count();
        let bridge = f.cells.iter().filter(|c| c.flags & FLAG_DRAWBRIDGE != 0).count();
        assert!(wall + bridge > 0, "level {level}: nothing to open");
        shape.push((wall, bridge));
    }
    assert_eq!(shape, vec![(4, 0), (8, 0), (8, 0), (0, 4), (4, 4)]);
}

/// **The classifier gives every cell a surface**, and the two that drive
/// behaviour are both present: surface 4, the only value an order handler
/// searches for (`Siege_FindCellSurface4`), and surface 5, the one
/// `Wall_Collapse` bills a breach against.
#[test]
fn the_six_passes_leave_no_cell_unclassified() {
    let s = sheets!();
    for level in 0..=4u8 {
        let f = castle::build(level, s.get(level).unwrap());
        assert_eq!(f.cells.iter().filter(|c| c.surface == 0).count(), 0, "level {level}");
        assert!(
            f.cells.iter().any(|c| c.surface == SURFACE_RAMPART_WALK),
            "level {level}: no rampart walk"
        );
        assert!(
            f.cells.iter().any(|c| c.surface == SURFACE_BAILEY),
            "level {level}: no bailey"
        );
    }
}

