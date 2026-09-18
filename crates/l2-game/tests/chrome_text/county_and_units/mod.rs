#![allow(unused_imports)]

mod garrison_and_mercenaries;
pub use garrison_and_mercenaries::*;
mod unit_panels;
pub use unit_panels::*;
mod zoom_and_battle;
pub use zoom_and_battle::*;
mod foraging_and_shoot;
pub use foraging_and_shoot::*;

use super::*;
use super::top_bar::*;
use super::panels::*;
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

const LEAD: i32 = 4;
const SPACE: i32 = 4;
const TRAILER: i32 = 4;

fn own_county(game: &Game) -> u8 {
    (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the local player holds a county")
}

fn find_on_row_from(
    canvas: &Canvas,
    f: &Font,
    s: &str,
    colour: u8,
    y: i32,
    from: i32,
) -> Option<i32> {
    let mut probe = Canvas::new(canvas.width - from as usize, canvas.height);
    for py in 0..canvas.height {
        for px in from as usize..canvas.width {
            probe.set(px - from as usize, py, canvas.at(px, py));
        }
    }
    find_on_row(&probe, f, s, colour, y).map(|x| x + from)
}

