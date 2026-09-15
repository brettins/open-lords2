#![allow(unused_imports)]

mod fog_tests;
pub use fog_tests::*;
mod march_tests;
pub use march_tests::*;

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

fn paint_at(game: &mut Game, assets: &Assets, x: u8, y: u8) -> (MapScreen, Canvas) {
    let mut screen = MapScreen::new();
    draw(&mut screen, game, assets);
    screen.centre_on_tile(x as usize, y as usize);
    let canvas = draw(&mut screen, game, assets);
    (screen, canvas)
}

fn map_pixels_differ(a: &Canvas, b: &Canvas, clip: l2_view::Clip) -> usize {
    a.pixels
        .iter()
        .zip(b.pixels.iter())
        .enumerate()
        .filter(|&(i, (p, q))| {
            let (x, y) = ((i % a.width) as i32, (i / a.width) as i32);
            p != q && clip.contains(x, y)
        })
        .count()
}

