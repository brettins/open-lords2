#![allow(unused_imports)]

mod castles;
pub use castles::*;
mod minimap_and_seasons;
pub use minimap_and_seasons::*;

use super::*;
use super::view_tests::*;
use super::interaction_tests::*;
use super::fog_and_march_tests::*;
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

/// A sprite's **exact ink**, found anywhere on the canvas.
///
/// The same idea as [`find_text`] and for the same reason: render the thing
/// Being looked for: keep the pixels it paints.
/// pattern. A PL8 blit copies only its opaque bytes, so a match is the frame's
/// own palette indices standing where the frame was blitted — several hundred
/// of them for a flag. That cannot arise from terrain.
///
/// Every place this frame's artwork stands, and how many opaque pixels had to
/// agree to make each one a match.
fn sprite_positions(
    canvas: &Canvas,
    frame: &l2_formats::pl8::DecodedFrame,
) -> (Vec<(i32, i32)>, usize) {
    let (w, h) = (frame.width as i32, frame.height as i32);
    let wanted: Vec<(i32, i32, u8)> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| frame.opaque[(y * w + x) as usize])
        .map(|(x, y)| (x, y, frame.indices[(y * w + x) as usize]))
        .collect();
    let mut found = Vec::new();
    if wanted.is_empty() {
        return (found, 0);
    }
    for oy in 0..=(canvas.height as i32 - h) {
        for ox in 0..=(canvas.width as i32 - w) {
            let (fx, fy, fi) = wanted[0];
            if canvas.at((ox + fx) as usize, (oy + fy) as usize) != fi {
                continue;
            }
            if wanted.iter().all(|&(x, y, i)| canvas.at((ox + x) as usize, (oy + y) as usize) == i)
            {
                found.push((ox, oy));
            }
        }
    }
    (found, wanted.len())
}

/// The first of them, scanning rows then columns.
fn find_sprite(
    canvas: &Canvas,
    frame: &l2_formats::pl8::DecodedFrame,
) -> Option<((i32, i32), usize)> {
    let (found, ink) = sprite_positions(canvas, frame);
    found.first().map(|&p| (p, ink))
}

/// The map centred on a county's town, drawn.
fn town_view(game: &mut Game, assets: &Assets, county: u8) -> (MapScreen, Canvas) {
    let mut screen = MapScreen::new();
    {
        let ctx = Ctx { game, assets };
        let town = MapScreen::town(&ctx, county);
        let &tile = town.first().expect("a county has a town");
        let (tx, ty) = l2_kingdom::map::coords(tile);
        screen.centre_on_tile(tx as usize, ty as usize);
    }
    let canvas = draw(&mut screen, game, assets);
    (screen, canvas)
}

