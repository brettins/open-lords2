#![allow(unused_imports)]

mod labels;
pub use labels::*;
mod layout;
pub use layout::*;

use super::*;
use super::strip_and_sidebar::*;
use super::panels::*;
use super::drawing_and_emboss::*;
use super::produce_and_pastures::*;
use common::*;
use l2_game::game::Assets;
use l2_game::game::MAX_TAX_RATE;
use l2_game::input::Event;
use l2_game::input::Key;
use l2_game::screen::Ctx;
use l2_game::screen::Machine;
use l2_game::screen::Screen;
use l2_game::screen::ScreenId;
use l2_game::screen::Transition;
use l2_game::screens::county::{self as county};
use l2_game::screens::county::CountyScreen;
use l2_game::screens::county::Panel;
use l2_game::screens::map;
use l2_game::screens::map::MapScreen;
use l2_game::screens::village::{self as village_screen};
use l2_game::screens::village::VillageScreen;
use l2_game::shell::font;
use l2_game::Game;
use l2_view::campaign;
use l2_view::Canvas;

/// Whether the string found at `at` carries `Ui_DrawText`'s **drop shadow**:
/// every pixel one right and one down of a glyph pixel that is not itself a
/// Glyph pixel is `0x3F`. Typed here.
/// `font::DROP_SHADOW_COLOUR`, so ablating the constant cannot move the probe.
fn is_dropped(canvas: &Canvas, f: &font::Font, s: &str, at: (i32, i32)) -> bool {
    const SHADOW: u8 = 0x3F;
    let (w, h) = (f.width(s).max(1), f.height(s).max(1));
    let mut probe = Canvas::new(w as usize + 1, h as usize + 1);
    f.draw(&mut probe, 0, 0, s, &font::Style { colour: 1, shadow: None, caps: None });
    let ink = |x: i32, y: i32| {
        x < probe.width as i32 && y < probe.height as i32 && probe.at(x as usize, y as usize) == 1
    };
    let mut checked = 0;
    for y in 0..h {
        for x in 0..w {
            if ink(x, y) && !ink(x + 1, y + 1) {
                checked += 1;
                if canvas.at((at.0 + x + 1) as usize, (at.1 + y + 1) as usize) != SHADOW {
                    return false;
                }
            }
        }
    }
    checked > 0
}

/// Pixels of `colour` inside a box — the "renders something" half of a string
/// that [`find_font_text`] could otherwise only find or not find.
fn ink_in(canvas: &Canvas, x: i32, y: i32, w: i32, h: i32, colour: u8) -> usize {
    (y..y + h)
        .flat_map(|py| (x..x + w).map(move |px| (px, py)))
        .filter(|&(px, py)| canvas.at(px as usize, py as usize) == colour)
        .count()
}

