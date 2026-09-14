#![allow(unused_imports)]
use super::*;
use super::demographics::*;
use super::agriculture::*;
use super::jobs_and_goods::*;
use super::ai_and_grants::*;
use super::military_and_movement::*;
use super::scoring::*;
use super::*;

// ---------------------------------------------------------------------------
// Castles
// ---------------------------------------------------------------------------

/// `L2.eng` group 71, and group 103 indices 19-24 for the options screen.
/// `docs/kingdom.md` §1.3.
pub const CASTLE_TYPE_COUNT: usize = 6;

pub const CASTLE_NAMES: [&str; CASTLE_TYPE_COUNT] = [
    "None",
    "Wooden palisade",
    "Motte and bailey",
    "Norman keep",
    "Stone castle",
    "Royal castle",
];

/// The default starting castle. Every player-owned county in the shipped
/// `lastturn.sav` has `castleType = 3`, and `L2.eng` group 103 index 22 - the
/// value word for the "Starting Castle" option - is `keep`.
/// `docs/kingdom.md` §7.5.
pub const CASTLE_STARTING_TYPE: u8 = 3;

/// The tax base `Tax_CollectAll` multiplies the population by, indexed by
/// castle type 0..=5. These are **immediates in the instruction stream**, not a
/// table (`docs/kingdom.md` §10).
///
/// They are the published castle tax bonuses in different units:
/// `480/320 = 1.50`, `560/320 = 1.75`, `640/320 = 2.00`, `720/320 = 2.25`,
/// `800/320 = 2.50` - exactly [`CASTLE_TAX_BONUS_PCT`].
pub const CASTLE_TAX_BASE: [i32; CASTLE_TYPE_COUNT] = [320, 480, 560, 640, 720, 800];

/// `g_castleTaxBonus` (`0x004D8A28`) - used only by the UI, per
/// `docs/kingdom.md` §10, but kept because it is the second, independent
/// statement of [`CASTLE_TAX_BASE`]. Six slots, the sixth zero.
pub const CASTLE_TAX_BONUS_PCT: [i32; 6] = [50, 75, 100, 125, 150, 0];

/// The highest tax rate the player can set. **[V]** twice over: `Tax_Increase`
/// (`0x0043AA32`) guards `taxRate < 0x32`, and [`TAX_HAPPINESS_OTHER`] holds
/// exactly 51 entries, one per rate `0 ..= 50`.
///
/// It was 100 until it was read - an arithmetic bound standing in for a rule -
/// and it lived in the *application* crate, where a rule has no business being.
pub const MAX_TAX_RATE: i32 = 50;

/// `g_taxHappinessOther` (`0x004D63D8`) - what one county's tax rate does to
/// the happiness of **every other county in the same realm**. **[V]**
///
/// 51 `i32` entries indexed by tax rate. `Tax_RecomputePreview` reads
/// `g_taxHappinessOther[rate * 4]` into county `+0x16`, and
/// `Tax_SumEmpireHappiness` sums that across the realm into a signed *byte* -
///
///
/// The shape is the point, because it is nothing like a formula: **flat zero
/// through rate 19**, then a shallow ramp reaching only −15 at the maximum.
/// Taxing at 19% costs your other counties nothing whatsoever.
///
/// Transcribed by hand from the executable, which is a step that can go wrong
/// silently - the first attempt was off by one at the start of the ramp. So
/// `tools/oracle/kingdom.ps1` checks all 51 entries against `Lords2.exe`, and
/// `the_tax_happiness_table_is_the_one_in_the_binary` asserts the shape here.
pub const TAX_HAPPINESS_OTHER: [i32; MAX_TAX_RATE as usize + 1] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //     0..9
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //    10..19
    -1, -1, -1, -1, //                  20..23
    -2, -2, -2, -2, //                  24..27
    -3, -3, -3, -3, //                  28..31
    -4, -4, -4, //                      32..34
    -5, -5, -5, //                      35..37
    -6, -6, //                          38..39
    -7, -7, //                          40..41
    -8, -8, //                          42..43
    -9, //                              44
    -10, -11, -12, -13, -14, -15, //    45..50
];

/// `0x004D89C0` - `(wood, stone)` to build, by castle type 1..=5.
pub const CASTLE_COST: [(i32, i32); 5] =
    [(400, 40), (800, 80), (200, 1000), (400, 2000), (800, 3000)];

/// `0x004D89E8` - the workforce a build consumes, by castle type 1..=5.
///
/// **`[V]` The table is two ints per castle level, not one**, and
/// `docs/kingdom.md` §7.5 prints one column: the bytes read
/// `200,200 400,400 800,800 1500,1500 2500,2500`. Ten ints is 40 bytes and
/// `0x004D89E8 + 40` is exactly `0x004D8A10`, where the garrison caps begin, so
/// the stride is not in doubt. The values kingdom.md gives are right.
///
/// **What the second column means is unknown** and is deliberately not guessed
/// at. Both columns hold the same number in all five rows, so nothing here can
/// distinguish "a duplicate" from "a second quantity that happens to match".
pub const CASTLE_WORKFORCE: [(i32, i32); 5] =
    [(200, 200), (400, 400), (800, 800), (1500, 1500), (2500, 2500)];

/// `0x004D8A10` - garrison cap. **Six slots, five used and a trailing zero**;
/// the used ones are castle types 1..=5. `[V]` - and that 24-byte stride is
/// what puts [`CASTLE_TAX_BONUS_PCT`] at `0x004D8A28` and
/// [`CASTLE_FREE_ARCHERS`] at `0x004D8A40`.
pub const CASTLE_GARRISON_CAP: [i32; 6] = [150, 200, 200, 400, 600, 0];

/// `0x004D8A40` - free archers the castle comes with, by castle type 1..=5,
/// with the same trailing zero sixth slot. The manual: *"A new castle will
/// automatically include a garrison. Its size will vary according to the size
/// of the castle."*
///
/// `0x004D8A40 + 24` is `0x004D8A58`, which is the first byte of
/// [`AI_PERSONALITY_STRIDE`]'s first record - the arithmetic closes on both
/// sides.
pub const CASTLE_FREE_ARCHERS: [i32; 6] = [50, 150, 150, 200, 300, 0];

