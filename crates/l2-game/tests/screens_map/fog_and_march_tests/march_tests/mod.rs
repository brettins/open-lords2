#![allow(unused_imports)]

mod drawn;
pub use drawn::*;
mod orders;
pub use orders::*;

use super::*;
use super::fog_tests::*;
use super::*;
use super::view_tests::*;
use super::interaction_tests::*;
use super::structures_tests::*;
use common::*;
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::input::Key;
use l2_game::screen::Ctx;
use l2_game::screen::Machine;
use l2_game::screen::Screen;
use l2_game::screen::ScreenId;
use l2_game::screen::Transition;
use l2_game::screens::county::Panel;
use l2_game::screens::map;
use l2_game::screens::map::MapScreen;
use l2_game::shell::font;
use l2_game::Game;
use l2_view::campaign;
use l2_view::chrome;
use l2_view::Canvas;

// Every expected number below is a literal out of `Lords2.exe` — the walk
// tables at `0x004D8108`/`0x004D8188` and `0x004D8308`/`0x004D8388`,
// `g_unitWalkFrames`, `Map_DrawPathMarker`'s `(0x14, 6)` and `0x4E` — and none
// is computed from the constant it is checking.

fn ink_at(canvas: &Canvas, art: &l2_formats::pl8::DecodedFrame, at: (i32, i32)) -> bool {
    let (w, h) = (art.width as i32, art.height as i32);
    let mut any = false;
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            if !art.opaque[i] {
                continue;
            }
            let (px, py) = (at.0 + x, at.1 + y);
            if px < 0 || py < 0 || px >= canvas.width as i32 || py >= canvas.height as i32 {
                return false;
            }
            if canvas.at(px as usize, py as usize) != art.indices[i] {
                return false;
            }
            any = true;
        }
    }
    any
}

fn at_rest(screen: &MapScreen, tile: (u8, u8), nudge: (i32, i32), art: &l2_formats::pl8::DecodedFrame) -> (i32, i32) {
    let (row, col) = campaign::tile_to_cell(tile.0 as usize, tile.1 as usize);
    let (sx, sy) = campaign::cell_to_screen(screen.viewport(), screen.zoom(), row, col);
    let z = screen.zoom();
    (sx + z.half_pitch + nudge.0 - art.width as i32 / 2, sy + z.half_pitch + nudge.1 - art.height as i32)
}

fn open_step_east(game: &Game, ok: impl Fn(usize, usize) -> bool) -> Option<((u8, u8), (u8, u8))> {
    use l2_kingdom::map::{flags, index};
    let map = &game.kingdom.campaign.map;
    let busy = flags::IMPASSABLE | flags::ROAD | flags::PLOT | flags::FARMLAND | flags::CASTLE | flags::SETTLEMENT;
    let open = |x: u8, y: u8| {
        let t = index(x, y);
        map.county[t] != 0 && map.flags[t] & busy == 0
    };
    for y in 16..48u8 {
        for x in 16..47u8 {
            if !open(x, y) || !open(x + 1, y) || !ok(index(x, y), index(x + 1, y)) {
                continue;
            }
            let crowded = game.kingdom.campaign.units.iter().any(|(_, u)| {
                (u.y as i32 - y as i32).abs() <= 3 && (u.x as i32 - x as i32).abs() <= 4
            });
            if !crowded {
                return Some(((x, y), (x + 1, y)));
            }
        }
    }
    None
}

