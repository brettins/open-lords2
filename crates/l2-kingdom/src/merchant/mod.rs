//! `crate::unit` models a merchant's *record* and `crate::movement` walks it,
//! and between them there was still no answer to the only question a merchant
//! ever asks: **where next?** Phase 6 of the turn machine waits for every
//! type-3 unit to stop moving, and a merchant with no destination never starts,
//! so the phase settled instantly and the merchants stood still for ever. The
//! missing piece is a table — `g_merchantRoutes` (`0x00567970`), six rows of
//! sixteen county ids — and the cursor walk that reads it.
//!
//! * the start counties are the **first entry of each row**, which is what
//!   makes `plane4.md`'s claim that *"the route row is `unitIndex − 1`, not the
//!   stored route number"* checkable — and it holds, six for six. That
//!   document marked its England start-county list `[I]`, predicted from a
//!   simulation of `Merchant_PickStartCounties`; the file says **14, 5, 13, 11,
//! 12, 4** and the prediction was `14, 5, 13, 11, 12, 4`. **`[V]`** now.
//!
//! * `nameIndex` (`+0x14F`) is `0, 1, 2, 3, 4, 5` — the route index, one per
//!   slot, as §3 says;
//! * **`moveAllowance` (`+0x154`) is stored as 0 in all six.** It is written by
//!   the tick handler every frame and never persisted, which is the direct
//!   evidence for `docs/armies.md` §2.1a's "tick-maintained" reading. A unit
//!   loaded from a save and never ticked has no allowance at all.

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

pub const ROUTES: usize = 6;

pub const ROUTE_SLOTS: usize = 16;

/// `County_FindFreeRoadTile` (`0x00428007`) tries the `(2r+1)²` box around the
/// county's anchor for **r = 1, 2, 3**.
pub const FREE_TILE_RADIUS: u8 = 3;

/// `g_merchantRoutes` (`0x00567970`) — six rows of sixteen county ids, `0`
/// meaning "no more".
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
    pub fn none() -> MerchantRoutes {
        MerchantRoutes { rows: [[0; ROUTE_SLOTS]; ROUTES] }
    }

    pub fn row(&self, route: usize) -> &[u8; ROUTE_SLOTS] {
        &self.rows[route]
    }

    pub fn set_row(&mut self, route: usize, row: [u8; ROUTE_SLOTS]) {
        self.rows[route] = row;
    }

    pub fn set_route(&mut self, route: usize, counties: &[u8]) {
        let mut row = [0u8; ROUTE_SLOTS];
        for (slot, &c) in row.iter_mut().zip(counties.iter()) {
            *slot = c;
        }
        self.rows[route] = row;
    }

    pub fn len(&self, route: usize) -> usize {
        self.rows[route].iter().position(|&c| c == 0).unwrap_or(ROUTE_SLOTS)
    }

    pub fn is_empty(&self, route: usize) -> bool {
        self.len(route) == 0
    }

    pub fn any(&self) -> bool {
        self.rows.iter().any(|r| r[0] != 0)
    }
}

pub fn route_row_for(id: usize) -> Option<usize> {
    id.checked_sub(1).filter(|&r| r < ROUTES)
}

