#![allow(unused_imports)]

mod drop_down_plate;
pub use drop_down_plate::*;
mod title_and_build;
pub use title_and_build::*;
mod tile_panel;
pub use tile_panel::*;

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

/// Whether our own 5 × 7 font's rendering of `s` in `colour` is anywhere on the
/// canvas.
///
/// **The first version of this asked the wrong question.** It looked for the
/// glyph pattern in *any* single colour, which matches every flat run of pixels
/// wide enough to hold it — so it reported the caption present on a canvas that
/// did not have it, on the strength of a patch of sea. A negative assertion is
/// only worth what its positive twin is, so this pins the colour the line it is
/// about
fn debug_font_absent(canvas: &Canvas, s: &str, colour: u8) -> bool {
    let w = l2_view::text::width(s).max(1);
    let h = l2_view::text::GLYPH_H;
    let mut probe = Canvas::new(w as usize, h as usize);
    l2_view::text::draw(&mut probe, 0, 0, s, 1);
    let wanted: Vec<(i32, i32)> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| probe.at(x as usize, y as usize) == 1)
        .collect();
    if wanted.is_empty() {
        return true;
    }
    for oy in 0..=(canvas.height as i32 - h) {
        'next: for ox in 0..=(canvas.width as i32 - w) {
            for &(x, y) in &wanted {
                if canvas.at((ox + x) as usize, (oy + y) as usize) != colour {
                    continue 'next;
                }
            }
            return false;
        }
    }
    true
}

// ------------------------------------------------- the menu drop-down's plate

