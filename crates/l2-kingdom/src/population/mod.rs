//! Population and migration — `docs/kingdom.md` §5,
//! `Population_UpdateAll` (`0x00449EF3`) and `Migration_UpdateAll`
//! (`0x0044A6BA`).
//!
//! ```text
//! popLast = pop;
//! cap     = Pct(pop, 20);
//! base    = Table_Lookup(pop, g_birthRateLadder, 20, 1);
//! death   = g_deathRateByHealth[healthBand] + g_deathRateBySeason[g_season];
//! factor  = happiness < 26 ? 25 : happiness < 51 ? 50
//!         : happiness < 76 ? 75 : happiness < 100 ? 100 : 120;
//! rate    = Pct(base, factor);             /* local_c: the SCALED rate */
//! births  = Pct(pop, rate);
//! deaths  = Pct(pop, death);
//! if (births == 0 && rate  != 0) births = 1;
//! if (deaths == 0 && death != 0) deaths = 1;
//! if (healthBand == 0)   deaths += 2;
//! if (rate < death)      deaths += 1; else births += 1;
//! swing   = pct < 0 ? Pct(deaths, -pct) + 10 : pct > 0 ? Pct(births, pct) + 10 : 0;
//! if (swing > cap) swing = cap;               /* county +0x2F8, the letter's figure */
//! if (pct < 0) deaths += swing; else if (pct > 0) births += swing;
//! pop += births - deaths;
//! if (pop < 1) { births = 0; deaths = pop; pop = 0; }
//! pop -= emigrants; pop += immigrants;
//! ```
//!
//! This is the pass `docs/kingdom.md` §9 reproduces twice each for births and
//! deaths, on two different happiness bands, from the shipped save. It is the
//! strongest evidence in that document and the reason most of §4 and §5 is
//! marked **`[V]`**.

mod migration;
pub use migration::*;
mod update;
pub use update::*;
mod tests_part;
pub use tests_part::*;

use crate::county::{ChangeReason, County, CHANGE_REASON_MIN_PCT, MAX_INFLOW_SOURCES};
use crate::math::pct;
use crate::tables::{Season, Tables};
use l2_net::{Quirk, Quirks};

pub const MIGRATION_CAP: i32 = 100;

/// `Pct(deaths or births, |pct|) + 10`. `[V]` — `Population_UpdateAll`
/// (`0x00449EF3`) adds the literal `10` in both arms, and nowhere else.
pub const EVENT_SWING_FLOOR: i32 = 10;

