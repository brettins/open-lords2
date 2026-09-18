#![allow(unused_imports)]

mod scenarios;
pub use scenarios::*;
mod simulation;
pub use simulation::*;

use super::*;
use super::helpers::*;
use super::food_and_ration::*;
use super::population_and_labour::*;
use super::simulation::*;
use l2_formats::save::Save;
use l2_kingdom::county::County;
use l2_kingdom::phase::SEASON_PIPELINE;
use l2_kingdom::tables::{health_band, Season, Tables, Weather};
use l2_scenario::{Scenario, STARTING_HEALTH_METER};

