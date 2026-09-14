//! The six merchant trade routes, and how a merchant picks where to go next —
//! `docs/formats/plane4.md` §2.1 … §2.3.
//!
//! # Why this module exists at all
//!
//! `crate::unit` models a merchant's *record* and `crate::movement` walks it,
//! and between them there was still no answer to the only question a merchant
//! ever asks: **where next?** Phase 6 of the turn machine waits for every
//! type-3 unit to stop moving, and a merchant with no destination never starts,
//! so the phase settled instantly and the merchants stood still for ever. The
//! missing piece is a table — `g_merchantRoutes` (`0x00567970`), six rows of
//! sixteen county ids — and the cursor walk that reads it.
//!
//! # The table is in the save
//!
//! `g_merchantRoutes` falls inside a saved block, so it can be read straight
//! out of `england-turn1.sav`. It holds:
//!
//! ```text
//! route 0  14  4  7  8  2
//! route 1   5 12  6  3  8  2  1
//! route 2  14 11 13  5 10  9  7  3  1
//! route 3  11 12  6 10  4  9  3  1
//! route 4  14 11  5 13 12 10  2
//! route 5  13  6  4  9  7  8
//! ```
//!
//! and the same save's unit array holds exactly six units, slots **1 … 6**, all
//! type 3, all owner 6, standing in counties **14, 5, 13, 11, 12, 4** with
//! `needsDestination = 1` and `routeCursor = 1`. Three things that could each
//! have failed:
//!
//! * the start counties are the **first entry of each row**, which is what
//!   makes `plane4.md`'s claim that *"the route row is `unitIndex − 1`, not the
//!   stored route number"* checkable — and it holds, six for six. That
//!   document marked its England start-county list `[I]`, predicted from a
//!   simulation of `Merchant_PickStartCounties`; the file says **14, 5, 13, 11,
//! 12, 4** and the prediction was `14, 5, 13, 11, 12, 4`. **`[V]`** now.
//! * `nameIndex` (`+0x14F`) is `0, 1, 2, 3, 4, 5` — the route index, one per
//!   slot, as §3 says;
//! * **`moveAllowance` (`+0x154`) is stored as 0 in all six.** It is written by
//!   the tick handler every frame and never persisted, which is the direct
//!   evidence for `docs/armies.md` §2.1a's "tick-maintained" reading. A unit
//!   loaded from a save and never ticked has no allowance at all.
//!
//! # What is deliberately *not* here
//!
//! `Merchant_SpawnAll` and `Merchant_PickStartCounties` (`plane4.md` §2.1,
//! §2.2) run once, at new-game, off the map file's plane 4. This crate has no
//! loader and never learns what `L2_maps.dat` is, so the routes arrive as data
//! — [`MerchantRoutes`] is filled by whoever built the scenario. The odd dedup
//! walk and its two known bugs stay on the loader's side of that seam.

mod traversal;
pub use traversal::*;
mod placement;
pub use placement::*;
mod tests_part;
pub use tests_part::*;

use crate::county::{County, MAX_COUNTIES};
use crate::map::{flags, CampaignMap};
use crate::movement;
use crate::unit::{UnitKind, Units};

/// Six routes, because plane 4 on a castle tile is a route number **1 … 6**.
pub const ROUTES: usize = 6;

/// Sixteen counties a route, the width of one `g_merchantRoutes` row.
///
/// `Merchant_RouteAppend` appends to the first free cell of a row and gives up
/// silently when the row is full; **0 of the 44 shipped maps overflow it**
/// (`plane4.md` §1), so the cap is never reached in practice.
pub const ROUTE_SLOTS: usize = 16;

/// The radius the free-tile search grows to before it gives up.
/// `County_FindFreeRoadTile` (`0x00428007`) tries the `(2r+1)²` box around the
/// county's anchor for **r = 1, 2, 3**.
pub const FREE_TILE_RADIUS: u8 = 3;

/// `g_merchantRoutes` (`0x00567970`) — six rows of sixteen county ids, `0`
/// meaning "no more".
///
/// A fixed array of `Vec`s on purpose: `docs/netcode.md` §5
/// wants iteration whose order cannot depend on anything but an index, and this
/// is walked by index in two places.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MerchantRoutes {
    rows: [[u8; ROUTE_SLOTS]; ROUTES],
}

impl Default for MerchantRoutes {
    fn default() -> Self {
        MerchantRoutes::none()
    }
}

impl MerchantRoutes {
    /// No routes at all — what a map with no merchants on it looks like, and
    /// what a hand-built kingdom starts with.
    pub fn none() -> MerchantRoutes {
        MerchantRoutes { rows: [[0; ROUTE_SLOTS]; ROUTES] }
    }

    /// The whole row, including its trailing zeroes.
    pub fn row(&self, route: usize) -> &[u8; ROUTE_SLOTS] {
        &self.rows[route]
    }

    pub fn set_row(&mut self, route: usize, row: [u8; ROUTE_SLOTS]) {
        self.rows[route] = row;
    }

    /// Fill a row from a list of counties, in order, the way
    /// `Merchant_RouteAppend` builds it.
    pub fn set_route(&mut self, route: usize, counties: &[u8]) {
        let mut row = [0u8; ROUTE_SLOTS];
        for (slot, &c) in row.iter_mut().zip(counties.iter()) {
            *slot = c;
        }
        self.rows[route] = row;
    }

    /// How many counties a route names before its first zero.
    pub fn len(&self, route: usize) -> usize {
        self.rows[route].iter().position(|&c| c == 0).unwrap_or(ROUTE_SLOTS)
    }

    pub fn is_empty(&self, route: usize) -> bool {
        self.len(route) == 0
    }

    /// Whether any route names anything. A kingdom whose routes are all empty
    /// has no merchants to advance, and phase 6 is then a formality.
    pub fn any(&self) -> bool {
        self.rows.iter().any(|r| r[0] != 0)
    }
}

/// The `g_merchantRoutes` row a unit reads: **its slot number minus one**.
///
/// Not the unit's stored route index. See [`advance_all`].
pub fn route_row_for(id: usize) -> Option<usize> {
    id.checked_sub(1).filter(|&r| r < ROUTES)
}

