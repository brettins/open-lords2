#![allow(unused_imports)]
use super::*;
use super::rack::*;
use super::animation::*;
use super::screenshots::*;
use std::path::PathBuf;
use l2_game::game::Assets;
use l2_game::input::{Event, Rect};
use l2_game::scenario;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::armoury;
use l2_game::Game;
use l2_kingdom::tables::{Tables, WEAPON_TYPE_COUNT};
use l2_view::Canvas;

/// **`arm_grid.pl8` names the six weapon types and nothing else usable.**
///
/// The shipped file holds 26 cells outside 1…6 — column 0 down the left edge
/// and an eleven-cell sliver at y 216 — and in the original every one of them
/// is a live hotspot that indexes the eight-slot levy basket with 60-something.
/// `docs/bugs.md` N13. We answer `None`, and this is where that is asserted
/// against the file.
#[test]
fn the_hit_map_names_six_weapons_and_the_stray_cells_are_refused() {
    let (_g, assets) = world!();
    assert!(assets.shell.has_armoury_grid(), "arm_grid.pl8 is in the install");

    let mut seen: Vec<u8> = Vec::new();
    for y in (0..480).step_by(8) {
        for x in (0..640).step_by(8) {
            if let Some(t) = assets.shell.armoury_grid(x, y) {
                if !seen.contains(&t) {
                    seen.push(t);
                }
            }
        }
    }
    seen.sort_unstable();
    assert_eq!(seen, vec![1, 2, 3, 4, 5, 6], "one region per weapon type, and no seventh");

    // The two known stray patches, named by position so that a different file
// fails loudly.
    assert_eq!(assets.shell.armoury_grid(0, 0), None, "the left-edge strip is not a rack");
    assert_eq!(assets.shell.armoury_grid(0, 112), None, "nor its last row");
    assert_eq!(assets.shell.armoury_grid(560, 216), None, "nor the sliver at y 216");
}

/// **Every rack's hit region sits on the weapon it opens.**
///
/// `g_armouryWallItems` says where each weapon is painted and `arm_grid.pl8`
/// says where it can be clicked; the two are different tables written by
/// different people, and if they had drifted the player would click a bow and
/// get a pike. The sprite's own width and height come out of the `.pl8`, so
/// this compares the picture with the hit map and nothing with itself.
#[test]
fn every_weapon_can_be_clicked_where_it_hangs() {
    let (_g, assets) = world!();
    let sheet = assets.shell.sheet(armoury::items_sheet(1)).expect("arm_it_r.pl8");

    for (slot, &(frame_index, x, y)) in armoury::WALL.iter().enumerate() {
        let weapon = slot as u8 + 1;
        let f = sheet.frame(frame_index).expect("a wall frame");
        let sprite = Rect::new(x, y, f.width as i32, f.height as i32);
        let region = grid_box(&assets, weapon).unwrap_or_else(|| panic!("weapon {weapon} has no cells"));

        // The grid's box and the sprite's box must be the same thing to within
        // a cell of rounding: same centre, and neither more than 16 pixels
        // adrift on any edge.
        for (name, a, b) in [
            ("left", region.x, sprite.x),
            ("top", region.y, sprite.y),
            ("right", region.x + region.w, sprite.x + sprite.w),
            ("bottom", region.y + region.h, sprite.y + sprite.h),
        ] {
            assert!(
                (a - b).abs() <= 16,
                "weapon {weapon}: the hit map's {name} edge is {a} and the sprite's is {b}",
            );
        }

        // And the hotspot rectangle — the fallback for a missing grid — covers
// the rack sprite at the bottom of the screen.
        let h = armoury::RACK_HOTSPOTS.iter().find(|h| h.4 == weapon).expect("a hotspot");
        assert!(h.1 >= 396, "weapon {weapon}'s fallback rectangle is not on the bottom row");
    }
}

