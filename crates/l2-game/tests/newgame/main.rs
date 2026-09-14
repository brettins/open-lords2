//! **Pick Ireland, and play Ireland.**
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-game --test newgame
//! ```
//!
//! The setup screen has been able to *name* a map since its list was drawn.
//! What it could not do was start one: whatever the list said, the world came
//! out of `lastturn.sav` and it was England. This file is the check on the
//! other half — that the slot the list highlights is the world the campaign
//! screen opens on, and that a turn runs in it.
//!
//! **Nothing here loads a save.** Every game starts from `Game::new`, which is
//! an empty world, so a test that passed by inheriting the fixture's England
//! would have nothing to inherit.
//!
//! Everything is driven through [`Screen::handle`] with real pointer
//! coordinates read out of the geometry tables, for the reason
//! `tests/setup.rs` gives: a test that calls a method the interface does not
//! reach proves nothing about the interface.

mod start_game;
pub use start_game::*;
mod campaign;
pub use campaign::*;
mod heraldry;
pub use heraldry::*;
mod army_size;
pub use army_size::*;

use std::path::PathBuf;

use l2_formats::maps::MapSet;
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::scenario;
use l2_game::screen::{Ctx, Screen, ScreenId, Transition};
use l2_game::screens::setup::{
    SetupPage, SetupScreen, CUSTOM_BUTTONS, CUSTOM_BUTTON_Y, MAP_LIST_ROW, MAP_LIST_ROWS,
    MAP_LIST_X, MAP_LIST_Y,
};
use l2_game::{turn, Game};
use l2_kingdom::realm::MAX_REALMS;

/// `L2.eng` group 101's first five names, which are the first five slots.
/// Ireland is slot 2, and it is on the list's first page, so choosing it is one
/// click.
const IRELAND: usize = 2;
const SCOTLAND: usize = 1;
const ENGLAND: usize = 0;

fn install() -> Option<PathBuf> {
    l2_testkit::install_dir()
}

macro_rules! assets {
    () => {{
        let Some(dir) = install() else {
            l2_testkit::skip!("no game install, so no L2_maps.dat and no L2.eng");
        };
        let platform =
            l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
        Assets::load(&platform.vfs).expect("assets load")
    }};
}

fn click(screen: &mut SetupScreen, game: &mut Game, assets: &Assets, x: i32, y: i32) -> Transition {
    let mut ctx = Ctx { game, assets };
    screen.handle(Event::Click { x, y }, &mut ctx)
}

/// Click row `row` of the map list, at the coordinates `FUN_00433905`'s hit
/// test uses.
fn pick_map(screen: &mut SetupScreen, game: &mut Game, assets: &Assets, row: usize) {
    assert!(row < MAP_LIST_ROWS);
    let y = MAP_LIST_Y + row as i32 * MAP_LIST_ROW + MAP_LIST_ROW / 2;
    click(screen, game, assets, MAP_LIST_X + 20, y);
}

/// *Start* — the second of page 7's three captions.
fn press_start(screen: &mut SetupScreen, game: &mut Game, assets: &Assets) -> Transition {
    click(screen, game, assets, CUSTOM_BUTTONS[1].0 + 20, CUSTOM_BUTTON_Y)
}

/// Open the custom page and let its first tick read the map, as the machine
/// does.
fn open(assets: &Assets, game: &mut Game) -> SetupScreen {
    let mut screen = SetupScreen::new(SetupPage::Custom);
    let mut ctx = Ctx { game, assets };
    screen.update(&mut ctx);
    screen
}

/// How many counties a slot has, straight out of the file — a second reading
/// of the number the world builder produces.
fn counties_in(assets: &Assets, slot: usize) -> usize {
    assets.slot(slot).expect("the slot").county_count()
}

// ---------------------------------------------------------------- the headline

