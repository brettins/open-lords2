

#[macro_use]
#[path = "../common/mod.rs"]
mod common;

mod render;
pub use render::*;
mod interaction;
pub use interaction::*;
mod animation;
pub use animation::*;

use common::*;

use l2_game::input::Event;
use l2_game::input::Key;
use l2_game::screen::Ctx;
use l2_game::screen::Machine;
use l2_game::screen::ScreenId;
use l2_game::screen::Transition;
use l2_game::screens::map;
use l2_game::screens::village::{self as village_screen};
use l2_game::screens::village::VillageScreen;
use l2_view::chrome;
use l2_view::village;
use l2_view::Canvas;


