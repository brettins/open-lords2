#![allow(unused_imports)]

mod layout_tests;
pub use layout_tests::*;
mod interaction_tests;
pub use interaction_tests::*;
mod timing_tests;
pub use timing_tests::*;
mod sound_tests;
pub use sound_tests::*;
mod close_tests;
pub use close_tests::*;

use super::*;
use super::page_quirks::*;
use l2_game::game::{Assets, Prefs, PRESENTATION};
use l2_game::input::{Event, Key};
use l2_game::press;
use l2_game::screen::{Ctx, Machine, Screen, ScreenId, Transition};
use l2_game::screens::options::{self, OptionsScreen, Page, Setting};
use l2_game::Game;
use l2_kingdom::{Quirk, Quirks};

