//! A battle that runs: units, figures, positions, orders and the tick loop.
//!
//! The rest of this crate is rules. [`melee`](crate::melee),
//! [`missile`](crate::missile), [`movement`](crate::movement),
//! [`pathfind`](crate::pathfind), [`unit`](crate::unit) and [`ai`](crate::ai)
//! each answer one question and none of them owns a coordinate. This module is
//! the composition of all of them into a battle — the thing the original's
//! `Battle_Update` is — and it is where positions live.
//!
//! It used to live in the renderer, which is how it came to be the one part of
//! the simulation sitting behind `winit` and `pixels`. `docs/plan-review.md` §5
//! is the argument for the move; `docs/netcode.md` D-3 is the reason it matters.
//!
//! # One frame, in the original's order
//!
//! ```text
//! BattleUnits_RebuildFromFigures   units mirror their figures; grudges age
//! Battle_UpdateAllUnits            recentre, dispatch one order handler per
//!                                  AI unit, count the reform timer down
//! BattleUnit_Reform                units whose destination moved, and units
//!                                  whose 500-frame timer expired, re-issue
//!                                  every figure a destination
//! Battle_UpdateAllMen              `targeted` decays; each figure moves,
//!                                  swings or stands
//! melee::tick                      damage, casualties, death
//! ```
//!
//! Order matters and is the original's: the strength advantage is recomputed
//! before any unit thinks, a unit is recentred on its figures before its handler
//! runs, and the reform countdown runs **outside** the human-control guard
//! player's units tidy their formation too (`docs/battle-ai.md` §5).
//!
//! # Determinism
//!
//! Integer arithmetic only, every sweep by ascending index, no hashing anywhere
//! a decision is made, nothing read from the clock. The one random draw in the
//! whole model is the AI's strength jitter, and it comes from the [`Ai`]'s own
//! `Pcg32` — carried in this struct like any other state.
//! `two_runs_of_the_same_battle_stay_identical` holds the property locally and
//! `tests/lockstep.rs` holds it across a socket.
//!
//! # What is faithful and what is not
//!
//! * Unit sizes, footprints, formation geometry and the deployment slot per
//!   unit: [`crate::formation`]. **[V]** from `g_troopBattleStats`.
//! * The seventeen order handlers, the 200-frame think, the 101-frame strength
//!   advantage: [`crate::ai`]. **[D]**
//! * Movement timing, pathfinding, melee: the modules named above.
//! * **Missiles fly.** A standing armed figure runs [`Self::fire_tick`], which
//!   is `BattleMan_FireMissile`; the arrow it looses is a real object stepping a
//!   Bresenham line four sub-steps a tick, and it hits whoever is standing in
//!   the cell it enters. [`crate::missile`]
//!   is the model and its header is the evidence. What is **not** modelled:
//!   the state that decides *when* a figure is in the original's firing state — here it is "standing at its destination and out
//!   of melee", which is `[I]`.
//! * **Fire, oil and the siege tower** are [`crate::fire`]'s and
//!   [`crate::siege`]'s rules, run from here in the original's places: a man
//!   burns at the head of his own frame (`Battle_UpdateAllMen`), a fire goes out
//!   and a stream of oil paints its cross between a missile's step and its
//!   countdown (`Missile_UpdateAll`), a wood spreads after the sweep
//!   (`FUN_004859E5`), and a tower docks when its step is refused
//!   (`BattleMan_Step`).
//! * **A figure already locked in a melee is not re-tasked by a reform.** The
//!   original re-tasks it and lets its next tick tear the duel down; we do not
//!   model that teardown, so pulling one side out here would leave a
//!   half-broken duel that is *less* like the original than leaving it alone.
//! * The original's figure states 10 (siege engine moving) and 12 (catapult
//!   aiming) are not modelled; those figures walk.

mod fighter;
pub use fighter::*;
mod muster;
pub use muster::*;
mod conclusion;
pub use conclusion::*;
mod simulation;
pub use simulation::*;
mod orders;
pub use orders::*;
mod combat;
pub use combat::*;
mod movement;
pub use movement::*;
mod tests;
pub use tests::*;

use crate::ai::{self, Ai, AiField};
use crate::facing::{facing_from_delta, FACING_DELTA};
use crate::fire;
use crate::formation::{self, FOOTPRINT, MAX_FIGURES_PER_UNIT, ROW_MAX, TYPE_PRIORITY, WEAPON_CLASS};
use crate::missile::{self, WeaponClass};
use crate::movement::Progress;
use crate::pathfind::{self, Grid, Outcome, Pos};
use crate::terrain::{self, Battlefield, DIM};
use crate::unit::{chebyshev, Units, CATEGORY_OF_TROOP, MAX_UNITS};
use crate::{Battle, Motion, Side, State, Troop, SIDE_A, SIDE_B};

/// One drawn man: a [`crate::Figure`] plus everywhere it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fighter {
    /// Index into [`BattleRunner::sim`]'s figures. Stable for the whole battle.
    pub sim: usize,
    pub troop: Troop,
    pub side: Side,
    pub x: u8,
    pub y: u8,
    /// Where this figure is walking to — the original's `tg x` / `tg y`, figure
    /// record `+0x24`/`+0x26`, which is what the developers' own debug panel
    /// calls them.
    ///
    /// **Corrected.** This comment used to say `+0x144`/`+0x146`, "written by
    /// `Formation_SendFigure` and by nothing else". Both halves were wrong and
    /// nothing tested either. No instruction in `Lords2.exe` references
    /// `+0x144` or `+0x146`; both fall inside the 300-byte path array at
    /// `+0x38`, which runs to `+0x164` (`docs/records.json`). And eleven
    /// functions write `tg x` / `tg y`, not one: `BattleMan_Create`,
    /// `BattleMan_Step`, `BattleMan_StateChase`, `BattleMan_StateEngineFire`,
    /// `BattleMan_StateCloseToAttack`, `Formation_SendFigure` and five
    /// unnamed functions in `0x00492000`. `BattleMan_StateChase` in particular
    /// rewrites it *every tick* from the chased figure's position, which is
    /// the behaviour "written by nothing else" would have ruled out.
    pub target: (u8, u8),
    pub facing: u8,
    /// Sub-cell progress, 0 … 16 in twos, from [`crate::movement`].
    pub progress: Progress,
    pub anim: Motion,
    /// The figure's own animation counter. Seeded per figure so that identical
    /// men do not march in lockstep — the original seeds `+0x0E` the same way.
    pub phase: u8,
    /// Waypoints from the pathfinder, consumed from the end.
    pub path: Vec<Pos>,
    /// Consecutive pathfinding failures. At four the figure stops trying —
    /// the original's `barred`, `+0x176`.
    pub barred: u8,
    /// Ticks to wait before asking the pathfinder again — `hold it`, `+0x165`.
    pub hold: u8,
    /// How many times this figure has been re-routed — `routed`, `+0x166`.
    /// Not morale; see `docs/battle.md` §8.3.
    pub reroutes: u16,
    /// **The moat cell this figure is shovelling into** — figure record
    /// `+0x190`, with `+0x18F` folded into the `Option`.
    ///
    /// The original latches it inside `Cell_TryEnter`: a figure in state 9
    /// whose step is refused *because the destination is water* keeps the
    /// destination and starts tipping. Ours latches it in
    /// [`BattleRunner::fill_moat_tick`] for the same reason and at the same
    /// moment — the step it cannot take.
    pub moat_cell: Option<u32>,
    /// Figure record `+0x18E` — frames since the last load went in, against
    /// [`crate::siege::MOAT_TICKS_PER_LOAD_HUMAN`] or its AI twin.
    pub moat_load: u8,
    /// Figure record `+0x168`, the debug panel's **`polar dirc`** — an
    /// orthogonal facing, 0, 2, 4 or 6.
    ///
    /// Two writers matter here. A **siege tower** walking keeps it on the
    /// orthogonal nearest its `dirc`, with hysteresis on the diagonals
    /// (`FUN_00488436`, [`crate::siege::tower_polar`]), and `FUN_00491492`
    /// starts its search for a wall to dock with from it. A **pot of oil** has
    /// it set to the axis it pours along (`FUN_0047A814`). Zero, as the zeroed
    /// record has it, until one of them writes.
    pub polar: u8,
    /// **How long this body has lain here** — figure record `+0x173`, counted
    /// by the corpse state's own tick.
    ///
    /// `docs/battle.md` §14.2: **state 2 is a corpse**. Its handler steps the
    /// collapse animation and counts `+0x173` to [`CORPSE_FRAMES`] before
    /// freeing the slot; **state 15 is its siege-engine twin** at
    /// [`ENGINE_CORPSE_FRAMES`]. Ours does not free the slot — the index is a
    /// key here and is not in the original — so the count is what says the body
    /// is gone, and [`BattleRunner::corpse_gone`] is what the renderer asks.
    pub corpse: u16,
}

/// State 2's count — `docs/battle.md` §14.2.
pub const CORPSE_FRAMES: u16 = 80;
/// State 15's, the siege engine's.
pub const ENGINE_CORPSE_FRAMES: u16 = 120;

/// One side's army as it is raised: troop counts in figures, plus who owns it.
#[derive(Debug, Clone, Copy)]
pub struct Army<'a> {
    /// `(troop, figures)` in raise order — what [`army_from_counts`] produces.
    pub troops: &'a [(Troop, u16)],
    /// The realm this army belongs to. **Zero means a free slot**, so an owner
    /// of 0 would make every unit read as dead; [`BattleRunner::deploy_armies`]
    /// refuses it.
    pub owner: u8,
    /// Controlled by a human. No order handler runs for a human unit
    /// (`Battle_UpdateAllUnits` guards on it), and
    /// `Battle_UpdateStrengthAdvantage` weighs the human side against the AI
    /// side — so **a battle with no human side reads a strength advantage of
    /// −100 and every AI unit takes its cautious branch.** That is the
    /// original's arithmetic, not a limitation here, and it is why
    /// [`BattleRunner::deploy`] makes one side human.
    pub human: bool,
}

/// One side's army as the **campaign** hands it over: real men per troop type,
/// not figures.
///
/// [`Army`] is the skirmish shape — a fixed number of figures of four men each,
/// which is what a `.skr` map and the shell want. A campaign army is a row of
/// eleven counts out of a `g_units` record (`docs/armies.md` §1.4), and how
/// many men one drawn figure stands for is **not** the caller's choice: it is
/// derived from the two armies together by `Battle_InitArmies`
/// (`docs/battle.md` §5.1). So this carries men and
/// [`BattleRunner::deploy_muster`] picks the scale.
#[derive(Debug, Clone, Copy)]
pub struct Muster<'a> {
    /// `(troop, men)` — real men, in any order; [`RAISE_ORDER`] is applied here.
    pub troops: &'a [(Troop, u32)],
    /// The realm this army belongs to. Zero is the free-slot marker and is
/// refused, as in [`BattleRunner::deploy_armies`].
    pub owner: u8,
    pub human: bool,
}

/// `g_menPerFigureTable` (`0x004D9658`) — men one drawn figure stands for, by
/// battlefield size class. **[V]** `docs/battle.md` §5.1.
pub const MEN_PER_FIGURE_TABLE: [u32; 9] = [4, 8, 16, 32, 64, 128, 256, 512, 1024];

/// `g_sizeClassLadder` (`0x004D95D8`) — the totals at which the size class
/// steps up. **[V]** `docs/battle.md` §5.1.
pub const SIZE_CLASS_LADDER: [u32; 8] = [305, 609, 1217, 2433, 4865, 9729, 19457, 38913];

/// Why a battle ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    /// One side has no men left. The only way a field battle ends by itself.
    Annihilation,
    /// A side left the field — [`BattleRunner::withdraw`].
    Withdrawal,
    /// **Siege only.** A besieging figure reached the castle's `0x08` cell —
    /// `DAT_00553F3C`. The garrison is still standing and the siege is over
    /// anyway.
    BrokeIn,
    /// **Siege only.** No breach and no siege engines left, against a castle
    /// of [`ASSAULT_REPEATS_BELOW_LEVEL`] or above: the besieger has no way in
    /// and loses. Below that level the same position resets the two progress
    /// scores and the battle carries on instead.
    AssaultFailed,
}

/// **The battle is over.** [`BattleRunner::conclusion`]'s answer.
///
/// `FUN_00477DFC` reaches
/// this point six ways and `FUN_00478419` turns the result into one of `L2.eng`
/// group 82's seven heading/body pairs. Two of those ways are in scope here;
/// the rest are sieges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Conclusion {
    /// The side left holding the field.
    pub winner: Side,
    pub cause: End,
}

/// How long the original leaves the outcome banner up before
/// `Battle_ReturnToCampaign` runs — `DAT_00568470` counting past 5000 in
/// `FUN_00477DFC`.
///
/// **Nothing about the result changes while it counts**, so this is presentation
/// timing; it is here because the write-back is on the far
/// side of it, and a caller reproducing the original's pacing needs the number.
pub const SETTLE_TICKS: u32 = 5000;

/// The battle: rules, battlefield, units, figures and the clock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattleRunner {
    /// The rules. Melee, damage and death happen in here.
    pub sim: Battle,
    pub field: Battlefield,
    pub fighters: Vec<Fighter>,
    /// The unit array the order handlers think with — `g_battleUnits`.
    pub units: Units,
    /// The AI's globals: the strength advantage, the commit counter, the
    /// frozen generator.
    pub ai: Ai,
    /// Everything positional the handlers read. Built from the battlefield.
    pub ai_field: AiField,
    /// Where each figure stands, indexed by figure — the slice
    /// [`crate::unit::Units::recentre`] and [`ai::update_all_units`] take.
    /// Rebuilt at the top of every tick from [`Self::fighters`].
    positions: Vec<(u8, u8)>,
    /// Which fighter stands on each cell, mirroring the original's cell byte
    /// `+5`. `None` is the original's zero.
    ///
    /// **A missile's hit test reads this and nothing else**, which is what makes
    /// a body in the flight path take the arrow.
    occupant: Vec<Option<u16>>,
    /// **The arrows in the air** — `g_missiles`, a hundred fixed slots.
    ///
    /// Simulation state like any other: two lockstep peers must agree about
    /// where a shot is,
    /// hundred records (`Sync_RecordDigest(&g_missiles + i*0x4c, 0x4c, 4)`).
    pub missiles: crate::missile::Missiles,
    /// Impassable terrain, built once from the battlefield flags.
    blocked: Vec<bool>,
    /// The castle, the two damage accumulators and the way in.
    /// [`crate::siege::SiegeState::field`] on a field battle, and every rule it
    /// carries is then inert.
    pub siege: crate::siege::SiegeState,
    /// One-shot latches for the garrison's first two missile units, which take
/// dispatch categories **9** and **10** — `BattleUnit_Create`,
    /// siege only, side 0 only. `docs/battle-ai.md` §1.2.
    wall_missile_latch: u8,
    /// The side that has left the field, if any — `DAT_0056D5C8` and
    /// `DAT_005656F8` folded into one. Outranks annihilation.
    withdrawn: Option<Side>,
    /// Men one figure of each side stands for, indexed `[side 0, side 4]` —
    /// `docs/battle.md` §5.1. Four for a skirmish; the campaign's ladder picks
    /// it in [`Self::deploy_muster`], and each side may differ.
    men_per_figure: [u16; 2],
    /// `g_battleSizeClass` — the rung of `g_sizeClassLadder` the two armies'
    /// total reached, before either side's scale was halved. Zero for a
    /// skirmish. `BattleMan_BurnTick` reads it; see [`Self::battle_size_class`].
    size_class: u8,
    /// **`DAT_0053E9D0` — a wood is burning.** Raised by a fire arrow setting
    /// woodland alight and kept up by [`fire::spread_woodland`] for as long as
    /// a frame turns something from catching to burning.
    pub wood_fire: bool,
    /// **`DAT_005530E8`** — a human's figures standing in woodland, counted
    /// **during** the man sweep and zeroed at its head,
    /// the sweep sees only the figures before it. Over three of them and an AI
    /// garrison looses fire arrows. Recomputed every frame before it is read,
    /// so it carries nothing from one frame to the next.
    humans_in_woods: u32,
    pub tick: u32,
}

/// Men one figure stands for. The size ladder that picks this from the two
/// armies' totals (`docs/battle.md` §5.1) belongs to whoever builds the armies;
/// this is the value the previous driver used and it is kept.
const MEN_PER_FIGURE: u16 = 4;

/// Seed for the AI's generator when the caller does not supply one.
///
/// Fixed, because a battle that seeded itself from the clock would not be
/// reproducible and could not be run in lockstep. `Battle_Start` seeds the
/// original's pair of LFSRs from a global we have not traced.
pub const DEFAULT_SEED: u64 = 0x004D_9870;

/// What a player's click asks of a unit, in the vocabulary
/// `BattleUnit_Order` (`0x00479E90`) takes.
///
/// The original's sixth parameter is called `facing` in `docs/symbols.json` and
/// it is one of exactly three values at the two call sites a player can reach:
/// `0` from `FUN_0043C634` (the click on the field) and `1` or `2` from
/// `FUN_0043C77A` (the `H` and `V` keys). Non-zero rewrites the unit's
/// destination back to where it already stands, so the two keys are a *turn on
/// the spot* and never a move.
///
/// `BattleUnit_Order` stores `facing - 1` in unit
/// `+0x09`, and the one function that reads `+0x09` is `Formation_ComputeRect`
/// (`0x0048A1C9`), whose whole use of it is `if (unit.field_0x9 == 1)
/// g_formationCols = 2;`. So the two keys pick between the troop type's own
/// figures-per-row and a two-wide column, which is line and column. **[V]**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Formation {
    /// `facing = 0` — write nothing; the unit keeps whatever it had.
    Keep,
    /// `facing = 1`, the `H` key: unit `+0x09` becomes 0, so the rectangle is
    /// as wide as the troop's `ROW_MAX`.
    Line,
    /// `facing = 2`, the `V` key: unit `+0x09` becomes 1, so the rectangle is
    /// forced to **two** columns however many men are in it.
    Column,
}

#[cfg(test)]
#[path = "../runner_fire_tests.rs"]
mod fire_tests;

