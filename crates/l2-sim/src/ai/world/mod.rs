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

pub struct World<'a> {
    pub units: &'a mut Units,
    pub figures: &'a mut [Figure],
    pub positions: &'a [(u8, u8)],
    pub field: &'a AiField,
    pub ai: &'a mut Ai,
}



