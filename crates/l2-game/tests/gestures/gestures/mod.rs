#![allow(unused_imports)]

mod repeating_gestures;
pub use repeating_gestures::*;
mod button_kinds;
pub use button_kinds::*;
mod double_click;
pub use double_click::*;
mod corner_closing;
pub use corner_closing::*;

use super::*;

use l2_game::game::Assets;
use l2_game::input::{Event, Rect};
use l2_game::press::{self, Kind, Press, Widget};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::county::{self, Panel};
use l2_game::Game;

