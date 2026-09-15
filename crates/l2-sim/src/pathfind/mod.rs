//! An earlier revision of this file asserted the opposite — "deferral, not
//! weighting" — and weighted nothing. That was wrong. The correction comes from
//! the decompiled `Path_Search` at `0x0047095E`:

mod grid;
pub use grid::*;
mod search_part;
pub use search_part::*;
mod tests_part;
pub use tests_part::*;

pub const DIM: usize = 80;
pub const CELLS: usize = DIM * DIM;

pub const OCCUPIED: u16 = 998;
pub const IMPASSABLE: u16 = 999;
pub const QUEUE_CAP: usize = 0x1900;
pub const MAX_WAYPOINTS: usize = 150;

const NEIGHBOURS: [(i32, i32); 8] =
    [(0, -1), (-1, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (1, 1), (0, 1)];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pos {
    pub x: u8,
    pub y: u8,
}

impl Pos {
    pub fn new(x: u8, y: u8) -> Self {
        Pos { x, y }
    }
    fn index(self) -> usize {
        self.y as usize * DIM + self.x as usize
    }
}

#[derive(Debug, Clone)]
pub struct Grid {
    pub step_cost: Vec<u8>,
    pub elevation: Vec<u8>,
    pub blocked: Vec<bool>,
    pub occupied: Vec<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    NoSearchNeeded,
    Found,
    Unreachable,
}

#[derive(Debug, Clone)]
pub struct Search {
    pub outcome: Outcome,
    pub cost: Vec<u16>,
}

/// `Path_Search` clears its counters with `FUN_004B3E51(0x004F6470, 0x1000)`,
/// and that helper counts **bytes**: its tail loop stores one byte and
/// decrements by one. `0x1000` is therefore 4,096 bytes against an array of
/// 6,400 `u8`, one per cell. Three things confirm the extent: the sibling call
/// passes `0x3200` for `g_pathCost`, which is exactly 6,400 × `u16`;
/// `docs/battle.md` records the counters as one byte per cell; and
/// `0x004F6470 + 6400` lands on the next global the same function
/// uses.
pub const CLEARED_COUNTERS: usize = 0x1000;

#[derive(Debug, Clone)]
pub struct Scratch {
    visits: Vec<u8>,
}

impl Scratch {
    pub fn new() -> Scratch {
        Scratch { visits: vec![0u8; CELLS] }
    }

    fn begin(&mut self) {
        for v in &mut self.visits[..CLEARED_COUNTERS] {
            *v = 0;
        }
    }

    pub fn visits(&self) -> &[u8] {
        &self.visits
    }
}

impl Default for Scratch {
    fn default() -> Self {
        Scratch::new()
    }
}

