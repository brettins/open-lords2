#![allow(unused_imports)]
use super::*;
use super::demographics::*;
use super::castle_and_tax::*;
use super::jobs_and_goods::*;
use super::ai_and_grants::*;
use super::military_and_movement::*;
use super::scoring::*;
use super::*;

// ---------------------------------------------------------------------------
// Weather
// ---------------------------------------------------------------------------

/// `g_herdWeatherPct` (`0x004D6560`) - the percentage swing the weather puts on
/// the herd, indexed by [`Weather`].
///
/// The signs match `L2.eng` group 66's own descriptions one for one: only
/// *Sunny* is *"Boosts growing crops"*, only *Cloudy* is *"Little effect on
/// farming"*
pub const HERD_WEATHER_PCT: [i32; 6] = [-2, -10, 5, 0, -5, -10];

// ---------------------------------------------------------------------------
// The herd's own births and deaths - `docs/kingdom.md` §13 and §13.1
// ---------------------------------------------------------------------------

/// **Three labourers a head is full staffing.** `[V]` - `FUN_0044DA99`
/// (`0x0044DA99`) opens with `PctOf(labour, herd * 3)`, which is the staffing
/// percentage every other term in the function is scaled by.
///
/// This is the rule a player describes as *"cows require more peasants to tend
/// them depending on how many cows there are"*, and `crates/l2-kingdom` did not
/// have it at all until `docs/kingdom.md` §13 was written.
pub const HERD_LABOUR_PER_HEAD: i32 = 3;

/// Staffing above this buys nothing. `if (199 < staffing) staffing = 200;` -
/// twice-staffed is the ceiling, and the comparison is against 199.
/// 200,
pub const HERD_STAFFING_MAX: i32 = 200;

/// Understaffing adds `(100 - staffing) / 3` to the death rate.
///
/// The binary writes it as `-((staffing - 100) / 3)`, which is **not** the same
/// as `(100 - staffing) / 3` in C - both truncate towards zero and the operand
/// is negated first, so the two agree. `[V]` and worth stating, because
/// `docs/kingdom.md` §13 annotates the expression as *"understaffed: negative"*
/// and it is positive: it is added to the **deaths**, not to the growth.
pub const HERD_UNDERSTAFFING_DIVISOR: i32 = 3;

/// The four crowding bands of `docs/kingdom.md` §13.1.
pub const HERD_CROWDING_COUNT: usize = 4;

/// The density `FUN_0044D913` substitutes when a county has no pasture at all,
/// which lands it in the top band by a wide margin before the explicit
/// no-pasture override does the same thing again.
pub const HERD_NO_PASTURE_DENSITY: i32 = 1000;

/// With no pasture the herd loses half its head - or **all** of them below six.
/// `deaths = (herd < 6) ? herd : herd / 2;`
pub const HERD_NO_PASTURE_KILL_ALL_BELOW: i32 = 6;
pub const HERD_NO_PASTURE_DIVISOR: i32 = 2;

/// `(density_max, level, death_rate, birth_rate)` for the four crowding bands.
///
/// * `density_max` - the highest `herd / fieldsCattle` in the band. The binary
///   tests `density < 11`, `< 21`, `< 31`, so the inclusive bounds are 10, 20,
/// 30 and everything above.
/// * `level` - what `FUN_0044D913` stores in county `+0x25C`, and what
///   `L2.eng` group 77 names *"Low herd crowding."*, *"Average herd
///   crowding."*, *"Herd overcrowded."* and *"Massive overcrowding!!"*.
/// * `death_rate` - per ten thousand head a season, before understaffing.
/// * `birth_rate` - per ten thousand head at 100% staffing, scaled by the
///   staffing percentage.
///
/// The two rate columns are the point of the whole table: an overcrowded herd
/// dies seven times as fast **and** breeds a seventh as often.
pub const HERD_CROWDING: [(i32, i32, i32, i32); HERD_CROWDING_COUNT] =
    [(10, 10, 1, 1400), (20, 20, 3, 900), (30, 30, 5, 500), (i32::MAX, 40, 7, 200)];

/// `(below, bonus)` added to the birth rate of a **small, fully staffed** herd.
///
/// Only when staffing has reached 100. A herd of four gets `+10000` per ten
/// thousand - a doubled birth rate - which is what stops a county that has lost
/// almost everything from being unable to recover.
pub const HERD_SMALL_BONUS: [(i32, i32); 3] = [(5, 10_000), (10, 5_000), (25, 2_000)];

/// The season whose calves arrive: `if (season == 1) births = births * 3 / 2;`
pub const HERD_CALVING_SEASON: u8 = Season::Spring as u8;
/// And the season that kills: `if (season == 4) deaths = deaths * 3 / 2;`
pub const HERD_CULLING_SEASON: u8 = Season::Winter as u8;
/// Both are exactly `* 3 / 2`, kept as a ratio.
pub const HERD_SEASON_BONUS: (i32, i32) = (3, 2);

