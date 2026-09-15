//! * Unit sizes, footprints, formation geometry and the deployment slot per
//!   unit: [`crate::formation`]. **[V]** from `g_troopBattleStats`.
//!
//! * The seventeen order handlers, the 200-frame think, the 101-frame strength
//!   advantage: [`crate::ai`]. **[D]**
//! * Movement timing, pathfinding, melee: the modules named above.
//!
//!   the state that decides *when* a figure is in the original's firing state — here it is "standing at its destination and out
//!   of melee", which is `[I]`.
//!
//! * **Fire, oil and the siege tower** are [`crate::fire`]'s and
//!   [`crate::siege`]'s rules, run from here in the original's places: a man
//!   burns at the head of his own frame (`Battle_UpdateAllMen`), a fire goes out
//!   and a stream of oil paints its cross between a missile's step and its
//!   countdown (`Missile_UpdateAll`), a wood spreads after the sweep
//!   (`FUN_004859E5`), and a tower docks when its step is refused
//!   (`BattleMan_Step`).

mod anim;
#[cfg(test)]
mod anim_tests;
#[cfg(test)]
mod dying_tests;
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
mod stop_short;
pub use stop_short::*;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fighter {
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
    pub progress: Progress,
    pub anim: Motion,
    /// The figure's own animation counter — figure `+0x0E`, which every
    /// `Anim_*` handler steps. Seeded per figure so that identical men do not
    /// march in lockstep.
    pub phase: u8,
    /// **The drawn facing** — figure `+0x0D`, written at the end of every
    /// animation handler as a copy of `dirc2` (`+0x19`).
    ///
    /// `docs/battle.md` §13.8: `+0x18` drives the sub-cell offset and the
    /// **walk** frame, `+0x19` the **strike** frame. We carry one facing for
    /// the simulation and this one for the pictures, because
    /// [`Self::fidget`] turns it and must not turn a man's step. **[V]**
    pub facing_drawn: u8,
    /// **The fidget counter** — figure `+0x0B`, `docs/battle.md` §14.6.
    ///
    /// `Anim_StandA2` (`0x004872AE`) counts it up each frame and, when it
    /// passes [`Self::fidget_period`], resets it and turns `facing_drawn` one
    /// step — left on an even map x, right on an odd one. A rank of men
    /// standing still shuffles, and no two neighbours shuffle together. **[V]**
    /// — one writer (`BattleMan_Create`), two readers, both `Anim_Stand*`.
    pub fidget: u8,
    /// **The fidget period** — figure `+0x0C`, seeded once as
    /// `((index * 9 + x * 16) & 0x3F) + 0xB4`, so 180 … 243 frames. **[V]**
    pub fidget_period: u8,
    pub path: Vec<Pos>,
    /// Consecutive pathfinding failures. At four the figure stops trying —
    /// the original's `barred`, `+0x176`.
    pub barred: u8,
    /// Ticks to wait before asking the pathfinder again — `hold it`, `+0x165`.
    pub hold: u8,
    /// How many times this figure has been re-routed — `routed`, `+0x166`.
    pub reroutes: u16,
    /// **The moat cell this figure is shovelling into** — figure record
    /// `+0x190`, with `+0x18F` folded into the `Option`.
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
    /// **Frames this man must stand still** — figure record `+0x172`, the
    /// original's `delay`, counted down by its state 1 handler.
    ///
    /// `BattleMan_Step` (`0x0048F1DD`) puts the man swapped out of his cell
    /// into it:
    ///
    /// ```c
    /// if (BattleMen_SwapPlaces() == 2) {            /* 0x0049005F */
    ///     other.state = 1;  other.delayState = 3;
    ///     other.stepFlags &= 0xFD;
    ///     other.delay = (other & 1) + 1;            /* 1 or 2 frames */
    ///     other.barred = 0;  cur.barred = 0;
    ///     return 0;
    /// }
    /// ```
    ///
    /// so a pushed man takes no step of his own that frame; without it he
    /// moves two cells in one tick. `delayState` is not carried — state 3 is
    /// walking and our mover re-derives `anim` from the target every tick —
    /// and `stepFlags` bit 1 is not modelled. `[D]`, both.
    ///
    /// **Three writers**, all in `BattleMan_Step` (`0x0048F1DD`): this swap
    /// (`00480000.c:6451`), the refused swap's 1-2 frame wait
    /// (`00480000.c:6461`) and the blocked arm's `delay = 100`
    /// (`00480000.c:6585`). `BattleMan_StateDelay` (`0x00482F91`) is the reader.
    pub delay: u8,
}

pub const CORPSE_FRAMES: u16 = 80;
pub const ENGINE_CORPSE_FRAMES: u16 = 120;

#[derive(Debug, Clone, Copy)]
pub struct Army<'a> {
    pub troops: &'a [(Troop, u16)],
    pub owner: u8,
    pub human: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Muster<'a> {
    pub troops: &'a [(Troop, u32)],
    pub owner: u8,
    pub human: bool,
}

/// `g_menPerFigureTable` (`0x004D9658`) — men one drawn figure stands for, by
/// battlefield size class. **[V]** `docs/battle.md` §5.1.
pub const MEN_PER_FIGURE_TABLE: [u32; 9] = [4, 8, 16, 32, 64, 128, 256, 512, 1024];

/// `g_sizeClassLadder` (`0x004D95D8`) — the totals at which the size class
/// steps up. **[V]** `docs/battle.md` §5.1.
pub const SIZE_CLASS_LADDER: [u32; 8] = [305, 609, 1217, 2433, 4865, 9729, 19457, 38913];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    Annihilation,
    Withdrawal,
    /// **Siege only.** A besieging figure reached the castle's `0x08` cell —
    /// `DAT_00553F3C`. The garrison is still standing and the siege is over
    /// anyway.
    BrokeIn,
    AssaultFailed,
}

/// `FUN_00477DFC` reaches
/// this point six ways and `FUN_00478419` turns the result into one of `L2.eng`
/// group 82's seven heading/body pairs. Two of those ways are in scope here;
/// the rest are sieges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Conclusion {
    pub winner: Side,
    pub cause: End,
}

/// How long the original leaves the outcome banner up before
/// `Battle_ReturnToCampaign` runs — `DAT_00568470` counting past 5000 in
/// `FUN_00477DFC`.
pub const SETTLE_TICKS: u32 = 5000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattleRunner {
    pub sim: Battle,
    pub field: Battlefield,
    pub fighters: Vec<Fighter>,
    pub units: Units,
    pub ai: Ai,
    pub ai_field: AiField,
    positions: Vec<(u8, u8)>,
    occupant: Vec<Option<u16>>,
    pub missiles: crate::missile::Missiles,
    blocked: Vec<bool>,
    pub siege: crate::siege::SiegeState,
    wall_missile_latch: u8,
    /// The side that has left the field, if any — `DAT_0056D5C8` and
    /// `DAT_005656F8` folded into one. Outranks annihilation.
    withdrawn: Option<Side>,
    men_per_figure: [u16; 2],
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

const MEN_PER_FIGURE: u16 = 4;

pub const DEFAULT_SEED: u64 = 0x004D_9870;

/// What a player's click asks of a unit, in the vocabulary
/// `BattleUnit_Order` (`0x00479E90`) takes.
///
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
    Keep,
    /// `facing = 1`, the `H` key: unit `+0x09` becomes 0, so the rectangle is
    /// as wide as the troop's `ROW_MAX`.
    Line,
    /// `facing = 2`, the `V` key: unit `+0x09` becomes 1, so the rectangle is
    /// forced to **two** columns however many men are in it.
    Column,
}

#[cfg(test)]
#[path = "../runner_fire_tests/mod.rs"]
mod fire_tests;

