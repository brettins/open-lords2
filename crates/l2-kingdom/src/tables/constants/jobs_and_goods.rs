#![allow(unused_imports)]
use super::*;
use super::demographics::*;
use super::agriculture::*;
use super::castle_and_tax::*;
use super::ai_and_grants::*;
use super::military_and_movement::*;
use super::scoring::*;

// ---------------------------------------------------------------------------
// Industry and weapons
// ---------------------------------------------------------------------------

/// The order `Industry_Produce` is called in, from the driver
/// `FUN_0044E852` at the `Season_Advance` slot `docs/kingdom.md` §3.4 lists.
///
/// **`[V]` and it is neither order the document gives.** §3.4's call list says
/// *"wood, iron, stone, weapons"* and §7.4's table is indexed wood, iron,
/// weapons, stone; the driver runs **weapons over every county first**, in its
/// own loop, and then iron, stone and wood per county in a second loop. The
/// weapons-first ordering is load bearing - the blacksmith spends the wood and
/// iron that the *previous* season's mining produced, because this season's has
/// not run yet.
pub const INDUSTRY_ORDER: [Commodity; 4] =
    [Commodity::Weapons, Commodity::Iron, Commodity::Stone, Commodity::Wood];

/// The order `County_RefreshEstimates` (`0x004485A5`) refreshes the four
/// industry *ceilings* in — **iron, stone, wood, weapons**, which is neither
/// [`INDUSTRY_ORDER`] nor the record order.
///
/// It does not matter, because no industry's ceiling reads another's, and it is
/// written down because the four argument lists it comes from are a second,
/// independent reading of [`COMMODITY`]: `(1, 4, 15, 1)`, `(3, 5, 15, 2)`,
/// `(0, 6, 20, 1)`, `(2, 7, 15, 4)` are `(record, job, base efficiency,
/// divisor)` and every one of the twelve numbers agrees with that table.
/// **`[V]`**
pub const INDUSTRY_ESTIMATE_ORDER: [Commodity; 4] =
    [Commodity::Iron, Commodity::Stone, Commodity::Wood, Commodity::Weapons];

/// The nine assignable labour slots at county `+0xC4 + job*0x0C`.
///
/// **`[V]` and `docs/kingdom.md` §7.4's job column is wrong by one.** The
/// document reads the numbers straight off `L2.eng` group 74, which has *ten*
/// strings starting with *"Idle people"*; the record array has **nine** entries
/// and record `r` is group-74 string `r + 1`, because "idle people" is the
/// remainder.
///
/// Three independent facts fix the offset, and they agree:
///
/// * `Industry_Produce`'s driver passes job 7 for weapons, 4 for iron, 5 for
///   stone and 6 for wood - each one less than §7.4's column;
/// * the labour allocator (`FUN_0044F6E7`) gates slot 6 on the *wood* industry
///   record's enable flag, slot 4 on *iron*, slot 5 on *stone* and slot 7 on
///   the *blacksmith* - the same mapping from the other direction;
/// * the allocator clears exactly nine records, `for (c = 0; c < 9; c++)`.
pub const JOB_COUNT: usize = 9;

/// `L2.eng` group 74 strings 1..=9, which is what the nine records are.
pub const JOB_NAMES: [&str; JOB_COUNT] = [
    "Grain farming",
    "Cattle farming",
    "Field reclamation",
    "Castle building",
    "Iron mining",
    "Stone quarrying",
    "Wood cutting",
    "Blacksmith",
    "Idle townsfolk",
];

/// **`[V]`** - `Grain_SeasonTick` (`0x0044C8AE`) passes county `+0xC4` with no
/// job stride to all three of `Grain_Sow`, `Grain_Grow` and `Grain_Harvest`,
/// which is record 0. The crate's previous placeholder guess of slot 0 turns
/// out to have been right, and it is *"Grain farming"*, not *"Idle people"* -
/// see [`JOB_COUNT`].
pub const JOB_GRAIN_FARMING: usize = 0;
pub const JOB_CATTLE_FARMING: usize = 1;
pub const JOB_FIELD_RECLAMATION: usize = 2;
/// `[I]` - named from group 74 string 4, not traced to `Castle_BuildTick`.
pub const JOB_CASTLE_BUILDING: usize = 3;
pub const JOB_IRON_MINING: usize = 4;
pub const JOB_STONE_QUARRYING: usize = 5;
pub const JOB_WOOD_CUTTING: usize = 6;
pub const JOB_BLACKSMITH: usize = 7;
pub const JOB_IDLE_TOWNSFOLK: usize = 8;

/// The six weapon types, in the order `g_weaponCost` stores them.
pub const WEAPON_TYPE_COUNT: usize = 6;

pub const WEAPON_NAMES: [&str; WEAPON_TYPE_COUNT] =
    ["Crossbow", "Mace", "Sword", "Pike", "Bow", "Armour"];

/// `g_weaponCost` (`0x004D8990`) - six `{wood, iron}` pairs. Identical to two
/// independently published tables. `docs/kingdom.md` §7.4.
pub const WEAPON_COST: [(i32, i32); WEAPON_TYPE_COUNT] =
    [(6, 10), (4, 4), (3, 10), (6, 3), (13, 0), (4, 18)];

// ---------------------------------------------------------------------------
// Trade
// ---------------------------------------------------------------------------

/// `0x004D8910` - the merchant base **sell** price, indexed by `L2.eng` group 6
/// good id 1..=14. Index 0 is unused.
///
/// The two zeros are sheep and wool, the two goods a county cannot produce or
/// trade in the base game. A published guide's prices are uniformly twice
/// these
/// then buy. `docs/kingdom.md` §10 and §11.
pub const GOOD_SELL_PRICE: [i32; 15] = [0, 2, 12, 0, 1, 0, 1, 2, 1, 13, 16, 10, 24, 23, 44];

pub const GOOD_NAMES: [&str; 15] = [
    "-", "Grain", "Cattle", "Sheep", "Ale", "Wool", "Iron", "Stone", "Timber", "Pikes", "Bows",
    "Maces", "Crossbows", "Swords", "Mail",
];

// ---------------------------------------------------------------------------
// The AI's advantages
// ---------------------------------------------------------------------------

/// `g_aiGoldGrant` (`0x004DC1E0`) - `int[5][4]`, indexed `[lord][difficulty]`,
/// used when the realm holds **three or more** counties.
///
/// **`[V]`, all five rows, read out of the bytes at `0x004DC1E0`.**
/// `docs/kingdom.md` §8.2 gave only the endpoints - *"from all zeros for lord 0
/// up to `250, 600, 1100, 1800`"* - and this crate zeroed rows 1..=3 rather
/// than invent them. They are no longer invented; `docs/symbols.md` carries the
/// same five rows, derived independently.
///
/// Rows 1 and 3 are **identical**, and that row 2 is the only row
/// that pays anything at difficulty 0.
///
/// Row 0 being all zeros is the load-bearing part: **the human's `lord` byte is
/// 0, so the human gets nothing.**
pub const AI_GOLD_GRANT: [[i32; 4]; 5] = [
    [0, 0, 0, 0],           // lord 0 - the human
    [0, 400, 700, 1200],    // lord 1
    [100, 500, 800, 1400],  // lord 2
    [0, 400, 700, 1200],    // lord 3 - the same row as lord 1
    [250, 600, 1100, 1800], // lord 4
];

/// population, up to [`ALE_HAPPINESS_MAX`].
///
/// **`[V]`, and it settles the claim `docs/kingdom.md` §12 records as
/// unverified.** The published figure is *"+1 per 20% of the population, cap
/// +5"*; `FUN_00428C42` computes `tenth = population / 10` and then compares
/// the crowns spent against `tenth`, `2*tenth` … `5*tenth`. So it is **+1 per
/// 10%**, and the cap of +5 is reached at half the population. The published
/// cap is right and the published step is twice too big.
///
/// Two functions in the binary compute this same ladder - the purchase
/// (`FUN_00428C42`) and the panel's preview (`FUN_00435673`) - and they agree
/// line for line, which is the second source.
///
/// Like the efficiency ceiling this is an immediate and not a table:
/// `MOV ECX, 0x0A` at `0x00428C79`, feeding the `IDIV` that makes the step.
/// `tools/oracle/kingdom.ps1` reads it out of `.text`.
pub const ALE_HAPPINESS_STEP_PCT: i32 = 10;

/// The most happiness ale can ever be worth in one county.
///
/// **The cap is cumulative and nothing resets it.** County `+0x219` holds the
/// total already granted and the bonus is clamped to `5 - that`; no write to
/// `+0x219` other than this `+=` was found anywhere in the binary.
/// can be given at most **five happiness from ale for the whole game**, not
/// five per season. `[D]` - a negative, and negatives are hard to prove; the
/// search was a cross-reference of every instruction touching the offset.
///
/// The 5 is `MOV EAX, 5` at `0x00428C5A` — the `5 - given` clamp — and the
/// ladder above it stores its rungs as `MOV dword ptr [ebp-8], 5 … 0`. Both
/// are checked out of `.text` by `tools/oracle/kingdom.ps1`, which is what
/// pins the rung *count* to the cap: one number does both jobs in the
/// original, and it is one field here.
pub const ALE_HAPPINESS_MAX: i32 = 5;

