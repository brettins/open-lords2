#![allow(unused_imports)]

mod operations;
pub use operations::*;
mod targeting;
pub use targeting::*;
mod movement;
pub use movement::*;

use super::*;
use super::wants::*;
use super::muster::*;
use super::raid::*;
use super::aim::*;
use crate::county::{County, MAX_COUNTIES};
use crate::kingdom::Kingdom;
use crate::levy::{self, LevyBasket, Muster};
use crate::map::{flags, CampaignMap, MAP_DIM};
use crate::math::pct;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use crate::unit::{TroopType, UnitKind, Units};


#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ArmyReport {
    pub garrisons_raised: Vec<usize>,
    pub frontier_raised: Vec<usize>,
    pub evacuations: Vec<Evacuation>,
}

/// `FUN_00437535` starts the battle itself. This crate reports it, for the
/// same reason [`crate::ai::taunt`] returns its letters
/// them: a battle is not this crate's to start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Eviction {
    pub unit: usize,
    pub county: u8,
    pub besieger: Option<usize>,
    pub destroyed: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MoveReport {
    pub marching: Vec<usize>,
    pub evictions: Vec<Eviction>,
    pub disbanded: Vec<usize>,
}

