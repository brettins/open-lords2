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
//! There is **one** random number drawn in the whole battle model, and it is
//! the AI's: the strength advantage carries a −10…+21 jitter, re-rolled every
//! 101 frames ([`ai`]). It comes from `l2_net::Pcg32`, the generator frozen
//! in-tree, carried in the simulation state and advanced only by simulation
//! code — never from a system source, and never from a crate whose stream may
//! change in a minor version. Nothing in the melee, missile, movement or
//! pathfinding model touches it.
//!
//! # Where positions live
//!
//! Not here. A [`Figure`] has men, hits and a recovery counter and no
//! coordinates, because none of the rules this crate implements need them. The
//! two places that genuinely do — the pathfinder and the AI — take positions as
//! *arguments*: [`pathfind`] takes a [`Grid`], and [`ai`] takes a slice of
//! per-figure positions. Whoever owns the battlefield supplies them.
//!
//! # Data in, no loaders
//!
//! Two tables carry everything tunable: [`TroopTable`] for the combat constants
//! and [`ai::AiField`] for the battlefield's positions. Both are plain values
//! this crate already understands; building one out of a mod document is
//! `l2-mods`'s job. A simulation that can load its own rules is a simulation
//! that can fail to load, and two lockstep peers that fail differently desync.

pub mod ai;
pub mod battle;
pub mod figure;
pub mod melee;
pub mod missile;
pub mod movement;
pub mod pathfind;
pub mod troop;
pub mod unit;

pub use ai::{Ai, AiField, Action, World};
pub use battle::Battle;
pub use figure::{Figure, Role, Side, State, SIDE_A, SIDE_B};
pub use missile::{MissileStats, WeaponClass};
pub use movement::{move_delay, ticks_per_cell, CellEntry, Progress};
pub use pathfind::{Grid, Outcome, Pos};
pub use troop::{Troop, TroopStats, TroopTable, ALL_TROOPS};
pub use unit::{BattleUnit, Units, MAX_UNITS};

/// The array bound the original allocates nothing beyond. An army that would
/// produce more figures than this is silently truncated, and the shipped data
/// sits right against the ceiling — see `docs/battle.md` §5.4.
pub const MAX_FIGURES: usize = 80;
