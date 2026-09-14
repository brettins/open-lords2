//! Battlefield pathfinding.
//!
//! Reimplemented from `docs/battle.md` §8.3. Figures normally walk **straight at
//! their target** with no search at all; the pathfinder runs only when one is
//! blocked, and only if it has not already failed four times.
//!
//! The distinctive part
//! different paths: **terrain cost is charged twice over, by weighting *and* by
//! deferral.** The recorded cost is `cost[cur] + 1 + step_cost[neighbour]`, and
//! separately an expensive cell must be popped `step_cost` extra times before it
//! expands. Cheap ground therefore spreads first
//! accumulated cost, not hop count.
//!
//! An earlier revision of this file asserted the opposite — "deferral, not
//! weighting" — and weighted nothing. That was wrong. The correction comes from
//! the decompiled `Path_Search` at `0x0047095E`:
//!
//! ```text
//! sVar3    = cost[cur] + 1;
//! cost[nb] = stepCost[nb] + sVar3;
//! ```
//!
//! A neighbour is considered only while
//! `cost[neighbour] == 0`, so the first cost written to a cell stands even when a
//! cheaper route reaches it later. Reproduced
//! deliberately — the original's paths are the specification, and "fixing" this
//! changes where armies walk.
//!
//! On a `.skr` battlefield every step costs zero, because
//! `Battlefield_BuildFromSkr` never sets the flags `Path_BuildStepCost` looks
//! for. Field pathfinding is therefore a plain breadth-first search
//! weighting only ever bites inside castles.
//!
//! Integer arithmetic, fixed neighbour order, no iteration over a hash. Two
//! machines searching the same grid produce the same path.

mod grid;
pub use grid::*;
mod search_part;
pub use search_part::*;
mod tests_part;
pub use tests_part::*;

pub const DIM: usize = 80;
pub const CELLS: usize = DIM * DIM;

/// A **friendly figure** stands here (`Path_BuildBlockedMap`, `0x3E6`).
///
/// Not the same as impassable: if the
/// *destination* is occupied the original clears it to 0 and paths onto
/// it anyway, leaving the mover to swap or wait.
pub const OCCUPIED: u16 = 998;
/// Terrain that can never be entered (`Path_BuildTerrainTemplate`): flags `0x10`
/// or `0x40`, or the one-cell map border. A destination at this value aborts the
/// search before it starts.
pub const IMPASSABLE: u16 = 999;
/// The original's frontier queue holds this many entries and **wraps**
/// search that outgrows it silently overwrites its own queue. Reproduced rather
/// than fixed: a search that would have overrun produces the original's result.
pub const QUEUE_CAP: usize = 0x1900;
/// `Path_Extract` writes at most this many waypoints.
pub const MAX_WAYPOINTS: usize = 150;

/// Eight neighbours in a fixed order. Order decides tie-breaks, so it is part of
/// the specification, not an implementation detail.
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

/// The battlefield as the pathfinder sees it.
#[derive(Debug, Clone)]
pub struct Grid {
    /// Extra pops a cell must take before it expands, *and* the surcharge added
    /// to its recorded cost. **0 is ordinary ground** — the original's scale,
    /// where a `.skr` battlefield is zero everywhere.
    pub step_cost: Vec<u8>,
    pub elevation: Vec<u8>,
    /// Impassable terrain.
    pub blocked: Vec<bool>,
    /// A friendly figure is standing here. Blocks routing *through*, but not
    /// arrival at, the cell.
    pub occupied: Vec<bool>,
}

/// Why a search did not need to run, or did not succeed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The target is adjacent, or the straight line is clear. No search ran.
    NoSearchNeeded,
    Found,
    /// The flood fill exhausted its frontier without reaching the destination.
    Unreachable,
}

#[derive(Debug, Clone)]
pub struct Search {
    pub outcome: Outcome,
    /// Hop count per cell; 0 is unvisited, 1 is the start.
    pub cost: Vec<u16>,
}

/// How many of the 6,400 visit counters `Path_Search` clears.
///
/// **4,096, not 6,400 — and this is a bug in the original that we reproduce.**
///
/// `Path_Search` clears its counters with `FUN_004B3E51(0x004F6470, 0x1000)`,
/// and that helper counts **bytes**: its tail loop stores one byte and
/// decrements by one. `0x1000` is therefore 4,096 bytes against an array of
/// 6,400 `u8`, one per cell. Three things confirm the extent: the sibling call
/// passes `0x3200` for `g_pathCost`, which is exactly 6,400 × `u16`;
/// `docs/battle.md` records the counters as one byte per cell; and
/// `0x004F6470 + 6400` lands on the next global the same function
/// uses.
///
/// So cells 4,096 and above — **rows 51 to 79, the bottom 36% of the
/// battlefield** — begin each search holding whatever counts the *previous*
/// search left there. An expensive cell down there may expand immediately
/// because it is already "visited enough"
///
/// This only bites where step costs are non-zero: a `.skr`
/// field is uniformly zero-cost (see the module docs). It is reproduced rather
/// than fixed because the original's paths are the specification.
///
/// **It also makes pathfinding order-dependent**, which is a determinism
/// concern: the result of a search depends on which searches
/// ran before it. Lockstep peers run the same searches in the same order, so
/// they stay identical — but a caller that reorders searches changes the paths.
pub const CLEARED_COUNTERS: usize = 0x1000;

/// The scratch space `Path_Search` keeps between calls.
///
/// In the original these are globals, and that is not an implementation detail
/// to tidy away: the visit counters survive from one search to the next, and
/// only the first [`CLEARED_COUNTERS`] of them are reset. Modelling them as a
/// local would quietly fix the original's bug.
#[derive(Debug, Clone)]
pub struct Scratch {
    visits: Vec<u8>,
}

impl Scratch {
    pub fn new() -> Scratch {
        Scratch { visits: vec![0u8; CELLS] }
    }

    /// Clear exactly what the original clears, and no more.
    fn begin(&mut self) {
        for v in &mut self.visits[..CLEARED_COUNTERS] {
            *v = 0;
        }
    }

    /// The raw counters. Exposed because the carry-over above
    /// [`CLEARED_COUNTERS`] is a behaviour to be tested, not an accident to be
    /// hidden.
    pub fn visits(&self) -> &[u8] {
        &self.visits
    }
}

impl Default for Scratch {
    fn default() -> Self {
        Scratch::new()
    }
}

