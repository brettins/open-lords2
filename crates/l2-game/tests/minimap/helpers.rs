#![allow(unused_imports)]
use super::*;
use super::overlays::*;
use super::ui::*;
use std::collections::BTreeSet;
use std::path::PathBuf;
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::scenario;
use l2_game::screen::{Ctx, Screen};
use l2_game::screens::map::{MapScreen, MINIMAP_MODE_BUTTONS};
use l2_game::Game;
use l2_kingdom::county::LABOUR_NO_FLOOR;
use l2_kingdom::tables::Tables;
use l2_kingdom::tables::{JOB_COUNT, JOB_GRAIN_FARMING, JOB_IDLE_TOWNSFOLK};
use l2_mods::Platform;
use l2_view::chrome::{
    self, Minimap, MinimapMode, MINIMAP_DIM, MINIMAP_RATING_RAMP, MINIMAP_REALM_RAMP,
    MINIMAP_SELECTED, MINIMAP_X, MINIMAP_Y,
};
use l2_view::Canvas;

fn draw(screen: &mut MapScreen, game: &mut Game, assets: &Assets) -> Canvas {
    let mut canvas = Canvas::screen();
    let ctx = Ctx { game, assets };
    screen.draw(&ctx, &mut canvas);
    canvas
}

/// Click one of the four buttons in the strip beside the minimap.
fn click(screen: &mut MapScreen, game: &mut Game, assets: &Assets, button: usize) {
    let r = MINIMAP_MODE_BUTTONS[button];
    let mut ctx = Ctx { game, assets };
    screen.handle(Event::Click { x: r.x + 2, y: r.y + 2 }, &mut ctx);
}

/// The distinct colours drawn over the land pixels of the counties in `want` —
/// shades 11..=13 only, so the selected county's `0x20` never enters.
fn land_colours(canvas: &Canvas, m: &Minimap, want: &BTreeSet<u8>) -> BTreeSet<u8> {
    let mut seen = BTreeSet::new();
    for y in 0..MINIMAP_DIM {
        for x in 0..MINIMAP_DIM {
            let i = (y * MINIMAP_DIM + x) as usize;
            if !(11..=13).contains(&m.shades[i]) || !want.contains(&m.counties[i]) {
                continue;
            }
            seen.insert(canvas.at((MINIMAP_X + x) as usize, (MINIMAP_Y + y) as usize));
        }
    }
    seen
}

/// The counties in the raster whose `band` — one of the three ratings — is `b`.
fn with_band(
    game: &Game,
    m: &Minimap,
    band: fn(l2_kingdom::county::MinimapBands) -> u8,
    b: u8,
) -> BTreeSet<u8> {
    counties_in_raster(m)
        .into_iter()
        .filter(|&c| band(game.kingdom.counties[c as usize].minimap_bands()) == b)
        .collect()
}

/// Every county id that has land pixels in this raster.
fn counties_in_raster(m: &Minimap) -> Vec<u8> {
    let mut ids: Vec<u8> = m
        .counties
        .iter()
        .zip(&m.shades)
        .filter(|(_, &s)| (11..=13).contains(&s))
        .map(|(&c, _)| c)
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids.retain(|&c| c != 0);
    ids
}

/// Give the local player every county, and put each one in a known state so a
/// band is reached on purpose.
fn hand_the_player_everything(game: &mut Game) -> Vec<usize> {
    let ids: Vec<usize> = (1..=game.kingdom.county_count as usize).collect();
    for &id in &ids {
        let player = game.player;
        let c = &mut game.kingdom.counties[id];
        c.owner = player;
        // Fed, and neither short of workers nor carrying any slack, so every
        // rating but the one a test sets is the "draw nothing" band 6.
        c.ration_achieved = 3;
        c.ration_wanted = 3;
        c.labour = [0; JOB_COUNT];
        c.labour_wanted = [LABOUR_NO_FLOOR; JOB_COUNT];
        c.labour_useful = [0; JOB_COUNT];
    }
    ids
}

