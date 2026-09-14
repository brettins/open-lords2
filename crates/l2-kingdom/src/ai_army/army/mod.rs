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

// ---------------------------------------------------------------------------
// The passes, over a whole kingdom
// ---------------------------------------------------------------------------

/// What one turn of the AI's army handling did, for a caller that wants to
/// show it or a test that wants to assert on it.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ArmyReport {
    /// Armies raised to fill a castle garrison — step 7's second pass.
    pub garrisons_raised: Vec<usize>,
    /// Armies raised on a threatened frontier county — step 7's third pass.
    pub frontier_raised: Vec<usize>,
    /// Counties written off, and what the pass wanted to carry out of them.
    pub evacuations: Vec<Evacuation>,
}

/// A garrison [`Mission::GARRISON`] turned out of a castle whose county has
/// changed hands, and — where it had one — the besieger that was waiting for
/// it.
///
/// `FUN_00437535` starts the battle itself. This crate reports it, for the
/// same reason [`crate::ai::taunt`] returns its letters
/// them: a battle is not this crate's to start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Eviction {
    pub unit: usize,
    pub county: u8,
    /// The besieger's slot, if the garrison was under siege when it was
    /// turned out.
    pub besieger: Option<usize>,
    /// True when there was nowhere to put the garrison and it was destroyed.
    pub destroyed: bool,
}

/// What [`Kingdom::run_ai_move_armies`] did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MoveReport {
    /// The armies that were given a fresh path this step.
    pub marching: Vec<usize>,
    /// Garrisons turned out of castles in counties the realm has lost.
    pub evictions: Vec<Eviction>,
    /// Armies disbanded because [`Mission::JOIN_GARRISON`] found no castle
    /// with room for them.
    pub disbanded: Vec<usize>,
}

