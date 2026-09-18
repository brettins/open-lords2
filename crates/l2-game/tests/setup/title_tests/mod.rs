#![allow(unused_imports)]

mod build_stamp_tests;
pub use build_stamp_tests::*;
mod clock_tests;
pub use clock_tests::*;

use super::*;
use super::setup_tests::*;
use std::path::PathBuf;
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::scenario;
use l2_game::screen::{Ctx, Screen, ScreenId, Transition};
use l2_game::screens::setup::{
    SetupPage, SetupScreen, MAP_LIST_ROW, MAP_LIST_ROWS, MAP_LIST_X, MAP_LIST_Y, OPTION_CELLS,
    OPTION_LIST,
};
use l2_game::setup::{self, option, SetupOptions, COUNTY_STATUS, STARTING_GOLD, START_ARMOURY};
use l2_game::Game;
use l2_game::shell::{font, Pen};
use l2_view::Canvas;
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::Tables;
use l2_mods::Platform;

