#![allow(unused_imports)]
use super::*;
use super::title_and_build::*;
use super::tile_panel::*;
use super::*;
use super::top_bar::*;
use super::county_and_units::*;
use super::png_part::*;
use std::path::PathBuf;
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Screen};
use l2_game::screens::map::MapScreen;
use l2_game::screens::setup::{SetupPage, SetupScreen};
use l2_game::shell::font::{self, Font, Style};
use l2_game::{scenario, Game};
use l2_kingdom::tables::Tables;
use l2_mods::Platform;
use l2_view::Canvas;

/// **`FUN_00409429` is not `Ui_DrawBox`, and the difference is a whole 16-pixel
/// row of the menu drop-down.**
///
/// A player reported the text *"clipping into the 'fold' of the scroll at the
/// top"* with *"a bit of empty space from the last bit of text to the bottom
/// 'fold'"*. The captions were never wrong: their pitch is `g_menuBarItems`'
/// own `y` column, 0, 20, 40, …, and `screens::menubar` carries it. The plate
/// was, because border **set 2** omits the top rail and fills its interior from
/// the box's own `y` — so the parchment reaches the menu bar and the first
/// caption at `y = 38` sits fourteen pixels into it, not two pixels into a rail.
///
/// The assertion is an **equality against the original's own second call**:
/// draw the drop-down plate, then draw `Ui_DrawBoxInterior(x + 0x10, y, cols -
/// 2, rows - 1)` on its own, and require the top row's middle band to be the
/// same pixels. No threshold, and nothing about what the artwork looks like.
///
/// Ablated by restoring `r > 0` to the interior test in
/// `l2_view::chrome::Chrome::draw_box`.
#[test]
fn the_drop_down_plate_has_no_top_rail() {
    let (mut game, assets) = world!();
    let Some(chrome) = assets.chrome.as_ref() else {
        l2_testkit::skip!("no Panels.pl8, so there is no box artwork to compare");
    };
    let _ = &mut game;

    // `FUN_0040C725`: `FUN_00409429(x, y + 0x12, 0x0C, rows)`, and the Help
    // menu's seven items make `(7 * 0x15) / 16 + 2` = 11 rows.
    const X: i32 = 40;
    const Y: i32 = 24;
    const COLS: i32 = 0x0C;
    const ROWS: i32 = 11;
    const CELL: i32 = l2_view::chrome::panels::CELL;

    let mut plate = Canvas::screen();
    chrome.draw_box(&mut plate, X, Y, COLS, ROWS, 2);

    // The second half of `FUN_00409429`, on its own, at the same place.
    let mut interior = Canvas::screen();
    for r in 0..(ROWS - 1) {
        for c in 0..(COLS - 2) {
            let frame = l2_view::chrome::panels::TEXTURE
                + (c as usize) % l2_view::chrome::panels::TEXTURE_DIM
                + ((r as usize) % l2_view::chrome::panels::TEXTURE_DIM)
                    * l2_view::chrome::panels::TEXTURE_DIM;
            chrome.draw_panel_frame(&mut interior, frame, X + CELL + c * CELL, Y + r * CELL);
        }
    }

    let mut differ = 0;
    for y in Y..Y + CELL {
        for x in (X + CELL)..(X + (COLS - 1) * CELL) {
            if plate.at(x as usize, y as usize) != interior.at(x as usize, y as usize) {
                differ += 1;
            }
        }
    }
    assert_eq!(
        differ,
        0,
        "the plate's top row is not the interior the original fills it with - \
         {differ} pixels of a {} x {CELL} band differ, which is the top rail we \
         should not be drawing",
        (COLS - 2) * CELL
    );

    // And the top corners are **edge** pieces, not corners: `frame = 0x1C` and
    // `0x28`, which is what the left and right edges use one row down.
    for (c, label) in [(0, "top-left"), (COLS - 1, "top-right")] {
        for y in 0..CELL {
            for x in 0..CELL {
                let (px, py) = (X + c * CELL + x, Y + y);
                assert_eq!(
                    plate.at(px as usize, py as usize),
                    plate.at(px as usize, (py + CELL) as usize),
                    "the {label} cell is not the edge piece the row below it uses"
                );
            }
        }
    }
}

// ------------------------------------------------------------- the title page

