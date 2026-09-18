#![allow(unused_imports)]
use super::*;
use super::picture_comparison::*;
use super::banner_and_wall_rendering::*;
use super::*;
use super::tables_and_sheets::*;
use l2_game::battlefield::LiveBattle;
use l2_game::game::Assets;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::Game;
use l2_sim::runner::{Army, BattleRunner};
use l2_sim::siege::{code, frames_with_code, STRUCTURE_STONE, STRUCTURE_WOOD};
use l2_sim::terrain::{tileset, DIM};
use l2_sim::Troop;
use l2_view::scene::{self, Ground};
use l2_view::Canvas;

#[test]
fn the_cell_selector_moves_a_tile_between_the_two_sheets() {
    let dir = l2_testkit::install!();
    let read = |n: &str| {
        std::fs::read(dir.join(n)).map_err(|e| format!("{n}: {e}"))
    };
    let one = l2_view::sheet::Sheet::new(read("T32_stn1.pl8").expect("stn1")).expect("stn1");
    let two = l2_view::sheet::Sheet::new(read("T32_stn2.pl8").expect("stn2")).expect("stn2");

    let mut field = l2_sim::siege::our_castle(4);
    let cam = scene::Camera { x: 30, y: 20 };
    let cell = (cam.y + 5) * DIM + (cam.x + 5);
    let (px, py) = (scene::ORIGIN_X + 5 * scene::TILE, scene::ORIGIN_Y + 5 * scene::TILE);

    field.cells[cell].gfx = 4;
    field.cells[cell].elevation = 0;
    field.cells[cell].terrain = 0;

    field.cells[cell].flags2 = 0;
    let mut a = Canvas::screen();
    scene::draw_terrain(&mut a, &field, &one, Some(&two), cam);

    field.cells[cell].flags2 = tileset::SECOND;
    let mut b = Canvas::screen();
    scene::draw_terrain(&mut b, &field, &one, Some(&two), cam);

    let mut moved = 0;
    for y in 0..scene::TILE {
        for x in 0..scene::TILE {
            let (x, y) = ((px + x) as usize, (py + y) as usize);
            if a.at(x, y) != b.at(x, y) {
                moved += 1;
            }
        }
    }
    assert!(moved > 100, "only {moved} of 1024 pixels moved with the selector");
    assert_eq!(a.diff_count(&b), moved, "nothing outside that one cell changed");
}

#[test]
fn a_raised_cell_takes_a_second_blit_from_the_second_sheet() {
    let dir = l2_testkit::install!();
    let read = |n: &str| std::fs::read(dir.join(n)).expect("a sheet");
    let one = l2_view::sheet::Sheet::new(read("T32_stn1.pl8")).expect("stn1");
    let two = l2_view::sheet::Sheet::new(read("T32_stn2.pl8")).expect("stn2");

    let mut field = l2_sim::siege::our_castle(4);
    let cam = scene::Camera { x: 30, y: 20 };
    let cell = (cam.y + 6) * DIM + (cam.x + 6);
    field.cells[cell].flags2 = 0;
    field.cells[cell].gfx = 0xAC;
    field.cells[cell].terrain = 1;

    let mut flat = Canvas::screen();
    field.cells[cell].elevation = 0;
    scene::draw_terrain(&mut flat, &field, &one, Some(&two), cam);

    let mut raised = Canvas::screen();
    field.cells[cell].elevation = 2;
    scene::draw_terrain(&mut raised, &field, &one, Some(&two), cam);

    let moved = flat.diff_count(&raised);
    assert!(moved > 0, "the overlay painted nothing");
    assert!(
        moved <= (scene::TILE * scene::TILE) as usize,
        "{moved} pixels moved for a one-cell overlay"
    );
    assert!(two.frame(0x8C).is_some(), "T32_stn2.pl8 carries the overlay frame");
}

