#![allow(unused_imports)]
use super::*;
use super::demographics::*;
use super::agriculture::*;
use super::castle_and_tax::*;
use super::jobs_and_goods::*;
use super::ai_and_grants::*;
use super::scoring::*;
use super::*;

/// The number of rows `g_armyHappinessCost` has, which is the percentage
/// domain 0..=101: index 101 is the last slot
/// before the merchant price table at `0x004D8910` begins. A ruleset may
/// change every cost in the table and may not change how many there are — the
/// same line [`JOB_COUNT`] and [`WEAPON_TYPE_COUNT`] are on.
pub const ARMY_HAPPINESS_COST_LEN: usize = 102;

/// `g_armyHappinessCost` (`0x004D8778`) - the happiness raising an army costs
/// the county it is raised in, indexed by **the percentage of the county's
/// population being taken**.
///
/// `[V]` on the table and on the indexing: `FUN_004A5003` computes
/// `pct = PctOf(50, population)` — the share of the county fifty men are — and
/// then reads `g_armyHappinessCost[pct]`, raises `Pct(population, pct)` men,
/// and subtracts the cost from both `happiness` (`+0x0C`) and `shownArmy`
/// (`+0x15`, `L2.eng` group 85 *"From army"*). That is the writer
/// `docs/kingdom.md` §12 records as not found.
///
/// The shape is the rule: taking a twentieth of a county costs 2 happiness and
/// taking half of it costs 90. The last 41 entries are all 101, so beyond 60%
/// the cost is flat and ruinous.
///
/// 102 entries, `0x004D8778 … 0x004D8910`, which is exactly where the merchant
/// price table begins.
///
/// All 102 are checked against the executable by `tools/oracle/kingdom.ps1`,
/// together with the merchant table that bounds them — so the length is held
/// by address arithmetic.
pub const ARMY_HAPPINESS_COST: [i32; ARMY_HAPPINESS_COST_LEN] = [
    0, 1, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 11, 13, 15, 17, 19, 21, 23, 25,
    27, 29, 31, 34, 37, 40, 44, 48, 52, 56, 60, 64, 68, 72, 75, 78, 80, 82, 84, 86, 88, 90, 91, 92,
    93, 94, 95, 96, 97, 98, 99, 100, 101, 101, 101, 101, 101, 101, 101, 101, 101, 101, 101, 101,
    101, 101, 101, 101, 101, 101, 101, 101, 101, 101, 101, 101, 101, 101, 101, 101, 101, 101, 101,
    101, 101, 101, 101, 101, 101, 101, 101, 101, 101,
];

/// The happiness cost of taking `pct` percent of a county into an army.
///
/// **The original does not bound the index.** `PctOf(50, population)` exceeds
/// 101 for any county under 50 people, and the read then lands on the first
/// entry of the merchant price table, which is 0 — a free army. This clamps
/// instead, because reproducing a read of a *different table* would mean
/// hard-coding that the two happen to be adjacent, and a mod that resizes
/// either would make the reproduction meaningless. Flagged
/// silently smoothed: see [`ARMY_HAPPINESS_COST`].
#[inline]
pub fn army_happiness_cost(pct: i32) -> i32 {
    if pct <= 0 {
        return ARMY_HAPPINESS_COST[0];
    }
    ARMY_HAPPINESS_COST[(pct as usize).min(ARMY_HAPPINESS_COST.len() - 1)]
}

// ---------------------------------------------------------------------------
// Units on the campaign map - docs/armies.md
// ---------------------------------------------------------------------------
//
// Every number below was read out of the instruction stream
// `.data`: an army's movement budget, its step costs and its desertion rate are
// `MOV`/`ADD`/`CMP` immediates, exactly the case `docs/decisions.md` C16
// describes. `tools/oracle/kingdom.ps1`'s second tier checks each of them
// against `Lords2.exe`, so "the budget is 15" is held by the executable rather
// than by this comment.

/// `Army_Tick` (`0x0046521F`) writes this to `+0x154` **every tick**, with no
/// condition on the army's size, its owner or its terrain. `[V]` — the panel
/// prints `15 - movesUsed` beside `L2.eng` 31/22 *"moves left."*
pub const MOVE_ALLOWANCE_ARMY: i32 = 15;

/// The other three tick handlers — `0x00465486` (peasant mob), `0x00465622`
/// (merchant) and `0x00465761` (transport) — write **10** to the same byte.
/// `[V]`, three functions agreeing.
pub const MOVE_ALLOWANCE_OTHER: i32 = 10;

/// A step onto a road tile. `Unit_StepOnce` (`0x0046634D`) reaches it as an
/// `INC` of `+0x153`
/// to read and the oracle check tags the opcode instead of a value.
pub const STEP_COST_ROAD: i32 = 1;

/// Every step that is not a road step. `Unit_StepOnce`'s `ADD EAX, 3`.
pub const STEP_COST_OPEN: i32 = 3;

/// **How far a unit has to get across a tile before it enters the next one.**
///
/// `Unit_StepOnce` (`0x0046634D`) keeps a sub-tile accumulator in `+0x149` and
/// only calls `Unit_NeighbourTile` — the thing that moves the unit —
/// on the tick the accumulator reaches this. `[V]`:
///
/// ```c
/// if ((char)g_units[g_movingUnit].field_0x149 < '\x10') {  local_8 = 1;  }
/// else { field_0x14b |= 1; field_0x149 = 0; local_8 = 2; }
/// ```
///
/// See [`crate::units_tick`] for what the three numbers here work out to in
/// ticks a tile, and why leaving them out is the difference between a march
/// and a teleport.
pub const SUBTILE_SPAN: u8 = 0x10;

/// What one admitted tick adds to the accumulator **in single player**.
/// `Unit_StepOnce`'s `if (g_multiplayer == 0) field_0x149 += 2; else += 4;`.
pub const SUBTILE_STEP_SOLO: u8 = 2;

/// And in a network game — **twice the speed**, which is a simulation
/// difference and not a display one.
///
/// It is unreachable today: nothing below `l2-game` knows whether the session
/// is networked, and threading `g_multiplayer` into `Kingdom` would put a
/// session property into the lockstep digest. Both peers of a network game
/// take the same arm, so the value agrees where it matters; what does not yet
/// exist is a way to *select* it. **Named** — a unit that
/// walked at half speed the day multiplayer landed would be a defect nobody
/// would think to look for here.
pub const SUBTILE_STEP_NET: u8 = 4;

/// How many ticks the accumulator waits between admissions: none on a road,
/// three off one.
///
/// `Unit_StepOnce`'s `cVar1 = onRoad ? 0 : 3`, tested as
/// `if (cVar1 < ++field_0x14a)`.
/// as an open one **on top of** costing a third as much
/// ([`STEP_COST_ROAD`] against [`STEP_COST_OPEN`]) — two independent
/// mechanisms, and only the second one was here.
pub const SUBTILE_DIVIDER: [u8; 2] = [3, 0];

/// `Unit_CrossField` (`0x0046673C`) charges this **before** `Unit_StepOnce`'s
/// general `+3`,
/// `Move_BuildCostMap` stores a single literal `6` for the same tile. Two
/// unrelated codings landing on one number is what makes the field cost `[V]`.
pub const STEP_COST_FIELD_EXTRA: i32 = 3;

/// `Unit_TrampleTile` (`0x0046873F`) charges this for walking over a resource
/// site, and the move ends there.
pub const STEP_COST_TRAMPLE: i32 = 7;

/// What `Unit_TrampleTile` writes into the industry record's
/// `disabledSeasons`. **It always writes 3**
/// dependence on the army's size.
pub const TRAMPLE_DISABLED_SEASONS: i32 = 3;

/// `Move_BuildCostMap` (`0x0046FF43`) stores this for a castle site, an intact
/// settlement and an occupied dwelling plot: passable in principle, ruinous in
/// practice, so the pathfinder routes round them.
pub const MOVE_COST_BLOCKED: i32 = 100;

/// `Move_BuildCostMap`'s impassable marker — sea, mountain, woodland, and any
/// tile whose county byte is above [`crate::county::MAX_COUNTY_ID`].
pub const MOVE_COST_IMPASSABLE: i32 = 0;

/// `Army_Combine` (`0x004AA181`) merges when `menA + menB <= 1500`. The
/// decompiler renders the test as `< 0x5DD`; the instruction is
/// `CMP EAX, 0x5DC` followed by `JLE`, so the constant to carry is **1500**.
pub const ARMY_MAX_MEN: i32 = 1500;

/// `FUN_00435B4D` refuses a levy below this with message `0x94` — `L2.eng` 148,
/// *"impractical to create an army of less than 50 men"*. Bypassed entirely
/// when a mercenary band is being hired, because the band supplies the men.
pub const ARMY_MIN_MEN: i32 = 50;

/// The peasant-mob tick (`0x00465486`) destroys a **type-2** unit whose men
/// fall below this.
///
/// **`docs/armies.md` §0 files this under *"minimum army"*, and it is not an
/// army rule.** `Army_Tick` has no such test; the sub-30 destruction is in the
/// revolting-peasants handler, which is what `g_unitTickTable` slot 2 points
/// at. See [`crate::unit::UnitKind::PeasantMob`].
pub const MOB_DESTROYED_BELOW_MEN: i32 = 30;

/// `Army_Desert` (`0x004AD16C`) takes this percentage off each troop count.
pub const DESERTION_PCT: i32 = 10;

/// …but only from a troop count that **exceeds** this. A type with ten men or
/// fewer loses none,
pub const DESERTION_MIN_TROOPS: i32 = 10;

/// `Army_Starve` (`0x004ACE5E`) destroys the army once its starvation counter
/// reaches this. Below it, counter 1 only warns and 2..=4 desert.
pub const STARVATION_LIMIT: i32 = 5;

/// `Army_Create` writes this to county `+0x2F4`, the levy surcharge
/// `Levy_SetPercent` adds to every subsequent levy in the same county.
/// **Nothing was found that decays it** (`docs/armies.md` §6.1).
pub const LEVY_SURCHARGE: i32 = 15;

/// `Levy_SetPercent` (`0x00435EBC`) clamps the happiness cost to this before
/// walking the percentage back.
pub const LEVY_COST_MAX: i32 = 100;

/// The auto-equip path (`0x004A50AE` with mode 1) moves men into a weapon slot
/// this many at a time, round-robin over the six weapon types.
pub const LEVY_AUTO_EQUIP_BATCH: i32 = 10;

/// …for at most this many rounds. At six types and ten men a round that is
/// 3,000 men, twice [`ARMY_MAX_MEN`], so the bound never bites in play; it is
/// carried because it is the original's own loop guard.
pub const LEVY_AUTO_EQUIP_ROUNDS: i32 = 50;

/// The two thresholds `Army_Tick` picks the sprite bank on: under 301 men one
/// bank, under 601 the next, above that the third. The instructions are
/// `CMP …, 300` / `CMP …, 600` with `JG`, so the *stored* numbers are 300 and
/// 600 and the classes break at 301 and 601.
pub const ARMY_SIZE_CLASS_MAX: [i32; 2] = [300, 600];

/// `g_unitWalkFrames` (`0x004D6A78`) — the walk-cycle ping-pong `Army_Tick`
/// adds to the sprite bank. **Sixteen entries**, `0,1,2,1` four times over:
/// `0x004D6A78 + 64` is `0x004D6AB8`, where a different table (0,1,2,3…)
/// begins, which is what fixes the length.
pub const UNIT_WALK_FRAMES: [i32; 16] = [0, 1, 2, 1, 0, 1, 2, 1, 0, 1, 2, 1, 0, 1, 2, 1];

