
mod fertility;
pub use fertility::*;
mod reclaim;
pub use reclaim::*;
mod grain;
pub use grain::*;
mod herd;
pub use herd::*;

use crate::county::{County, MAX_FIELDS};
use crate::field::FieldType;
use crate::math::{clamp, pct, pct_of, per_myriad};
use crate::tables::{HerdCrowdingRow, Season, Tables, Weather, HERD_CROWDING_COUNT};
use l2_net::{Quirk, Quirks};


/// The fertility scale's bounds. `L2.eng` group 22 names seven levels from
/// *"Infertile — almost no production"* to *"Excellent fertility — bumper
/// crop!"*, but `docs/kingdom.md` §7.2 could not trace the mapping from this
/// scalar to those seven, so this crate does not invent one.
pub const FERTILITY_MIN: i32 = -100;
pub const FERTILITY_MAX: i32 = 100;

pub const FERTILITY_PER_FALLOW: i32 = 6;
pub const FERTILITY_PER_GRAIN: i32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Factor(pub i32, pub i32);

impl Factor {
    pub const NONE: Factor = Factor(1, 1);

    pub fn apply(self, value: i32) -> i32 {
        ((value as i64 * self.0 as i64) / self.1 as i64) as i32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HerdGrowth {
    pub births: i32,
    pub deaths: i32,
}

impl HerdGrowth {
    pub fn net(self) -> i32 {
        self.births - self.deaths
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrainEstimate {
    /// `+0xC8` — the wanted floor, or [`crate::county::LABOUR_NO_FLOOR`].
    pub wanted: i32,
    /// `+0xCC` — the useful ceiling, the only one `Labour_Allocate` reads.
    pub useful: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HerdEstimate {
    /// `+0xD4` — the break-even staffing, below which the count is drawn red.
    pub wanted: i32,
    /// `+0xD8` — the growth-maximising staffing, the only one
    /// `Labour_Allocate` reads.
    pub useful: i32,
}

#[cfg(test)]
mod tests;

