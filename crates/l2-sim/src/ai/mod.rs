//! The battle AI: what an *unordered* unit does.
//!
//! Reimplemented from `docs/battle-ai.md`, which reads the whole of it out of
//! `Lords2.exe`. `Battle_UpdateAllUnits` (`0x00489401`) dispatches one order
//! handler per unit unless that unit's owner is human; there are **25 dispatch
//! slots holding 18 distinct functions, one of which is an empty `return`
//! filling seven slots**, so **17 real handlers**. All 18 are here.
//!
//! # The headline
//!
//! The whole AI turns on one number.
//!
//! ```text
//! strength_advantage = (weighted AI men * 100 / weighted human men) - 100  ± jitter
//! unit.orders  (+0x1A)  = how many times this unit has thought
//! unit.last_attacker    = the unit that last hit me, remembered for 50 frames
//! ```
//!
//! Every handler is a variation on one skeleton, and three consequences fall
//! straight out of it:
//!
//! * **a unit decides once every 200 frames** (100 for two of the siege
//!   defenders) — nothing else in the battle is that slow;
//! * **a unit in melee stops thinking**: 13 of the 17 refuse to run while any
//!   of their figures is fighting, so once the line makes contact most of the
//!   AI stops manoeuvring;
//! * **aggression is global, not per unit and not per side.** One word,
//!   recomputed every 101 frames from the whole battlefield, read by every
//!   handler that has a mood at all.
//!
//! # Two things that read wrong until you know them
//!
//! `orders` (`+0x1A`) is a **script program counter**, not an order: nothing a
//! player clicks ever writes it, and the handlers read it as `orders < 10`,
//! `orders % 5 == 0`. And `re targ` (`+0x14`) does **not** re-target — at zero
//! the unit reforms *its own figures* onto formation slots.
//!
//! # Where the numbers come from, and where they don't
//!
//! `AGGRESSION_THRESHOLD` = 5 and `SORTIE_THRESHOLD` = 260 are **[V]**: both
//! are `MOV imm32` writes in `Rules_InitConstants`, recovered statically by
//! `tools/oracle/initconsts.ps1` *and* read out of a live process
//! (`docs/decisions.md` C15, C16).
//!
//! Everything positional — castle approach points, wall slots, rally waypoints,
//! the castle objective — lives in [`AiField`], which this crate takes as plain
//! data and never loads. That is deliberate twice over. It is the same seam
//! [`crate::TroopTable`] uses, so a ruleset or a battlefield builder supplies
//! the numbers and the simulation stays unable to read a file. And it is
//! honest: those tables are filled by `Battlefield_BuildCastle`, which has
//! **not** been decompiled, so their *contents* are not ours to assert.
//! `docs/battle-ai.md` §9 says so; the seam keeps our ignorance visible rather
//! than inventing a castle.
//!
//! # Determinism
//!
//! The strength advantage carries a **−10…+21 jitter**, and with the threshold
//! at 5 that jitter alone decides whether the field AI attacks in any battle
//! within about thirty points of even. It therefore has to be part of the
//! lockstep state: it comes from [`Pcg32`], the generator frozen in-tree
//! (`docs/netcode.md` D-3), advanced only by simulation code and carried in
//! [`Ai`] like any other field. **No system source, no clock, no thread-local.**
//!
//! The original draws it from `Rand_Advance`, a pair of 31-bit Fibonacci LFSRs.
//! We reproduce the *distribution and the cadence*, not the stream: replicating
//! their seeding would buy nothing, since our battles are not their battles,
//! and a second generator is a second value stream to freeze forever.
//!
//! Beyond that: integer arithmetic only, every sweep by ascending index, no
//! iteration in hash order, nothing branching on an address. The handler tables
//! are `const` arrays of `fn` pointers indexed by category — the *index*
//! decides, never the pointer value.

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

/// The battlefield is 80 × 80 cells. `docs/formats/skr.md`, and `battle.md` §3.
pub const DIM: usize = 80;
pub const CELLS: usize = DIM * DIM;

/// `g_aiAggressionThreshold` (`0x0057C8B4`) = **5**. **[V]**
///
/// Every field handler attacks above this. Read by the three field handlers and
/// by nothing else.
pub const AGGRESSION_THRESHOLD: i32 = 5;

/// `g_aiSortieThreshold` (`0x00552FF0`) = **260**. **[V]**
///
/// A garrison sallies out only when it believes it is 3.6× the attacker's
/// strength, which is close to never.
pub const SORTIE_THRESHOLD: i32 = 260;

/// Frames between one unit's decisions, in 15 of the 17 handlers. **[D]**
pub const THINK_INTERVAL: i16 = 200;

/// The two exceptions: `SiegeDefFoot` and `SiegeDefMelee` think twice as often.
pub const THINK_INTERVAL_FAST: i16 = 100;

/// Frames between recomputations of the strength advantage. **[D]**
///
/// `Battle_UpdateAllUnits` counts up and fires at `> 100`, which is every 101st
/// frame, not every 100th.
pub const ADVANTAGE_INTERVAL: i32 = 101;

/// `UnitOrder_SiegeAttFoot` abandons its script and charges at or above this.
/// **[D]**
pub const SIEGE_CHARGE_ADVANTAGE: i32 = 151;

/// The strength weighting `Battle_UpdateStrengthAdvantage` applies per troop
/// type, indexed by [`Troop::index`]. **[D]**
///
/// A knight counts four men and a peasant one. Siege engines fall off the end
/// of the ladder and count 1, which is what the missing `else` in the original
/// does — reproduced.
pub const STRENGTH_WEIGHT: [i32; 11] = [1, 2, 3, 3, 2, 2, 4, 1, 1, 1, 1];

