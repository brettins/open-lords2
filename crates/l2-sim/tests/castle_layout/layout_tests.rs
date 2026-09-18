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

/// [`l2_sim::siege::DOCK_WALL_ELEVATION`] is 2 exactly — `FUN_00491492` tests
/// `elevation == 2` on the cell two ahead of a siege tower — and
/// `Oil_FindPourTarget` refuses to look below 2. [`siege::our_castle`] puts its
/// wall at [`siege::WALL_ELEVATION`], **one**, so both of those are dead
/// against it whatever the rules say.
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

#[test]
fn only_the_two_largest_castles_carry_a_drawbridge() {
    let s = sheets!();
    for level in 0..=4u8 {
        let f = castle::build(level, s.get(level).unwrap());
        let bridge = f.cells.iter().filter(|c| c.flags & FLAG_DRAWBRIDGE != 0).count();
        assert_eq!(bridge, if level >= 3 { 4 } else { 0 }, "level {level}");
    }
}

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

