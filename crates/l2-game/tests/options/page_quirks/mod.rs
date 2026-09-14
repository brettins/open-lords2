#![allow(unused_imports)]

mod quirk_tests;
pub use quirk_tests::*;

use super::*;
use super::widget_tests::*;
use l2_game::game::{Assets, Prefs, PRESENTATION};
use l2_game::input::{Event, Key};
use l2_game::press;
use l2_game::screen::{Ctx, Machine, Screen, ScreenId, Transition};
use l2_game::screens::options::{self, OptionsScreen, Page, Setting};
use l2_game::Game;
use l2_kingdom::{Quirk, Quirks};

