#![allow(unused_imports)]

mod methods;
pub use methods::*;

use super::*;
use super::handlers::*;
use super::tests::*;
use crate::figure::{Figure, State};
use crate::troop::Troop;
use crate::unit::{chebyshev, pct_of, Units, MAX_UNITS, REFORM_INTERVAL};
use l2_net::Pcg32;

/// Everything a handler can see, borrowed for one dispatch.
pub struct World<'a> {
    pub units: &'a mut Units,
    pub figures: &'a mut [Figure],
    /// Where each figure stands, indexed by figure. `l2-sim` does not own
    /// positions; whoever does supplies them here.
    pub positions: &'a [(u8, u8)],
    pub field: &'a AiField,
    pub ai: &'a mut Ai,
}

// The `to_*` methods below take `&mut self`, which trips
// `wrong_self_convention`. They are named for the original's `Order_To*`
// routines — `Order_ToCell`, `Order_ToWallSlot`, `Order_ToCastleObjective` —
// and keeping that correspondence is worth more than the Rust naming idiom,
// because it is what lets a reader check each one against the binary.
// ---------------------------------------------------------------------------
// The seventeen handlers, plus the stub
// ---------------------------------------------------------------------------


