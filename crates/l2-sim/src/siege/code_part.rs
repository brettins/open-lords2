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
    /// `flags = 4; elevation = 3`.
    pub const RAISED_3: u8 = 5;
    /// The keep's door: `surface = 6`, `flags = 8`, `flags2 |= 0x80`, and
    /// `elevation` **1 for a wooden castle and 4 for a stone one**.
    pub const KEEP: u8 = 6;
    /// `surface = 7`, `elevation = 2`.
    pub const SEVEN: u8 = 7;
    /// The curtain wall: `surface = 8`, `flags = 0x20 | 4`, `elevation = 1`.
    pub const WALL: u8 = 8;
    /// The drawbridge: `surface = 0x0B`, `flags = 0x40`, `elevation = 0`.
    /// Stone castles only.
    pub const DRAWBRIDGE: u8 = 9;
    /// `surface = 0x0E`, `flags |= 4`, `elevation = 0`.
    pub const TEN: u8 = 10;
    /// `flags = 4`, `elevation = 1` — the wall walk's height with no wall flag.
    pub const RAISED_1: u8 = 11;
    /// `flags = 4`, `elevation = 2`.
    pub const RAISED_2: u8 = 12;
}

