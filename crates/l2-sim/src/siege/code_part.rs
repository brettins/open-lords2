#![allow(unused_imports)]
use super::*;

use castle::*;
use damage::*;
use drawbridge::*;
use moat::*;
use siegetower::*;
use crate::terrain::{Battlefield, Cell, DIM};

/// The structure codes [`STRUCTURE_STONE`]'s first byte carries above 4, and
/// what `Battlefield_BuildCastle` turns each into. **[V]** — its ladder, in
/// order.
pub mod code {
    pub const RAISED_3: u8 = 5;
    pub const KEEP: u8 = 6;
    pub const SEVEN: u8 = 7;
    pub const WALL: u8 = 8;
    pub const DRAWBRIDGE: u8 = 9;
    pub const TEN: u8 = 10;
    pub const RAISED_1: u8 = 11;
    pub const RAISED_2: u8 = 12;
}

