#![allow(unused_imports)]

mod moves;
pub use moves::*;
mod targets;
pub use targets::*;
mod surfaces;
pub use surfaces::*;

use super::*;

use super::*;
use super::handlers::*;
use super::tests::*;
use crate::figure::{Figure, State};
use crate::troop::Troop;
use crate::unit::{chebyshev, pct_of, Units, MAX_UNITS, REFORM_INTERVAL};
use l2_net::Pcg32;

