#![allow(unused_imports)]

mod menu_bar;
pub use menu_bar::*;
mod battle_events;
pub use battle_events::*;

use super::*;
use super::render::*;
use super::motion::*;
use super::panel::*;
use super::entities::*;
use l2_formats::{DecodedFrame, Palette};
use l2_game::battlefield::{self as bf, LiveBattle};
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::menubar;
use l2_game::Game;
use l2_sim::runner::{Army, BattleRunner};
use l2_sim::terrain::DIM;
use l2_sim::{Motion, Troop, SIDE_A, SIDE_B};
use l2_view::sheet::Sheet;
use l2_view::Canvas;

// third colour and no skip, and not one of the sixteen handlers reads
// `g_battlePhase`.

/// A string in one of the **original's** fonts, found by its set pixels. The
/// bar is drawn in `Fntl2_14.pl8`, which our own 5 × 7 probe cannot see.
fn find_font_text(
    canvas: &Canvas,
    font: &l2_game::shell::font::Font,
    s: &str,
    colour: u8,
) -> Option<(i32, i32)> {
    let style = l2_game::shell::font::Style { colour: 1, shadow: None, caps: None };
    let w = font.width(s).max(1);
    let h = font.height(s).max(1);
    let mut probe = Canvas::new(w as usize, h as usize);
    font.draw(&mut probe, 0, 0, s, &style);
    let mut wanted: Vec<(i32, i32)> = Vec::new();
    for y in 0..h {
        for x in 0..w {
            if probe.at(x as usize, y as usize) == 1 {
                wanted.push((x, y));
            }
        }
    }
    if wanted.is_empty() {
        return None;
    }
    for oy in 0..(canvas.height as i32 - h) {
        for ox in 0..(canvas.width as i32 - w) {
            let (fx, fy) = wanted[0];
            if canvas.at((ox + fx) as usize, (oy + fy) as usize) != colour {
                continue;
            }
            if wanted.iter().all(|(x, y)| canvas.at((ox + x) as usize, (oy + y) as usize) == colour)
            {
                return Some((ox, oy));
            }
        }
    }
    None
}

/// A battle with twenty swordsmen a side, staged the way the tests above do.
fn skirmish() -> (Game, Machine) {
    staged(30, &[(Troop::Swordsmen, 20)], &[(Troop::Swordsmen, 20)], |(x, y)| {
        (x as i32 - 7, y as i32 - 7)
    })
}

