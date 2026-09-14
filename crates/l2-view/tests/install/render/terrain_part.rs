#![allow(unused_imports)]
use super::*;
use super::battle_part::*;
use super::ui::*;
use super::sprites::*;
use super::*;
use super::corpus::*;
use super::oracle::*;
use std::{fs, path::{Path, PathBuf}};
use l2_sim::runner::{self as battle, BattleRunner};
use l2_sim::terrain;
use l2_sim::{Troop, SIDE_A, SIDE_B};
use l2_view::campaign;
use l2_view::canvas::Canvas;
use l2_view::chrome;
use l2_view::figures::{self, Anim, Colour};
use l2_view::scene::{self, BattleAssets, Camera};
use l2_view::sheet::Sheet;

/// Every graphic index a shipped battlefield asks for must exist in the
/// tileset. This is what would catch a wrong tileset, a wrong table, or a
/// wrong formula — all at once, over all twenty maps.
#[test]
fn every_shipped_battlefield_asks_only_for_tiles_that_exist() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(skr) = read(&dir, "USER.SKR") else {
        l2_testkit::skip!("USER.SKR not present - skipping");
    };
    let tiles = Sheet::new(read(&dir, scene::TILESET).expect(scene::TILESET)).unwrap();
    assert_eq!(tiles.frame_count(), 252, "T32_bat1.pl8 should hold 252 frames");

    let skr = l2_formats::Skr::parse(&skr).expect("USER.SKR parses");
    let mut cells = 0usize;
    for map in 0..skr.map_count() {
        let field = terrain::build(skr.terrain(map).unwrap(), 1);
        for c in &field.cells {
            assert!(
                tiles.frame(c.gfx as usize).is_some(),
                "map {map}: graphic {} is not a frame of the tileset",
                c.gfx
            );
            cells += 1;
        }
        // Both markers must have been found and expanded.
        assert_ne!(field.home_side0, (0, 0), "map {map} lost its 0x04 marker");
        assert_ne!(field.home_side4, (0, 0), "map {map} lost its 0x0F marker");
    }
    assert_eq!(cells, 20 * 6400);
    eprintln!("battlefields: {cells} cells across 20 maps, every tile present");
}

/// The first map of `USER.SKR` is the only one that is not the editor's blank
/// template: a river, two bridges and woodland. Drawing it must fill the
/// viewport completely — a hole would mean a missing or mis-indexed tile.
#[test]
fn the_sample_battlefield_renders_with_no_holes() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(skr) = read(&dir, "USER.SKR") else {
        l2_testkit::skip!("USER.SKR not present - skipping");
    };
    let tiles = Sheet::new(read(&dir, scene::TILESET).expect(scene::TILESET)).unwrap();
    let skr = l2_formats::Skr::parse(&skr).unwrap();
    let field = terrain::build(skr.terrain(0).unwrap(), 1);

    // Map 0 really does have water and woodland in it; if it did not, "no
    // holes" would be a much weaker claim.
    let water = field.cells.iter().filter(|c| c.terrain == terrain::id::WATER).count();
    let wood = field.cells.iter().filter(|c| c.terrain == terrain::id::WOODLAND).count();
    assert!(water > 100, "map 0 should be mostly river; found {water} water cells");
    assert!(wood > 0, "map 0 should have woodland");

    // Sweep the whole 80 x 80 map through the 15 x 14 viewport.
    let mut canvas = Canvas::screen();
    for cam_y in (0..terrain::DIM - scene::VIEW_ROWS).step_by(7) {
        for cam_x in (0..terrain::DIM - scene::VIEW_COLS).step_by(7) {
            canvas.clear(0);
            let cam = Camera { x: cam_x, y: cam_y };
            scene::draw_terrain(&mut canvas, &field, &tiles, None, cam);
            for row in 0..scene::VIEW_ROWS as i32 * scene::TILE {
                for col in 0..scene::VIEW_COLS as i32 * scene::TILE {
                    let y = (scene::ORIGIN_Y + row) as usize;
                    let x = (scene::ORIGIN_X + col) as usize;
                    assert_ne!(
                        canvas.at(x, y),
                        0,
                        "hole at screen ({x}, {y}) with the camera at ({cam_x}, {cam_y})"
                    );
                }
            }
        }
    }
    // Outside the viewport nothing was painted.
    assert_eq!(canvas.at(0, 0), 0, "the strip above the viewport should be untouched");
    assert_eq!(canvas.at(600, 300), 0, "the panel area should be untouched");
}

