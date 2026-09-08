//! Deterministic battle simulation for Lords of the Realm II.
//!
//! Reimplemented from `docs/battle.md`, which reads the model out of the
//! original binary. This crate is our own code implementing documented
//! behaviour; nothing is copied from the original.
//!
//! # Determinism
//!
//! Lockstep networking requires two machines running the same commands to reach
//! bit-identical state (`docs/netcode.md`). Three rules follow, and this crate
//! holds to all three:
//!
//! * **No floating point anywhere.** Integer arithmetic only.
//! * **No iteration in hash order.** Figures live in a `Vec` and are always
//!   walked by index.
//! * **No dependence on addresses or allocation.** Nothing branches on a
//!   pointer, and no `HashMap` ordering escapes into a decision.
//!
//! There is deliberately no random number generator here yet. Nothing in the
//! melee model needs one, and a PRNG is exactly the thing that must be frozen
//! in-tree rather than pulled from a crate whose stream may change in a minor
//! version.

pub mod battle;
pub mod figure;
pub mod melee;
pub mod missile;
pub mod movement;
pub mod pathfind;
pub mod troop;

pub use battle::Battle;
pub use figure::{Figure, Role, Side, State, SIDE_A, SIDE_B};
pub use missile::{MissileStats, WeaponClass};
pub use movement::{move_delay, ticks_per_cell, CellEntry, Progress};
pub use pathfind::{Grid, Outcome, Pos};
pub use troop::{Troop, TroopStats, TroopTable, ALL_TROOPS};

/// The array bound the original allocates nothing beyond. An army that would
/// produce more figures than this is silently truncated, and the shipped data
/// sits right against the ceiling — see `docs/battle.md` §5.4.
pub const MAX_FIGURES: usize = 80;
