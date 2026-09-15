//! `Minimap_DrawOverlay` (`0x00410CBD`) and `FUN_00451BBA` are the two functions
//! under test; `docs/screens.md` §3.2 describes them.


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

fn install() -> Option<PathBuf> {
    l2_testkit::install_dir()
}

macro_rules! world {
    () => {{
        let Some(dir) = install() else {
            l2_testkit::skip!("no game install, so there is no minimap raster to draw");
        };
        let platform = Platform::builder().base(&dir).build().expect("the install mounts");
        let assets = Assets::load(&platform.vfs).expect("assets load");
        let save = l2_testkit::england!();
        let game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
        let raster = assets.minimap(game.map_slot).expect("the slot has a MAPnn.PL8");
        (game, assets, raster)
    }};
}

mod helpers;
pub use helpers::*;
mod overlays;
pub use overlays::*;
mod ui;
pub use ui::*;

