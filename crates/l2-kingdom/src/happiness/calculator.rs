#![allow(unused_imports)]
use super::*;
use super::update_part::*;
use super::actions::*;
use crate::county::County;
use crate::math::clamp;
use crate::tables::Tables;
use l2_net::{Quirk, Quirks};

/// The happiness a county holds steady at, given its three terms. Zero means
/// the county neither rises nor falls.
///
/// This is not a rule of its own — it is the sum [`update`] adds — but it is.
/// the number every player-facing statement about the game is really about, so
/// it is worth being able to ask for directly.
pub fn steady_state(t: &Tables, tax_rate: i32, health_band: u8, ration_level: i32) -> i32 {
    (crate::tax::FREE_TAX_RATE - tax_rate)
        + crate::health::happiness(t, health_band)
        + t.ration_happiness(ration_level)
}

