//! Reimplemented from `docs/battle-ai.md`, which reads the whole of it out of
//! `Lords2.exe`. `Battle_UpdateAllUnits` (`0x00489401`) dispatches one order
//! handler per unit unless that unit's owner is human; there are **25 dispatch
//! slots holding 18 distinct functions, one of which is an empty `return`
//! filling seven slots**, so **17 real handlers**. All 18 are here.
//!
//! ```text
//! strength_advantage = (weighted AI men * 100 / weighted human men) - 100  ± jitter
//! unit.orders  (+0x1A)  = how many times this unit has thought
//! unit.last_attacker    = the unit that last hit me, remembered for 50 frames
//! ```
//!
//! `orders` (`+0x1A`) is a **script program counter**, not an order: nothing a
//! player clicks ever writes it, and the handlers read it as `orders < 10`,
//! `orders % 5 == 0`. And `re targ` (`+0x14`) does **not** re-target — at zero
//! the unit reforms *its own figures* onto formation slots.
//!
//! `AGGRESSION_THRESHOLD` = 5 and `SORTIE_THRESHOLD` = 260 are **[V]**: both
//! are `MOV imm32` writes in `Rules_InitConstants`, recovered statically by
//! `tools/oracle/initconsts.ps1` *and* read out of a live process
//! (`docs/decisions.md` C15, C16).

mod types;
pub use types::*;
mod handlers_part;
pub use handlers_part::*;

mod world;
pub use world::*;
mod handlers;
pub use handlers::*;
mod tests;
pub use tests::*;

use crate::figure::{Figure, State};
use crate::troop::Troop;
use crate::unit::{chebyshev, pct_of, Units, MAX_UNITS, REFORM_INTERVAL};
use l2_net::Pcg32;

pub const DIM: usize = 80;
pub const CELLS: usize = DIM * DIM;

/// `g_aiAggressionThreshold` (`0x0057C8B4`) = **5**. **[V]**
pub const AGGRESSION_THRESHOLD: i32 = 5;

/// `g_aiSortieThreshold` (`0x00552FF0`) = **260**. **[V]**
pub const SORTIE_THRESHOLD: i32 = 260;

/// Frames between one unit's decisions, in 15 of the 17 handlers. **[D]**
pub const THINK_INTERVAL: i16 = 200;

pub const THINK_INTERVAL_FAST: i16 = 100;

/// Frames between recomputations of the strength advantage. **[D]**
pub const ADVANTAGE_INTERVAL: i32 = 101;

/// **[D]**
pub const SIEGE_CHARGE_ADVANTAGE: i32 = 151;

/// The strength weighting `Battle_UpdateStrengthAdvantage` applies per troop
/// type, indexed by [`Troop::index`]. **[D]**
pub const STRENGTH_WEIGHT: [i32; 11] = [1, 2, 3, 3, 2, 2, 4, 1, 1, 1, 1];

