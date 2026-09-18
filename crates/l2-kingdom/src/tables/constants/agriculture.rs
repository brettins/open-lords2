#![allow(unused_imports)]
use super::*;
use super::demographics::*;
use super::castle_and_tax::*;
use super::jobs_and_goods::*;
use super::ai_and_grants::*;
use super::military_and_movement::*;
use super::scoring::*;
use super::*;


/// `g_herdWeatherPct` (`0x004D6560`) - the percentage swing the weather puts on
/// the herd, indexed by [`Weather`].
///
/// The signs match `L2.eng` group 66's own descriptions one for one: only
/// *Sunny* is *"Boosts growing crops"*, only *Cloudy* is *"Little effect on
/// farming"*
pub const HERD_WEATHER_PCT: [i32; 6] = [-2, -10, 5, 0, -5, -10];


/// **Three labourers a head is full staffing.** `[V]` - `FUN_0044DA99`
/// (`0x0044DA99`) opens with `PctOf(labour, herd * 3)`, which is the staffing
/// percentage every other term in the function is scaled by.
pub const HERD_LABOUR_PER_HEAD: i32 = 3;

pub const HERD_STAFFING_MAX: i32 = 200;

/// The binary writes it as `-((staffing - 100) / 3)`, which is **not** the same
/// as `(100 - staffing) / 3` in C - both truncate towards zero and the operand
/// is negated first, so the two agree. `[V]` and worth stating, because
/// `docs/kingdom.md` §13 annotates the expression as *"understaffed: negative"*
/// and it is positive: it is added to the **deaths**, not to the growth.
pub const HERD_UNDERSTAFFING_DIVISOR: i32 = 3;

pub const HERD_CROWDING_COUNT: usize = 4;

/// The density `FUN_0044D913` substitutes when a county has no pasture at all,
/// which lands it in the top band by a wide margin before the explicit
/// no-pasture override does the same thing again.
pub const HERD_NO_PASTURE_DENSITY: i32 = 1000;

pub const HERD_NO_PASTURE_KILL_ALL_BELOW: i32 = 6;
pub const HERD_NO_PASTURE_DIVISOR: i32 = 2;

/// * `level` - what `FUN_0044D913` stores in county `+0x25C`, and what
///   `L2.eng` group 77 names *"Low herd crowding."*, *"Average herd
///   crowding."*, *"Herd overcrowded."* and *"Massive overcrowding!!"*.
pub const HERD_CROWDING: [(i32, i32, i32, i32); HERD_CROWDING_COUNT] =
    [(10, 10, 1, 1400), (20, 20, 3, 900), (30, 30, 5, 500), (i32::MAX, 40, 7, 200)];

pub const HERD_SMALL_BONUS: [(i32, i32); 3] = [(5, 10_000), (10, 5_000), (25, 2_000)];

pub const HERD_CALVING_SEASON: u8 = Season::Spring as u8;
pub const HERD_CULLING_SEASON: u8 = Season::Winter as u8;
pub const HERD_SEASON_BONUS: (i32, i32) = (3, 2);

