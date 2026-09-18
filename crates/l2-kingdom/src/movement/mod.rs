//! | | battlefield `Path_Search` `0x0047095E` | campaign `Move_FloodFill` `0x0046F700` |
//! |---|---|---|
//! | grid | 80 × 80 | 64 × 64 |
//! | how cost is charged | weighting **and** deferral: an expensive cell is re-queued `stepCost` times | weighting only — `dist[n] = dist[cur] + cost[n]` |
//! | relaxation | **none** — the first cost written to a cell stands | **full** — a cheaper later route rewrites and re-enqueues |
//! | blocked | sentinels 998 (friendly figure) and 999 (terrain) | cost **0**, and nothing else |
//! | occupancy | routes around friendly figures | **units are invisible to it** |
//! | early out | stops when the destination is reached | no goal test at all; always fills the component |
//! | diagonals | always | **forbidden out of a road tile** |
//!
//! # What was `[I]` in `docs/armies.md` §8 and is now read
//!
//! §8 said *"`Move_FloodFill` (2,115 bytes) was not read … that the fill itself
//! is a cost-weighted breadth-first search is `[I]`"*. It has been read. It is
//! **SPFA — a FIFO-queue Bellman–Ford with full relaxation** — not a
//! breadth-first search
//! relaxation branch a FIFO queue over non-uniform weights produces a field
//! that is not a shortest-cost field at all
//! [`extract_path`] would then be unsound. Everything below is `[D]` from
//! `0x0046F700` unless marked otherwise.

mod pathfinding;
pub use pathfinding::*;
mod stepper;
pub use stepper::*;
mod tests_part;
pub use tests_part::*;

use crate::county::{County, MAX_COUNTIES};
use crate::map::{coords, index, terrain, CampaignMap, CostMap, MAP_DIM, MAP_TILES};
use crate::realm::{Realm, MAX_REALMS};
use crate::unit::{UnitKind, Units, MAX_PATH};

pub const FILL_NEIGHBOURS: [(i32, i32); 8] =
    [(0, -1), (1, 0), (0, 1), (-1, 0), (1, -1), (1, 1), (-1, 1), (-1, -1)];

pub const ORTHOGONALS: usize = 4;

pub const STEP_DIRECTIONS: [(i32, i32); 8] = [
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
];

pub const QUEUE_CAP: usize = 1024;

pub const START_DISTANCE: i16 = 1;

pub const ROAD_PREFERENCE_COST: i16 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Routing {
    Direct,
    PreferRoads,
}

impl Routing {
    #[inline]
    fn adjust(self, cost: i16) -> i16 {
        match self {
            Routing::Direct => cost,
            Routing::PreferRoads if cost > 1 => ROAD_PREFERENCE_COST,
            Routing::PreferRoads => cost,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistanceField {
    dist: Vec<i16>,
    start: (u8, u8),
}

impl DistanceField {
    pub fn raw(&self, x: u8, y: u8) -> i16 {
        self.dist[index(x, y)]
    }

    pub fn cost_to(&self, x: u8, y: u8) -> Option<i32> {
        match self.dist[index(x, y)] {
            0 => None,
            d => Some(d as i32 - START_DISTANCE as i32),
        }
    }

    pub fn start(&self) -> (u8, u8) {
        self.start
    }

    pub fn reached(&self) -> impl Iterator<Item = ((u8, u8), i32)> + '_ {
        self.dist.iter().enumerate().filter_map(|(i, &d)| {
            (d != 0).then(|| (coords(i), d as i32 - START_DISTANCE as i32))
        })
    }
}

/// What [`try_enter`] makes of the tile a unit is about to step onto — the
/// return codes of `Unit_TryEnterTile` (`0x00466C3C`), named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    Open,
    Road,
    Field,
    Castle,
    Settlement,
    Plot,
    Occupied(usize),
}

impl Entry {
    pub fn ends_the_move(self) -> bool {
        !matches!(self, Entry::Open | Entry::Road | Entry::Field)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Offence {
    pub against: u8,
    pub by: u8,
    pub amount: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub entry: Entry,
    pub charged: i32,
    pub moved: bool,
    pub entered_county: Option<u8>,
    pub reached_castle: Option<u8>,
    /// **This is a different tile from [`Step::reached_castle`] and a different
    /// rule**
    /// county town* — `docs/decisions.md` C25
    /// name — and walking onto it is how a county is **taken**. `0x80` with a
    /// castle terrain is the castle itself.
    ///
    /// `Unit_ReachCastleBuilding` (`0x004686A0`) splits on ownership:
    pub reached_castle_building: Option<u8>,
    pub field_destroyed: Option<u8>,
    pub site_ruined: Option<(u8, usize)>,
    /// **A dwelling was burnt down**, and this is the county that lost a
    /// quarter of its people. `Unit_BurnDwelling` (`0x00468AE2`).
    pub dwelling_burnt: Option<u8>,
    pub offence: Option<Offence>,
}

impl Step {
    fn nothing(entry: Entry) -> Step {
        Step {
            entry,
            charged: 0,
            moved: false,
            entered_county: None,
            reached_castle: None,
            reached_castle_building: None,
            field_destroyed: None,
            site_ruined: None,
            dwelling_burnt: None,
            offence: None,
        }
    }
}

pub const FIELD_TRAMPLE_OFFENCE: i32 = 10;

/// What burning a dwelling costs — `Unit_BurnDwelling` (`0x00468AE2`) passes
/// `'\x14'` = 20, and unlike the trample it does so whoever the burner is.
///
/// One difference, visible only in a save with a corrupt `g_movingUnit`: the
/// original passes `g_units[g_movingUnit].owner` as the offender
/// the unit it was handed. The mover sets `g_movingUnit` to that unit before
/// the call, so the two are the same value on every path. `[D]`
pub const DWELLING_BURN_OFFENCE: i32 = 20;

pub const RUIN_LADDER: [(u8, u8, usize); 4] = [
    (0, 3, 1),   // iron
    (4, 6, 3),   // stone
    (7, 9, 2),   // weapons
    (10, 12, 0), // wood
];

