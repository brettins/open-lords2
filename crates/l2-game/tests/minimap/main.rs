//! **The minimap's four modes, counted in pixels.**
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" \
//! LORDS2_FIXTURES="E:\dev\lords2-fixtures" cargo test -p l2-game --test minimap
//! ```
//!
//! The claim these tests exist to settle is a visual one — *"the three
//! statistic modes recolour the minimap, and from a different table than the
//! ownership mode"* — so it is turned into a number the canvas can answer: **the
//! set of distinct palette indices drawn over the county land pixels of the
//! user's own `Map01.pl8`.**
//!
//! Sampling the *land* pixels is what
//! makes the sets exact. The panel artwork behind the minimap, and the sea, are
//! full of the same greys the realm ramp uses, so a naive rectangle sweep
//! reports colours nothing in this code path drew — it did, and the first
//! version of this file failed on `0x2F` and `0x32` coming out of `Misc_cty`
//! frame 54.
//!
//! Shade 10 is skipped for the same reason: it is the shade the selected county
//! has replaced with `0x20`, and leaving it in would put a colour in every set
//! that has nothing to do with the mode.
//!
//! `Minimap_DrawOverlay` (`0x00410CBD`) and `FUN_00451BBA` are the two functions
//! under test; `docs/screens.md` §3.2 describes them.

mod helpers;
pub use helpers::*;
mod overlays;
pub use overlays::*;
mod ui;
pub use ui::*;

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

