use super::*;

/// `g_dairyPerHead` (`0x00553F60`) - the standing herd feeds five people per
/// head per season for free, without being slaughtered. `docs/kingdom.md`
/// §4.3, corroborated by a strategy guide and by a player's measurement of
/// 80 cows feeding 400 people.
pub const DAIRY_PER_HEAD: i32 = 5;

/// `g_foodPerHead` (`docs/kingdom.md` §4.3) - one slaughtered animal feeds ten.
pub const FOOD_PER_HEAD: i32 = 10;

/// `g_foodPerSack` (`docs/kingdom.md` §4.3) - one sack of grain feeds six.
pub const FOOD_PER_SACK: i32 = 6;

/// `g_grainYieldPerSack` (`0x0057C8E0`) = 12. Confirmed verbatim by the game's
/// own FAQ page, `L2.eng` group 292 index 4: *"Each sack planted will grow into
/// 12 sacks"*. `docs/kingdom.md` §7.1.
pub const GRAIN_YIELD_PER_SACK: i32 = 12;

/// `g_grainMaxSacksPerField` (`0x00552FFC`) = 10.
///
/// **The printed manual says 5, twice
/// `docs/decisions.md` C10 and `docs/kingdom.md` §7.1 and §11. Two independent
/// players measuring their own saves reported 6 fields sowing 60 sacks and 9
/// fields sowing 90.
pub const GRAIN_MAX_SACKS_PER_FIELD: i32 = 10;

/// The labour divisor in `Grain_Sow`'s "can this many sacks be tended?" test:
/// 5 with *Advanced Farming* on, 2 with it off. `docs/kingdom.md` §7.1.
///
/// Note the direction: the *smaller* divisor demands *more* labour, so turning
/// Advanced Farming off makes sowing harder, not easier.
pub const GRAIN_LABOUR_DIVISOR_ADVANCED: i32 = 5;
pub const GRAIN_LABOUR_DIVISOR_BASIC: i32 = 2;

/// `Grain_Grow` (`0x0044D15A`) and `Grain_Harvest` (`0x0044D1E5`) cap the crop
/// at `labour * this`, and their multiplier is **not** the sowing divisor.
///
/// `Rules_InitConstants` (`0x004983B7`) writes four numbers where this crate
/// had two: `g_grainLabourDivisorAdv = 5` (`0x005533BC`), `0x00553538 = 10`,
/// `0x00553218 = 3` and `g_grainLabourDivisor = 2` (`0x0057D34C`). Each of the
/// three grain steps picks between *its own* advanced value and the **same**
/// basic value, so with *Advanced Farming* off all three read
/// [`GRAIN_LABOUR_DIVISOR_BASIC`] — once as a divisor and twice as a
/// multiplier. That coincidence is the original's, and it is why there are two
/// new constants here and not four. **`[V]`**
pub const GRAIN_GROW_PER_WORKER_ADVANCED: i32 = 10;

/// …and the harvest's, which is far tighter: **three sacks a reaper**, out of a
/// workforce `Grain_Harvest` has already halved. `docs/kingdom.md` §7.1.
pub const GRAIN_HARVEST_PER_WORKER_ADVANCED: i32 = 3;

/// A field is fully reclaimed at 800, and `Field_ReclaimTick` (`0x0044C093`)
/// moves it by at most 200 a season. 200/800 is exactly the manual's *"you
/// will never be able to reclaim more than a quarter of a field in a single
/// season"*. `docs/kingdom.md` §7.2.
pub const FIELD_PROGRESS_MAX: i32 = 800;
pub const FIELD_RECLAIM_PER_SEASON: i32 = 200;

/// The random-event population modifier is capped at this percentage of the
/// county. `docs/kingdom.md` §5 and §8.1.
pub const EVENT_POPULATION_CAP_PCT: i32 = 20;

/// Random events are drawn only once the year passes this. `docs/kingdom.md`
/// §8.1.
pub const EVENT_FIRST_YEAR: i32 = 1268;

// ---------------------------------------------------------------------------
// Seasons
// ---------------------------------------------------------------------------

/// `g_deathRateBySeason` - percent, added to the health death rate.
/// `Spring 4, Summer 0, Autumn 2, Winter 8`. Index 0 unused.
///
/// `docs/kingdom.md` §9's reproduction needs `[4] == 8`, which is what pins
/// the array as 1-based and Winter as season 4.
pub const DEATH_RATE_BY_SEASON: [i32; 5] = [0, 4, 0, 2, 8];

/// The seasonal push on the per-county dryness accumulator: `+8, +24, +12,
/// -12` for Spring, Summer, Autumn, Winter. Index 0 unused.
/// `docs/kingdom.md` §7.3.
pub const DRYNESS_BY_SEASON: [i32; 5] = [0, 8, 24, 12, -12];

// ---------------------------------------------------------------------------
// Rations and health
// ---------------------------------------------------------------------------

/// The six ration levels, `L2.eng` group 21. `docs/kingdom.md` §4.2.
pub const RATION_LEVEL_COUNT: usize = 6;

/// `g_rationTable` (`0x004D6738`) - six `{divisor, multiplier}` pairs. The
/// requirement is `DivCeil(people, divisor) * multiplier`.
///
/// | level | pair | food needed | `L2.eng` group 21 |
/// |---:|---|---|---|
/// | 0 | (1, 0) | none    | None |
/// | 1 | (4, 1) | pop / 4 | Quarter |
/// | 2 | (2, 1) | pop / 2 | Half |
/// | 3 | (1, 1) | pop     | Normal |
/// | 4 | (1, 2) | pop x 2 | Double |
/// | 5 | (1, 3) | pop x 3 | Triple |
pub const RATION_TABLE: [(i32, i32); RATION_LEVEL_COUNT] =
    [(1, 0), (4, 1), (2, 1), (1, 1), (1, 2), (1, 3)];

pub const RATION_NAMES: [&str; RATION_LEVEL_COUNT] =
    ["None", "Quarter", "Half", "Normal", "Double", "Triple"];

/// The happiness a ration level is worth: the single expression `3L - 8` at the
/// end of `Ration_Apply`, not a table. `docs/kingdom.md` §4.2 and §10.
///
/// Normal (3) is the first positive row, which is exactly what the manual says.
#[inline]
pub const fn ration_happiness(level: i32) -> i32 {
    3 * level - 8
}

/// The five health bands, `L2.eng` group 20: `Diseased, Sick, Average, Good,
/// Perfect`.
pub const HEALTH_BAND_COUNT: usize = 5;

pub const HEALTH_BAND_NAMES: [&str; HEALTH_BAND_COUNT] =
    ["Diseased", "Sick", "Average", "Good", "Perfect"];

/// `g_healthDeltaTable` (`0x004D64A8`) - `int[6][5]`, indexed
/// `[rationLevel][healthBand]`, added to the health meter each season.
///
/// The sign pattern in the last column is the rule players feel:
/// **Perfect health decays under anything less than Double rations.**
pub const HEALTH_DELTA: [[i32; HEALTH_BAND_COUNT]; RATION_LEVEL_COUNT] = [
    // Diseased  Sick  Average  Good  Perfect
    [-8, -10, -13, -16, -20], // None
    [-4, -6, -9, -12, -15],   // Quarter
    [-2, -4, -6, -8, -12],    // Half
    [8, 4, 2, 1, -1],         // Normal
    [12, 8, 4, 2, 0],         // Double
    [20, 12, 6, 3, 1],        // Triple
];

/// `g_healthBandLadder` (`0x004D6520`) - **five `{threshold, band}` pairs**,
/// `(10,0) (35,1) (65,2) (90,3) (100,4)`.
///
/// **`[V]` The pair layout is read out of the bytes, and `docs/kingdom.md`
/// §4.2's "else 4" is wrong**: the last case is an explicit `(100, 4)` row, not
/// a fallthrough. Two independent checks pin it. Five pairs of `int` is 40
/// bytes and `0x004D6520 + 40` is exactly `0x004D6548`, where
/// `g_healthHappiness` begins; and the interleaved 0,1,2,3,4 cannot be a
/// threshold array, because thresholds do not decrease.
///
/// The comparison sense is *inclusive*:
/// `docs/kingdom.md` §9 needs a starting meter of 65 to band as **2**, which
/// only happens if 65 is `<= 65`.
///
/// **No behaviour changes with the correction**, because the meter is clamped
/// to 0..=100 before it is banded and so never falls off the end.
pub const HEALTH_BAND_LADDER: [(i32, i32); HEALTH_BAND_COUNT] =
    [(10, 0), (35, 1), (65, 2), (90, 3), (100, 4)];

/// Band a health meter through [`HEALTH_BAND_LADDER`].
#[inline]
pub fn health_band(meter: i32) -> u8 {
    let mut i = 0;
    while i < HEALTH_BAND_LADDER.len() {
        if meter <= HEALTH_BAND_LADDER[i].0 {
            return HEALTH_BAND_LADDER[i].1 as u8;
        }
        i += 1;
    }
    HEALTH_BAND_LADDER[HEALTH_BAND_LADDER.len() - 1].1 as u8
}

/// `g_healthHappiness` (`0x004D6548`) - the happiness a health band is worth
/// per season, indexed by band 0..=4.
///
/// The array in the binary has **six** slots, the sixth zero: `0x004D6548 + 24`
/// is exactly `0x004D6560`, where `g_herdWeatherPct` begins. The same
/// five-used-plus-a-trailing-zero shape carries the three castle tables. Only
/// the five used slots are carried here.
pub const HEALTH_HAPPINESS: [i32; HEALTH_BAND_COUNT] = [-10, -5, 0, 1, 2];

/// `g_deathRateByHealth` - percent, by band 0..=4. Added to
/// [`DEATH_RATE_BY_SEASON`]. A Diseased county in winter loses 43% of its
/// people in one season.
pub const DEATH_RATE_BY_HEALTH: [i32; HEALTH_BAND_COUNT] = [35, 20, 8, 3, 0];

// ---------------------------------------------------------------------------
// Population
// ---------------------------------------------------------------------------

/// `g_birthRateLadder` (`0x004D6308`) - twenty `{population, percent}` pairs.
/// The first row whose population threshold is `>=` the county's population
/// gives the base birth rate; above the last row the last percent is used.
///
/// This is the "soft population cap" players describe: at 2,000 people the
/// birth rate is 2%, which cannot keep up with Winter's 8% death rate.
pub const BIRTH_RATE_LADDER: [(i32, i32); 20] = [
    (40, 100),
    (80, 70),
    (100, 50),
    (250, 30),
    (500, 20),
    (700, 15),
    (800, 14),
    (900, 13),
    (1000, 12),
    (1100, 11),
    (1200, 10),
    (1300, 9),
    (1400, 8),
    (1500, 7),
    (1600, 6),
    (1700, 5),
    (1800, 4),
    (1900, 3),
    (2000, 2),
    (3000, 1),
];

/// `Table_Lookup(pop, g_birthRateLadder, 20, 1)`. `docs/kingdom.md` §5.1.
#[inline]
pub fn birth_rate(population: i32) -> i32 {
    let mut i = 0;
    while i < BIRTH_RATE_LADDER.len() {
        if population <= BIRTH_RATE_LADDER[i].0 {
            return BIRTH_RATE_LADDER[i].1;
        }
        i += 1;
    }
    BIRTH_RATE_LADDER[BIRTH_RATE_LADDER.len() - 1].1
}

/// The happiness band that scales the birth rate:
/// `<26 -> 25, <51 -> 50, <76 -> 75, <100 -> 100, else 120`.
/// `docs/kingdom.md` §5.
#[inline]
pub fn happiness_birth_factor(happiness: i32) -> i32 {
    if happiness < 26 {
        25
    } else if happiness < 51 {
        50
    } else if happiness < 76 {
        75
    } else if happiness < 100 {
        100
    } else {
        120
    }
}

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

/// The flat efficiency every industry runs at when *Advanced Farming* is off.
///
/// **`[V]`** - `FUN_0044F248`'s first line is
/// `if (g_optAdvancedFarming == 0) return 80;`. The 15% and 20% bases, and the
/// whole ramp, only exist in an Advanced Farming game. Nothing in
/// `docs/kingdom.md` §7.4 says so, and the England turn-one fixture has the option **off**,
/// so the FAQ's *"30 serfs working at 15% efficiency"* describes the advanced
/// game only.
pub const EFFICIENCY_WITHOUT_ADVANCED_FARMING: i32 = 80;

/// The efficiency ceiling in `FUN_0044F248`.
///
/// **`[V]`, and in the instruction stream**: the ramp
/// ends `CMP dword ptr [ebp-0x0C], 0x64` / `MOV dword ptr [ebp-0x0C], 0x64` at
/// `0x0044F2E8`, and the flat 80 above is the `MOV EAX, 0x50` at `0x0044F278`.
/// `tools/oracle/kingdom.ps1` recovers both immediates out of `.text` — the
/// technique `docs/decisions.md` C16 established, applied to a rule rather
/// than to a `Rules_InitConstants` global.
pub const EFFICIENCY_MAX: i32 = 100;

/// What `FUN_0044EF4E` returns as "no limit" for an enabled non-weapon
/// industry. It is a literal 999,
/// with enough workers really is capped at 999 units a season.
pub const RESOURCE_LIMIT_UNLIMITED: i32 = 999;

/// The six weapon types, in the order `g_weaponCost` stores them.
pub const WEAPON_TYPE_COUNT: usize = 6;

pub const WEAPON_NAMES: [&str; WEAPON_TYPE_COUNT] =
    ["Crossbow", "Mace", "Sword", "Pike", "Bow", "Armour"];

/// `g_weaponCost` (`0x004D8990`) - six `{wood, iron}` pairs. Identical to two
/// independently published tables. `docs/kingdom.md` §7.4.
pub const WEAPON_COST: [(i32, i32); WEAPON_TYPE_COUNT] =
    [(6, 10), (4, 4), (3, 10), (6, 3), (13, 0), (4, 18)];

// ---------------------------------------------------------------------------
// Wages
// ---------------------------------------------------------------------------

/// `Wages_ForUnit` (`0x004AD52B`): a human pays `men / 4`; an AI pays
/// `men / 3`, `men / 5` or `men / 10` by difficulty.
///
/// **Troop type does not enter it: a knight and a peasant cost the same.** A
/// player measured 250 men -> 62 crowns, 252 -> 63, 254 -> 63, across armies of
/// knights, of peasants and of mixed troops.
pub const WAGE_DIVISOR_HUMAN: i32 = 4;
pub const WAGE_DIVISOR_AI: [i32; 3] = [3, 5, 10];

/// The bankruptcy escalation in `Wages_PayAll` runs 0..=5. What each stage
/// *does* was not traced (`docs/kingdom.md` §2, `+0x158`, marked `[D]`).
pub const BANKRUPT_STAGE_MAX: u8 = 5;

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

/// `g_aiGoldGrantSmall` (`0x004DC230`) - the same shape, used instead when the
/// realm holds **fewer than three** counties. `[V]`
///
/// Uniformly smaller than [`AI_GOLD_GRANT`], which reads the opposite way round
/// to what "help the loser" would suggest: a cornered AI is given *less*, not
/// more. The same direction as the population/herd/grain grant, which stops
/// entirely once a realm has five counties - the grants reward a realm that is
/// already doing well.
pub const AI_GOLD_GRANT_SMALL: [[i32; 4]; 5] = [
    [0, 0, 0, 0],         // lord 0 - the human
    [0, 160, 250, 400],   // lord 1
    [40, 180, 300, 500],  // lord 2
    [0, 160, 250, 400],   // lord 3
    [100, 240, 400, 600], // lord 4
];

/// A realm with fewer than this many counties draws from
/// [`AI_GOLD_GRANT_SMALL`]. `[V]` - `docs/kingdom.md` §8.2 states it.
pub const AI_GOLD_GRANT_SMALL_COUNTIES: u8 = 3;

/// The AI's free population, herd and grain per county per season, as
/// `(population, herd, grain)` multipliers on the difficulty.
///
/// **`[V]`, and `docs/kingdom.md` §8.2 is wrong to state one tier.** It gives
/// *"`difficulty * 20` people, `difficulty * 5` head and `difficulty * 40`
/// sacks"* flatly. `AI_SetTaxRates` picks the tier from the realm's county
/// count (`+0x29`):
///
/// | realm counties | people | head | sacks |
/// |---|---:|---:|---:|
/// | 1 … 2 | `d * 20` | `d * 5` | `d * 40` |
/// | 3 … 4 | `d * 10` | `d * 2` | `d * 20` |
/// | 5 or more | **0** | **0** | **0** |
///
/// So the figures the document quotes are the *smallest* realm's tier, and a
/// realm with five counties gets no goods grant at all. A realm with zero
/// counties is gated out one level up and gets nothing either, gold included.
pub const AI_GRANT_TIERS: [(u8, i32, i32, i32); 2] = [(3, 20, 5, 40), (5, 10, 2, 20)];

/// The tier's per-difficulty multipliers for a realm holding `counties`
/// counties, or `(0, 0, 0)` above the last tier.
#[inline]
pub fn ai_grant_tier(counties: u8) -> (i32, i32, i32) {
    let mut i = 0;
    while i < AI_GRANT_TIERS.len() {
        let (below, p, h, g) = AI_GRANT_TIERS[i];
        if counties < below {
            return (p, h, g);
        }
        i += 1;
    }
    (0, 0, 0)
}

/// Kept for the documentation's own figures, which are the first tier's.
pub const AI_GRANT_POPULATION_PER_DIFFICULTY: i32 = AI_GRANT_TIERS[0].1;
pub const AI_GRANT_HERD_PER_DIFFICULTY: i32 = AI_GRANT_TIERS[0].2;
pub const AI_GRANT_GRAIN_PER_DIFFICULTY: i32 = AI_GRANT_TIERS[0].3;

/// The grant is gated on the county *already having some*, so it compounds
///.
pub const AI_GRANT_MIN_POPULATION: i32 = 20;
pub const AI_GRANT_MIN_HERD: i32 = 10;
pub const AI_GRANT_MIN_GRAIN: i32 = 50;

/// Buying ale is worth one happiness per this percentage of the county's
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

/// The history ring `Season_Advance`'s second-to-last pass (`FUN_004AE7DD`)
/// writes: **400 seasons x 16 counties x `{i32 population, i8 happiness}`**.
///
/// **`[V]`, and the length invariant closes it.** Save block 10 is
/// `0x0056D8C0` for **51,200** bytes, and `400 * 16 * 8` is exactly 51,200.
/// The write is `[head << 7 + county * 8 + 0x0056D8B8]`, so county `k` lands at
/// slot `k - 1` of the entry — the eight-byte offset between the block's base
/// and the instruction's is what makes the 1-based county array fit a 0-based
/// ring without spilling.
///
/// `docs/kingdom.md` §3.4 names this pass and nothing more.
pub const HISTORY_SEASONS: usize = 400;

/// The ring stores 16 county slots per season, which is [`MAX_COUNTY_ID`]
/// counties — the loop runs `for (c = 1; c < 0x11; c++)`.
///
/// [`MAX_COUNTY_ID`]: crate::county::MAX_COUNTY_ID
pub const HISTORY_COUNTIES: usize = 16;

// ---------------------------------------------------------------------------
// Score
// ---------------------------------------------------------------------------

/// The gold brackets in `Score_RankRealms` (`0x0049AA0E`) — the one term of the
/// score whose meaning is unambiguous. `docs/kingdom.md` §8.3.
///
/// # The ladder in the shipped executable is broken, and this reproduces it
///
/// **`[V]`, from the bytes.** It reads as three thresholds paying 50, 100 and
/// 200, and this function used to implement that. It is not what the binary
/// does. `0x0049AED1` onwards, disassembled by hand:
///
/// ```text
/// 0049aed1  81 b8 18c05700 d0070000   cmp  [eax+57c018], 2000
/// 0049aedb  0f 8e 1a000000            jle  0049aefb
/// 0049aee1  83 80 50bf5700 32         add  [eax+57bf50], 50      ; gold > 2000
/// 0049aef6  e9 6e000000               jmp  0049af69              ; next realm
/// 0049aefb  ...  cmp  [eax+57c018], 5000
/// 0049af13  0f 8e 1a000000            jle  0049af33
/// 0049af19  ...  add  [eax+57bf50], 100                          ; unreachable
/// 0049af33  ...  cmp  [eax+57c018], 10000
/// 0049af4b  0f 8e 18000000            jle  0049af69
/// 0049af51  ...  add  [eax+57bf50], 200                          ; unreachable
/// ```
///
/// The ladder is tested **smallest threshold first**: anything over 2,000 takes
/// the 50 and jumps to the next realm, and the 5,000 and 10,000 arms are only
/// reached by a treasury that has already failed `> 2000`. So the two richest
/// brackets are dead code and the shipped rule is:
///
/// | gold | bonus |
/// |---|---:|
/// | 0 … 2,000 | 0 |
/// | 2,001 and up | **50** |
///
/// A treasury is therefore worth **one castle**, not four, and `docs/rules.md`'s
/// *"hoarding past 10,000 adds nothing at all"* is true a great deal earlier than
/// it says. The table keeps its three thresholds
/// designed ladder is three numbers away — see [`Tables::DEFAULT`].
#[inline]
pub fn score_gold_bracket(gold: i32) -> i32 {
    if gold > 2_000 {
        50
    } else {
        0
    }
}

/// The six weights `Score_RankRealms` applies to six realm fields **none of
/// which `docs/kingdom.md` §8.3 could identify**. They are expressed as a
/// numerator and a denominator so that `+0x10 / 10` and `+0x54 / 5` are not
/// silently turned into rounded multiplications.
///
/// `(numerator, denominator)` for realm fields `+0x60, +0x10, +0x0C, +0x58,
/// +0x54, +0x4C` in that order.
pub const SCORE_WEIGHTS: [(i32, i32); 6] = [(10, 1), (1, 10), (2, 1), (2, 1), (1, 5), (50, 1)];

/// The offsets of the six score inputs, in the order [`SCORE_WEIGHTS`] applies.
///
/// `docs/kingdom.md` §8.3 says **none** of the six was identified. Five now
/// are, from `FUN_0049D1E0` — the AI turn's fourteenth step, which recomputes
/// exactly these realm fields once a turn:
///
/// | offset | weight | what `FUN_0049D1E0` writes there |
/// |---|---|---|
/// | `+0x60` | x10 | `PctOf(ownedCounties, g_countyCount)` — **share of the map**, 0..100 |
/// | `+0x10` | /10 | total population over the realm's counties |
/// | `+0x0C` | x2 | mean happiness over the realm's counties |
/// | `+0x58` | x2 | mean health meter over the realm's counties |
/// | `+0x54` | /5 | total men over the realm's armies |
/// | `+0x4C` | x50 | **castles held** — see below |
///
/// All six are identified, and the score reads as *castles above everything
/// else, then territory, then people, then how well they are doing, then the
/// army*. `[V]`
///
/// `+0x4C` was the last, and was found from the other end. A player described
/// the standings screen — swords and flags at differing heights, each with a
/// voice-over — and `L2.eng` group 35 is exactly that list: *Most counties,
/// Most castles, Most troops, Most crowns, Happiest people, Most people,
/// Greatest noble, undecided.* Five already had an input; **Most castles** did
/// not, and `+0x4C` had no name.
///
/// The coincidence is not the evidence; this is:
///
/// ```c
/// for (realm = 1; realm < 6; realm++) realm[0x4C] = 0;
/// for each county:
///     if (county[0x1C3] == 0)          /* not degraded  */
///         if (county[0x1C0] != 0)      /* castleType    */
///             realm[owner][0x4C]++;    /* count castles */
/// ```
///
/// `0x0053FB70 - 0x0053F9B0 = 0x1C0`, which is `castleType`. So it counts the
/// realm's counties holding a castle, and it carries **more weight than the
/// other five combined** — a real statement about what this game thinks winning
/// is.
pub const SCORE_INPUT_OFFSETS: [u16; 6] = [0x60, 0x10, 0x0C, 0x58, 0x54, 0x4C];

/// The slot of [`SCORE_INPUT_OFFSETS`] that holds `+0x4C`, the castle count.
///
/// **It is the one slot [`crate::realm::Realm::sync_score_inputs`] must not
/// touch**, because in the original it is not one of `Realm_UpdateTotals`'
/// writes: `Castle_BuildTick` (`0x004508DE`) owns it, alone. Verified
/// exhaustively — every instruction in `Lords2.exe` whose
/// operand mentions `g_realms + 0x4C` is one of seven, and they are
/// `Castle_BuildTick` twice (`0x0045090E` clears, `0x00450CB1` increments),
/// `Game_SetupRealmsAndCounties` once (`0x0049C14C`, the initial clear),
/// `Score_RankRealms` three times and one painter.
///
/// The timing that difference buys is real and is why this is stored
/// derived at scoring time. `Castle_BuildTick` is a *season* pass; nothing
/// between one season and the next rewrites the count,
/// by a siege in phase 2, or a county that changes hands, still scores its 50
/// until the next `Castle_BuildTick`. A count derived inside `compute_score`
/// would drop it immediately, which is a different game.
pub const SCORE_INPUT_CASTLES: usize = 5;

/// Names for the six score inputs, in the order [`SCORE_WEIGHTS`] applies.
pub const SCORE_INPUT_NAMES: [&str; 6] = [
    "share of the map, percent",
    "total population",
    "mean county happiness",
    "mean county health",
    "total men under arms",
    "castles held",
];

#[cfg(test)]
mod tests {
    use super::*;

    /// The health ladder's comparison sense, pinned by `docs/kingdom.md` §9:
    /// a starting meter of 65 must band as 2, and 67 must band as 3.
    #[test]
    fn the_health_ladder_is_inclusive_at_every_step() {
        assert_eq!(health_band(65), 2, "65 <= 65 is band 2 - the save says so");
        assert_eq!(health_band(66), 3);
        assert_eq!(health_band(67), 3, "the save's stored healthBand");
        assert_eq!(health_band(0), 0);
        assert_eq!(health_band(10), 0);
        assert_eq!(health_band(11), 1);
        assert_eq!(health_band(35), 1);
        assert_eq!(health_band(36), 2);
        assert_eq!(health_band(90), 3);
        assert_eq!(health_band(91), 4);
        assert_eq!(health_band(100), 4);
    }

    #[test]
    fn the_health_ladder_is_monotonic_over_its_whole_range() {
        let mut last = 0;
        for m in 0..=100 {
            let b = health_band(m);
            assert!(b >= last, "band went backwards at meter {m}");
            assert!(b < HEALTH_BAND_COUNT as u8);
            last = b;
        }
    }

    /// Every documented ration ratio, checked against its name.
    #[test]
    fn the_ration_table_matches_the_names_in_l2_eng_group_21() {
        let need = |level: usize, pop: i32| {
            let (d, m) = RATION_TABLE[level];
            crate::math::div_ceil(pop, d) * m
        };
        assert_eq!(need(0, 400), 0, "None");
        assert_eq!(need(1, 400), 100, "Quarter");
        assert_eq!(need(2, 400), 200, "Half");
        assert_eq!(need(3, 400), 400, "Normal");
        assert_eq!(need(4, 400), 800, "Double");
        assert_eq!(need(5, 400), 1200, "Triple");
    }

    /// The manual: *"A ration of Normal or above will improve happiness while
    /// half or quarter rations will decrease happiness"*. Normal is the first
/// positive row.
    #[test]
    fn normal_is_the_first_ration_level_worth_happiness() {
        for l in 0..3 {
            assert!(ration_happiness(l as i32) < 0, "level {l} should hurt");
        }
        for l in 3..RATION_LEVEL_COUNT {
            assert!(ration_happiness(l as i32) > 0, "level {l} should help");
        }
        assert_eq!(ration_happiness(3), 1);
        assert_eq!(ration_happiness(0), -8);
        assert_eq!(ration_happiness(5), 7);
    }

    /// The published FAQ, quoted in `docs/kingdom.md` §4.4: *"a county in good
    /// health (+1 happiness) with normal rations (+1) and 100 happiness can pay
    /// 7% taxes (-2) and remain at 100%. At perfect health, they can pay 8%."*
    ///
    /// This is the steady state of the whole layer in one assertion.
    #[test]
    fn the_break_even_tax_rate_is_seven_at_good_health_and_eight_at_perfect() {
        let steady = |band: usize, ration: i32, rate: i32| {
            (5 - rate) + HEALTH_HAPPINESS[band] + ration_happiness(ration)
        };
        assert_eq!(steady(3, 3, 7), 0, "Good health, Normal rations, 7%");
        assert_eq!(steady(4, 3, 8), 0, "Perfect health, Normal rations, 8%");
        assert!(steady(3, 3, 8) < 0, "8% is one too many at Good health");
        assert!(steady(4, 3, 9) < 0, "9% is one too many at Perfect health");
    }

    /// **Perfect health decays under anything less than Double rations** - the
    /// sign pattern in the delta table's last column.
    #[test]
    fn perfect_health_needs_double_rations_to_hold() {
        for level in 0..RATION_LEVEL_COUNT {
            let d = HEALTH_DELTA[level][4];
            if level < 4 {
                assert!(d < 0, "ration {level} should let Perfect decay, got {d}");
            } else {
                assert!(d >= 0, "ration {level} should hold Perfect, got {d}");
            }
        }
    }

    /// Recovering a starved county is fast at the bottom and slow at the top,
    /// in both directions: every row of the delta table is non-increasing.
    #[test]
    fn every_health_delta_row_is_non_increasing_across_the_bands() {
        for level in 0..RATION_LEVEL_COUNT {
            let row = HEALTH_DELTA[level];
            for b in 1..HEALTH_BAND_COUNT {
                assert!(
                    row[b] <= row[b - 1],
                    "ration {level} band {b}: {} should not exceed {}",
                    row[b],
                    row[b - 1]
                );
            }
        }
    }

    #[test]
    fn the_castle_tax_bases_are_the_published_bonus_percentages() {
        for t in 1..CASTLE_TYPE_COUNT {
            let bonus = CASTLE_TAX_BONUS_PCT[t - 1];
            assert_eq!(
                CASTLE_TAX_BASE[t] * 100,
                CASTLE_TAX_BASE[0] * (100 + bonus),
                "castle {t}: {} should be {}% above {}",
                CASTLE_TAX_BASE[t],
                bonus,
                CASTLE_TAX_BASE[0]
            );
        }
    }

    #[test]
    fn the_birth_ladder_is_a_decreasing_staircase() {
        for i in 1..BIRTH_RATE_LADDER.len() {
            assert!(BIRTH_RATE_LADDER[i].0 > BIRTH_RATE_LADDER[i - 1].0);
            assert!(BIRTH_RATE_LADDER[i].1 < BIRTH_RATE_LADDER[i - 1].1);
        }
        assert_eq!(birth_rate(1), 100);
        assert_eq!(birth_rate(40), 100);
        assert_eq!(birth_rate(41), 70);
        assert_eq!(birth_rate(417), 20, "docs/kingdom.md 9: pop 417 -> 20%");
        assert_eq!(birth_rate(500), 20);
        assert_eq!(birth_rate(501), 15);
        assert_eq!(birth_rate(2000), 2);
        assert_eq!(birth_rate(3000), 1);
        assert_eq!(birth_rate(50_000), 1, "above the last row the last row holds");
    }

    /// The soft cap players describe: at 2,000 people a Winter birth rate
    /// cannot keep up with a Winter death rate even at Perfect health.
    #[test]
    fn two_thousand_people_cannot_outbreed_winter() {
        let winter_deaths = DEATH_RATE_BY_HEALTH[4] + DEATH_RATE_BY_SEASON[Season::Winter as usize];
        assert_eq!(winter_deaths, 8);
        assert!(birth_rate(2000) < winter_deaths);
        assert!(birth_rate(500) > winter_deaths, "a small county still grows");
    }

    #[test]
    fn only_sunny_helps_the_herd_and_only_cloudy_is_neutral() {
        assert!(HERD_WEATHER_PCT[Weather::Sunny as usize] > 0);
        assert_eq!(HERD_WEATHER_PCT[Weather::Cloudy as usize], 0);
        for w in Weather::ALL {
            if w != Weather::Sunny && w != Weather::Cloudy {
                assert!(HERD_WEATHER_PCT[w as usize] < 0, "{} should hurt", w.name());
            }
        }
    }

    #[test]
    fn a_bow_costs_no_iron_and_armour_costs_the_most() {
        assert_eq!(WEAPON_COST[4], (13, 0), "bows need no iron");
        let dearest = WEAPON_COST.iter().map(|(w, i)| w + i).max().unwrap();
        assert_eq!(WEAPON_COST[5].0 + WEAPON_COST[5].1, dearest, "armour");
    }

    #[test]
    fn sheep_and_wool_are_the_two_goods_priced_zero() {
        let zeros: Vec<usize> =
            (1..GOOD_SELL_PRICE.len()).filter(|&i| GOOD_SELL_PRICE[i] == 0).collect();
        assert_eq!(zeros, vec![3, 5]);
        assert_eq!(GOOD_NAMES[3], "Sheep");
        assert_eq!(GOOD_NAMES[5], "Wool");
    }

    /// The human's `lord` byte is 0, and row 0 is all zeros.
    #[test]
    fn the_human_gets_no_gold_grant() {
        assert_eq!(AI_GOLD_GRANT[0], [0; 4]);
    }

    #[test]
    fn reclaiming_a_field_takes_exactly_four_seasons() {
        assert_eq!(FIELD_PROGRESS_MAX / FIELD_RECLAIM_PER_SEASON, 4);
    }
}

// ---------------------------------------------------------------------------
// The whole thing, as one value
// ---------------------------------------------------------------------------

