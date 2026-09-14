#![allow(unused_imports)]

mod world_tests;
pub use world_tests::*;
mod deal_tests;
pub use deal_tests::*;
mod validation_tests;
pub use validation_tests::*;

use super::*;
use super::diff::*;
use l2_formats::maps::{MapSet, Plane, PLANE_DIM, SLOT_LEN};
use l2_kingdom::map::MAP_TILES;
use l2_scenario::newgame::{self, MapError, NewGame};
use l2_scenario::{CountyState, Scenario};

