//! Every constant the kingdom layer turns on, with the address it was read
//! from in `docs/kingdom.md`.
//!
//! `docs/decisions.md` C11 is the reason this file exists as data rather than
//! as literals scattered through the rules: **no kingdom-layer rule is loaded
//! from a game data file**, so our engine has to carry the whole ruleset
//! itself. Keeping it in one module is what will eventually let `l2-mods`
//! override it.
//!
//! Nothing here is copied from the original. These are documented *facts* -
//! numbers, in the sense of `CLAUDE.md` rule 3 - transcribed from
//! `docs/kingdom.md` and re-typed here.
//!
//! Two conventions run through the file:
//!
//! * **Season-indexed arrays are 1-based**, with index 0 unused, because the
//!   original's are: `docs/kingdom.md` §9 pins `g_deathRateBySeason[4]
//!   = 8` for Winter, and `g_season` is used directly as an `L2.eng` group 29
//!   index where 0 is the string `No Season`.
//! * **County-indexed arrays are 1-based too**, for the same reason: record 0
//!   is never a county (§1).

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
/// **The printed manual says 5, twice, and the manual is wrong** - see
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

/// `g_season` is 1..=4 and indexes `L2.eng` group 29 directly:
/// `No Season, Spring, Summer, Autumn, Winter`. `docs/kingdom.md` §3.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Season {
    Spring = 1,
    Summer = 2,
    Autumn = 3,
    Winter = 4,
}

impl Season {
    pub const ALL: [Season; 4] = [Season::Spring, Season::Summer, Season::Autumn, Season::Winter];

    /// `None` for 0 (`No Season`) and for anything out of range.
    pub fn from_index(i: u8) -> Option<Season> {
        match i {
            1 => Some(Season::Spring),
            2 => Some(Season::Summer),
            3 => Some(Season::Autumn),
            4 => Some(Season::Winter),
            _ => None,
        }
    }

    pub fn index(self) -> u8 {
        self as u8
    }

    pub fn name(self) -> &'static str {
        match self {
            Season::Spring => "Spring",
            Season::Summer => "Summer",
            Season::Autumn => "Autumn",
            Season::Winter => "Winter",
        }
    }
}

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
/// The comparison sense is *inclusive*, and that is not a guess:
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

/// `L2.eng` group 66, six name/effect pairs, in this order.
/// `docs/kingdom.md` §7.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Weather {
    Frost = 0,
    Drought = 1,
    Sunny = 2,
    Cloudy = 3,
    Storms = 4,
    Flooding = 5,
}

impl Weather {
    pub const ALL: [Weather; 6] = [
        Weather::Frost,
        Weather::Drought,
        Weather::Sunny,
        Weather::Cloudy,
        Weather::Storms,
        Weather::Flooding,
    ];

    pub fn from_index(i: u8) -> Option<Weather> {
        Weather::ALL.get(i as usize).copied()
    }

    pub fn index(self) -> u8 {
        self as u8
    }

    pub fn name(self) -> &'static str {
        match self {
            Weather::Frost => "Frost",
            Weather::Drought => "Drought",
            Weather::Sunny => "Sunny",
            Weather::Cloudy => "Cloudy",
            Weather::Storms => "Storms",
            Weather::Flooding => "Flooding",
        }
    }
}

/// `g_herdWeatherPct` (`0x004D6560`) - the percentage swing the weather puts on
/// the herd, indexed by [`Weather`].
///
/// The signs match `L2.eng` group 66's own descriptions one for one: only
/// *Sunny* is *"Boosts growing crops"*, only *Cloudy* is *"Little effect on
/// farming"*, and the other four all say crops are lost.
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
/// twice-staffed is the ceiling, and the comparison is against 199 rather than
/// 200, so a staffing of exactly 199 is *not* rounded up.
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
///   30 and everything above.
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
/// Both are exactly `* 3 / 2`, kept as a ratio rather than as 150%.
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
/// so a large enough empire overflows it, which is a separate reproduced bug.
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

/// The four passes `Industry_Produce` (`0x0044EA92`) makes per county per
/// season, in the order it makes them. `docs/kingdom.md` §7.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Commodity {
    Wood = 0,
    Iron = 1,
    Weapons = 2,
    Stone = 3,
}

impl Commodity {
    pub const ALL: [Commodity; 4] =
        [Commodity::Wood, Commodity::Iron, Commodity::Weapons, Commodity::Stone];

    pub fn index(self) -> usize {
        self as usize
    }

    /// The `L2.eng` group 74 job whose workers this commodity draws on.
    pub fn job(self) -> usize {
        match self {
            Commodity::Wood => JOB_WOOD_CUTTING,
            Commodity::Iron => JOB_IRON_MINING,
            Commodity::Weapons => JOB_BLACKSMITH,
            Commodity::Stone => JOB_STONE_QUARRYING,
        }
    }

    /// The divisor applied to the worker count before the efficiency
    /// percentage. *"Iron and wood harvest at twice the quantity of stone"* is
    /// exactly this column.
    pub fn divisor(self) -> i32 {
        match self {
            Commodity::Wood | Commodity::Iron => 1,
            Commodity::Weapons => 4,
            Commodity::Stone => 2,
        }
    }

    /// Base efficiency percentage, the `param_4` the industry driver
    /// (`FUN_0044E852`) passes to each pass. The 15% also appears verbatim in a
    /// published FAQ (*"30 serfs working at 15% efficiency"*).
    ///
    /// It is the **increment** rather than the efficiency: see
    /// [`crate::industry::efficiency_ramp`]. With *Advanced Farming* off it is
    /// not used at all - the efficiency is a flat
    /// [`EFFICIENCY_WITHOUT_ADVANCED_FARMING`].
    pub fn base_efficiency(self) -> i32 {
        match self {
            Commodity::Wood => 20,
            _ => 15,
        }
    }
}

/// The order `Industry_Produce` is actually called in, from the driver
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

/// The nine assignable labour slots at county `+0xC4 + job*0x0C`.
///
/// **`[V]` and `docs/kingdom.md` §7.4's job column is wrong by one.** The
/// document reads the numbers straight off `L2.eng` group 74, which has *ten*
/// strings starting with *"Idle people"*; the record array has **nine** entries
/// and record `r` is group-74 string `r + 1`, because "idle people" is the
/// remainder rather than a job you can assign anyone to.
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
/// **`[V]`, and in the instruction stream rather than in `.data`**: the ramp
/// ends `CMP dword ptr [ebp-0x0C], 0x64` / `MOV dword ptr [ebp-0x0C], 0x64` at
/// `0x0044F2E8`, and the flat 80 above is the `MOV EAX, 0x50` at `0x0044F278`.
/// `tools/oracle/kingdom.ps1` recovers both immediates out of `.text` — the
/// technique `docs/decisions.md` C16 established, applied to a rule rather
/// than to a `Rules_InitConstants` global.
pub const EFFICIENCY_MAX: i32 = 100;

/// What `FUN_0044EF4E` returns as "no limit" for an enabled non-weapon
/// industry. It is a literal 999 rather than a saturating value, so a county
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
/// these, and the manual explains why: the merchant scroll shows `30/60`, sell
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
/// Note that rows 1 and 3 are **identical**, and that row 2 is the only row
/// that pays anything at difficulty 0. Lord number is not a difficulty ladder.
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
/// rather than rescues.
pub const AI_GRANT_MIN_POPULATION: i32 = 20;
pub const AI_GRANT_MIN_HERD: i32 = 10;
pub const AI_GRANT_MIN_GRAIN: i32 = 50;

// ---------------------------------------------------------------------------
// The AI's tax ladders
// ---------------------------------------------------------------------------

/// How many rungs a tax ladder has room for. The neutral ladder uses all
/// eight; the three personality ladders use five, five and six and pad the
/// rest. A size, not a balance figure.
pub const TAX_LADDER_RUNGS: usize = 8;

/// How many ladders the personality table chooses between.
pub const AI_TAX_LADDER_COUNT: usize = 3;

/// A tax ladder: `(happiness_below, rate)` rows, walked in order, taking the
/// first row the county's happiness is strictly below. The last row is the
/// fallback and its threshold is never tested.
///
/// **These are `if`/`else if` chains in the original, not a table** — there is
/// no address to read them from, and `tools/oracle/kingdom.ps1` recovers all
/// four out of `AI_SetTaxRates`' instruction stream instead: the thresholds
/// are the `CMP EAX, imm8` immediates and the rates are the
/// `MOV byte ptr [county+0xB9], imm8` stores, interleaved in source order.
pub type TaxLadder = [(i32, i32); TAX_LADDER_RUNGS];

/// The ladder `AI_SetTaxRates(0)` applies to **unowned** counties, which phase
/// 1 (`docs/kingdom.md` §3.1) runs every turn. `[V]`
///
/// Eight rungs, and the only ladder that goes above 12.
pub const AI_TAX_LADDER_NEUTRAL: TaxLadder =
    [(20, 0), (40, 1), (50, 2), (60, 3), (70, 4), (80, 6), (90, 8), (i32::MAX, 12)];

/// The three ladders an AI realm picks between, indexed by the *personality*
/// int at [`AI_PERSONALITY_TAX_LADDER`]. `[V]`
///
/// The unused rungs are padded by repeating the fallback threshold so all four
/// ladders have one shape; only the rows before `i32::MAX` are ever tested.
///
/// Ladder 0 is the greediest - 15% on a happy county. Ladder 2 is the gentlest
/// and the only one that charges nothing below 60 happiness.
pub const AI_TAX_LADDERS: [TaxLadder; AI_TAX_LADDER_COUNT] = [
    [(30, 0), (50, 2), (65, 4), (80, 10), (i32::MAX, 15), (i32::MAX, 15), (i32::MAX, 15), (i32::MAX, 15)],
    [(30, 0), (50, 1), (65, 3), (80, 7), (i32::MAX, 12), (i32::MAX, 12), (i32::MAX, 12), (i32::MAX, 12)],
    [(60, 0), (70, 1), (80, 2), (90, 3), (95, 8), (i32::MAX, 10), (i32::MAX, 10), (i32::MAX, 10)],
];

/// Walk a ladder. Happiness is a signed byte in the original and the
/// comparisons are signed, so a negative happiness lands on the first rung.
#[inline]
pub fn tax_rate_for(ladder: &TaxLadder, happiness: i32) -> i32 {
    let mut i = 0;
    while i < ladder.len() {
        if happiness < ladder[i].0 {
            return ladder[i].1;
        }
        i += 1;
    }
    ladder[ladder.len() - 1].1
}

/// `g_aiPersonality` (`0x004D8A58`) is records of this many bytes, one per AI
/// lord. `[I]` on the base and the stride: the code addresses it as
/// `base + (lord * 3 - 3) * 0x50`, i.e. three 0x50-byte rows per lord, and only
/// ever uses the first row. The arithmetic closes on the low side -
/// `0x004D8A40 + 24` (the free-archer table) is exactly `0x004D8A58`.
pub const AI_PERSONALITY_STRIDE: usize = 0xF0;

/// **Four** personality records, for lords 1..=4, indexed `lord - 1`.
///
/// `docs/kingdom.md` §2 says the lord byte runs *"1 … 5 for an AI lord"*, but
/// the table stops at four. A fifth record would begin at `0x004D8E18`, and
/// what is there does not fit the shape: the two fields below read 17 and 0
/// where every real record reads a farm style of 0, 1 or 9, and the field after
/// them reads 5000 where the four records read 100, 100, 200 and 50. So
/// `0x004D8E18` is taken to be **past the end of the table**, and this crate
/// refuses to answer for lord 5 rather than reproduce an out-of-bounds read of
/// bytes whose meaning is unknown. `[I]`, and it is the one thing about
/// `AI_SetTaxRates` still not settled.
pub const AI_PERSONALITY_COUNT: usize = 4;

/// The personality field at record `+0x04` that selects an [`AI_TAX_LADDERS`]
/// row. `[V]` for lords 1..=4 - three of the four AI lords tax on the same,
/// gentlest ladder and only lord 4 uses a different one.
pub const AI_PERSONALITY_TAX_LADDER: [usize; AI_PERSONALITY_COUNT] = [2, 2, 2, 1];

/// The personality field at record `+0x00` that `AI_ManageFields` copies into
/// county `+0x1FE` and dispatches on. `[V]` for lords 1..=4; 0, 1 and 9 are
/// exactly the three values the dispatch tests, which is a check on the field's
/// identity. **What each style does was not traced** - the three handlers
/// (`FUN_004A4052`, `FUN_004A42E3`, `FUN_004A440F`) allocate labour and are not
/// reproduced here.
pub const AI_PERSONALITY_FARM_STYLE: [u8; AI_PERSONALITY_COUNT] = [1, 1, 0, 9];

// --- the diplomacy half of the record, `docs/diplomacy.md` §8.4 -------------
//
// Every column below is `[V]`: read out of the four records in the file at
// 0x004D8A58, 0x004D8B48, 0x004D8C38, 0x004D8D28, and held against the binary
// by tools/oracle/kingdom.ps1. What each field *means* is `[D]` — one reader
// each, named in the doc comment.

/// Record `+0x08` — the increment a gift is judged against.
///
/// A gift under `bestGift + T/2` **costs** 8 standing and one at or above
/// `bestGift + T` gains 10, so `T` is both the price of a good gift and the
/// width of the band that insults. The Bishop's 50 makes him the cheapest lord
/// to please and the Countess's 200 the dearest.
/// Read by `Diplo_ReplyGift`. `docs/diplomacy.md` §3.1.
pub const AI_PERSONALITY_GIFT_INCREMENT: [i32; AI_PERSONALITY_COUNT] = [100, 100, 200, 50];

/// Record `+0x0C` — the base price of military help, multiplied by the pair's
/// `help_price_multiple`. Read by `Diplo_ReplyHelpRequest` and
/// `Diplo_ReplyAttackRequest`. §3.5.
pub const AI_PERSONALITY_HELP_PRICE: [i32; AI_PERSONALITY_COUNT] = [500, 1000, 1600, 1500];

/// Record `+0x10` — how much grudge an alliance survives.
///
/// The comparison is `tolerance < grudge`, so the Knight's 5 breaks on the
/// sixth point and the Bishop's 20 on the twenty-first. Read by `AI_Diplomacy`.
/// §4.1.
pub const AI_PERSONALITY_GRUDGE_TOLERANCE: [i32; AI_PERSONALITY_COUNT] = [5, 10, 15, 20];

/// Record `+0x14` — turns of courtship before an alliance is offered. The
/// Bishop moves in four turns where the Knight takes twelve. §4.1.
pub const AI_PERSONALITY_OFFER_INTERVAL: [i32; AI_PERSONALITY_COUNT] = [12, 10, 8, 4];

/// Record `+0x30` — the floor below which an ally will not move at all.
///
/// **`docs/diplomacy.md` §3.5 calls this a treasury floor and it is not one.**
/// `Diplo_ReplyHelpRequest` compares it against realm `+0x14`, which
/// [`crate::Realm::population_mean`] holds — the mean population of the
/// realm's counties, not its gold. §8.4's own note that the same field is
/// *"also a county-population floor at step 9"* is the corroboration: it is a
/// population floor in both places. See [`crate::diplomacy::reply_help_request`].
pub const AI_PERSONALITY_HELP_POPULATION_FLOOR: [i32; AI_PERSONALITY_COUNT] =
    [750, 800, 900, 1000];

/// Record `+0x40` — the percentage of a county's population one AI muster
/// conscripts. `[V]` on the values, `[D]` on the meaning; nothing in this
/// crate raises an army, so it is carried and not read. §8.2.
pub const AI_PERSONALITY_MUSTER_PCT: [i32; AI_PERSONALITY_COUNT] = [30, 30, 40, 50];

/// Record `+0x90` — how many castles a lord will have in progress at once.
///
/// The Bishop's **1** is half the story of §8.1: his money does not spread, so
/// whichever county he starts on gets the whole treasury's worth.
pub const AI_PERSONALITY_CASTLE_CONCURRENT: [i32; AI_PERSONALITY_COUNT] = [4, 3, 2, 1];

/// Record `+0xC8` — the county population a castle project needs before it is
/// started at all.
pub const AI_PERSONALITY_CASTLE_MIN_POPULATION: [i32; AI_PERSONALITY_COUNT] =
    [700, 650, 600, 600];

/// Record `+0xCC` … `+0xDC` — the treasury each castle type needs, by lord.
///
/// Columns are castle types 1..=5: palisade, motte & bailey, Norman keep, stone
/// castle, **royal castle**. `AI_BuildCastles` tests them from the top down and
/// takes the first the treasury clears.
///
/// **A zero is not a threshold of zero; it means the type is not offered to
/// that lord.** The original guards every rung with `threshold != 0 &&` before
/// the comparison, which is the only reason a zero row does not make every
/// lord build a palisade for nothing. See
/// [`AiPersonalityRow::castle_for_gold`].
///
/// The player's remark that *"the bishop always tries to build really big
/// castles"* is this table's last column: **2,000 crowns for the Bishop and
/// 10,000 for the Knight**, with the Baron and the Countess never reaching one
/// at any treasury. §8.1.
pub const AI_PERSONALITY_CASTLE_GOLD: [[i32; AI_CASTLE_LADDER_LEN]; AI_PERSONALITY_COUNT] = [
    [200, 0, 1000, 0, 10_000], // Knight
    [0, 500, 0, 4000, 0],      // Baron
    [0, 300, 0, 2000, 0],      // Countess
    [0, 0, 100, 0, 2000],      // Bishop
];

/// How many buildable castle types an AI chooses between — five, where
/// [`CASTLE_TYPE_COUNT`] is six because it counts *no castle* as type 0. A
/// size, not a balance figure.
pub const AI_CASTLE_LADDER_LEN: usize = 5;

/// The tax ladder an AI lord uses, or `None` when the lord byte names no
/// record: 0 is the human, 6 is an eliminated realm, and 5 is the value
/// [`AI_PERSONALITY_COUNT`] explains.
#[inline]
pub fn ai_tax_ladder(lord: u8) -> Option<&'static TaxLadder> {
    let index = (lord as usize).checked_sub(1)?;
    let row = *AI_PERSONALITY_TAX_LADDER.get(index)?;
    AI_TAX_LADDERS.get(row)
}

/// `AI_ManageFields` (`FUN_0049DD01`, AI step 5) adds a field to a county whose
/// field total is below the first threshold its population clears.
///
/// `(fields_below, population_above, fields_added)`, walked in order; the last
/// row has no field cap at all. `[V]`
pub const AI_FIELD_LADDER: [(i32, i32, i32); 6] = [
    (1, -1, 1),        // no fields at all: always add one
    (3, 200, 1),
    (5, 400, 1),
    (7, 600, 1),
    (9, 1000, 1),
    (i32::MAX, 1200, 2), // a big county adds two at once
];

// ---------------------------------------------------------------------------
// Ale, army and the history ring
// ---------------------------------------------------------------------------

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
/// `+0x219` other than this `+=` was found anywhere in the binary. So a county
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
/// domain 0..=101 rather than a balance figure: index 101 is the last slot
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
/// by address arithmetic rather than by this comment.
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
/// either would make the reproduction meaningless. Flagged rather than
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
// Every number below was read out of the instruction stream rather than out of
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
/// `INC` of `+0x153` rather than an `ADD`, which is why there is no immediate
/// to read and the oracle check tags the opcode instead of a value.
pub const STEP_COST_ROAD: i32 = 1;

/// Every step that is not a road step. `Unit_StepOnce`'s `ADD EAX, 3`.
pub const STEP_COST_OPEN: i32 = 3;

/// `Unit_CrossField` (`0x0046673C`) charges this **before** `Unit_StepOnce`'s
/// general `+3`, so a standing field costs `3 + 3 = 6` — and
/// `Move_BuildCostMap` stores a single literal `6` for the same tile. Two
/// unrelated codings landing on one number is what makes the field cost `[V]`.
pub const STEP_COST_FIELD_EXTRA: i32 = 3;

/// `Unit_TrampleTile` (`0x0046873F`) charges this for walking over a resource
/// site, and the move ends there.
pub const STEP_COST_TRAMPLE: i32 = 7;

/// What `Unit_TrampleTile` writes into the industry record's
/// `disabledSeasons`. **It always writes 3** — there is no ladder, and no
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
/// fewer loses none, so a small army stops shrinking rather than dying out.
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

/// The gold brackets in `Score_RankRealms` (`0x0049AA0E`) - the one term of the
/// score whose meaning is unambiguous. `docs/kingdom.md` §8.3.
#[inline]
pub fn score_gold_bracket(gold: i32) -> i32 {
    if gold > 10_000 {
        200
    } else if gold >= 5_001 {
        100
    } else if gold >= 2_001 {
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
    /// positive row and there is no zero row.
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

/// Every number above, gathered into a single plain value.
///
/// # Why this type exists
///
/// `docs/decisions.md` C11: **no kingdom rule is loaded from a game data
/// file.** Every economic constant lives in `Lords2.exe`, which is why modding
/// the original means patching a binary, and why an open engine is worth
/// building at all. Our engine therefore has to carry the whole ruleset
/// itself - and carrying it as `const` items makes it exactly as unreachable
/// as the 1996 binary made it.
///
/// So the same numbers are also available as one value of one type. A ruleset
/// loader can produce a `Tables`; a caller holding one is running on those
/// numbers and cannot tell where they came from.
///
/// # What this crate does *not* do
///
/// It does not read a file, parse a document, or know that mods exist. This is
/// plain data the simulation already understands; `l2-mods` is what builds one
/// out of a rule document, and the dependency points that way and never back.
/// A simulation that loads its own rules is a simulation that can fail to
/// load, and two lockstep peers that fail differently desync.
///
/// # Honest scope
///
/// [`Tables::DEFAULT`] is assembled *from the constants above*, which remain
/// the source of truth and keep their addresses and their evidence in their own
/// doc comments. What changed is that the simulation now reads the **table**:
/// [`crate::Kingdom`] carries one, [`crate::Kingdom::with_tables`] is how a
/// ruleset reaches it, and around thirty rule functions in this crate take
/// `&Tables` rather than a constant. `gathered_tests` holds the two readings
/// equal over their whole domain, so they cannot drift apart.
///
/// The ale ladder, the army-raising cost table, the efficiency ramp's bounds,
/// the AI's four tax ladders and its personality table were the last five
/// rules with no field here, and they now have one. **Every rule function in
/// this crate takes `&Tables`.**
///
/// One field is carried without being read: [`AiPersonalityRow::farm_style`].
/// `AI_ManageFields` dispatches on it into three labour allocators that were
/// never traced, so a ruleset can set it and nothing in this crate will
/// behave differently — which is said here, in the field's own doc comment and
/// in `docs/modding.md` §11 rather than left to be discovered.
///
/// Array *sizes* — [`JOB_COUNT`], [`RATION_LEVEL_COUNT`],
/// [`WEAPON_TYPE_COUNT`], [`ARMY_HAPPINESS_COST_LEN`], [`TAX_LADDER_RUNGS`],
/// [`AI_PERSONALITY_COUNT`] — are deliberately not fields: a ruleset that
/// changed one would be describing a different simulation rather than a
/// different balance, which is the same line `l2_sim::Troop::is_siege` draws on
/// the battle side. `docs/modding.md` §11 has the full division.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tables {
    pub food: FoodTable,
    pub grain: GrainTable,
    pub field: FieldTable,
    pub event: EventTable,
    /// Indexed by [`Season`] 1..=4; index 0 is the original's "No Season".
    pub season: [SeasonRow; 5],
    /// Indexed by ration level 0..=5.
    pub ration: [RationRow; RATION_LEVEL_COUNT],
    /// `ration_happiness(level) = slope * level + offset`. In the original this
    /// is not a table at all but the expression `3L - 8` at the end of
    /// `Ration_Apply` - an instruction, not data, which is the whole of C11 in
    /// one line.
    pub ration_happiness_slope: i32,
    pub ration_happiness_offset: i32,
    /// Indexed by health band 0..=4.
    pub health: [HealthBandRow; HEALTH_BAND_COUNT],
    /// `{inclusive upper bound, band}` pairs, in the binary's own layout at
    /// `0x004D6520`. Five of them, not four: the top band is an explicit
    /// `(100, 4)` entry rather than an `else`. Verified against the executable
    /// by `tools/oracle/kingdom.ps1`.
    pub health_band_ladder: [(i32, i32); HEALTH_BAND_COUNT],
    /// [`TAX_HAPPINESS_OTHER`], indexed by tax rate `0 ..= `[`MAX_TAX_RATE`].
    pub tax_happiness_other: [i32; MAX_TAX_RATE as usize + 1],
    pub population: PopulationTable,
    /// Indexed by [`Weather`].
    pub weather: [WeatherRow; 6],
    /// Staffing, crowding, and the births and deaths they produce.
    pub herd: HerdTable,
    pub castle: CastleTable,
    /// Indexed by [`Commodity`].
    pub commodity: [CommodityRow; 4],
    pub job: JobTable,
    /// Indexed by weapon type 0..=5.
    pub weapon: [WeaponRow; WEAPON_TYPE_COUNT],
    /// Indexed by `L2.eng` group 6 good id 0..=14; index 0 is unused.
    pub good: [GoodRow; 15],
    pub wages: WageTable,
    /// The efficiency ramp's two bounds — the ceiling it clamps to and the
    /// flat figure it returns with *Advanced Farming* off.
    pub efficiency: EfficiencyTable,
    pub ale: AleTable,
    /// `g_armyHappinessCost`, indexed by the percentage of the county taken.
    /// [`Tables::army_happiness_cost`] is the bounded read.
    pub army_happiness_cost: [i32; ARMY_HAPPINESS_COST_LEN],
    /// Movement, supply and desertion for a unit on the campaign map.
    /// `docs/armies.md`.
    pub unit: UnitTable,
    pub ai: AiTable,
    pub score: ScoreTable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FoodTable {
    pub dairy_per_head: i32,
    pub food_per_head: i32,
    pub food_per_sack: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrainTable {
    pub yield_per_sack: i32,
    pub max_sacks_per_field: i32,
    pub labour_divisor_advanced: i32,
    pub labour_divisor_basic: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldTable {
    pub progress_max: i32,
    pub reclaim_per_season: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventTable {
    pub population_cap_pct: i32,
    pub first_year: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeasonRow {
    pub death_rate: i32,
    pub dryness: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RationRow {
    pub divisor: i32,
    pub multiplier: i32,
    /// Added to the health meter, by health band 0..=4.
    pub health_delta: [i32; HEALTH_BAND_COUNT],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HealthBandRow {
    pub happiness: i32,
    pub death_rate: i32,
}

/// The two ladders the population rules walk.
///
/// Both are `(threshold, value)` in order, and in both the final row is the
/// catch-all whose threshold is never consulted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PopulationTable {
    /// `population <= up_to` gives `percent`. Twenty rows.
    pub birth_rate_ladder: [(i32, i32); 20],
    /// `happiness < below` gives `percent`. Five rows.
    pub happiness_factor_ladder: [(i32, i32); 5],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeatherRow {
    pub herd_pct: i32,
}

/// One crowding band. See [`HERD_CROWDING`] for what each column is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HerdCrowdingRow {
    /// The highest `herd / fieldsCattle` that lands in this band, inclusive.
    /// The last row is the catch-all and its bound is never consulted.
    pub density_max: i32,
    /// The value stored in county `+0x25C` and shown by `L2.eng` group 77.
    pub level: i32,
    /// Deaths per ten thousand head, before understaffing is added.
    pub death_rate: i32,
    /// Births per ten thousand head at 100% staffing.
    pub birth_rate: i32,
}

/// Everything `FUN_0044DA99` and `FUN_0044D913` read — the rule that a herd
/// needs tending, and the rule that says how crowded it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HerdTable {
    /// [`HERD_LABOUR_PER_HEAD`]. Zero would divide by zero in `PctOf`'s
    /// caller's arithmetic sense — `PctOf` itself returns 0 — so it is treated
    /// as "no staffing requirement" rather than refused.
    pub labour_per_head: i32,
    pub staffing_max: i32,
    /// [`HERD_UNDERSTAFFING_DIVISOR`].
    pub understaffing_divisor: i32,
    pub crowding: [HerdCrowdingRow; HERD_CROWDING_COUNT],
    /// [`HERD_SMALL_BONUS`], `(below, bonus)`, tried in order.
    pub small_bonus: [(i32, i32); 3],
    pub no_pasture_density: i32,
    pub no_pasture_kill_all_below: i32,
    pub no_pasture_divisor: i32,
    /// [`HERD_CALVING_SEASON`] and [`HERD_CULLING_SEASON`], as `g_season`
    /// indices. 0 — the original's *No Season* — disables the bonus.
    pub calving_season: u8,
    pub culling_season: u8,
    /// `(numerator, denominator)`, `3 / 2` in the original.
    pub season_bonus: (i32, i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CastleTable {
    pub starting_type: u8,
    /// By castle type 0..=5.
    pub tax_base: [i32; CASTLE_TYPE_COUNT],
    /// By castle type 1..=5, plus the trailing zero the binary stores.
    ///
    /// Six slots rather than five, throughout. `tools/oracle/kingdom.ps1`
    /// confirmed the layout: `g_castleGarrisonCap` at `0x004D8A10` holds
    /// `150, 200, 200, 400, 600, 0`, and it is that 24-byte stride that puts
    /// the tax bonuses at `0x004D8A28` and the free archers at `0x004D8A40`.
    /// Keeping the sixth slot means the arrays are the shape the game has,
    /// rather than a tidied copy that no longer matches the addresses.
    pub tax_bonus_pct: [i32; 6],
    /// `(wood, stone)` by castle type 1..=5.
    pub cost: [(i32, i32); 5],
    /// Two ints per castle level in the binary, both holding the same number.
    /// What the second column is for is **not established**, so it is carried
    /// rather than discarded — see `docs/kingdom.md`.
    pub workforce: [(i32, i32); 5],
    pub garrison_cap: [i32; 6],
    pub free_archers: [i32; 6],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommodityRow {
    pub job: usize,
    pub divisor: i32,
    pub base_efficiency: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobTable {
    pub count: usize,
    pub iron_mining: usize,
    pub stone_quarrying: usize,
    pub wood_cutting: usize,
    pub blacksmith: usize,
    /// **Not established** - see [`JOB_GRAIN_FARMING`].
    pub grain_farming: usize,
    /// **`[V]`** - `Herd_SeasonTick` (`0x0044D60D`) passes county `+0xD0` as
    /// the herd's labour, and `(0xD0 - 0xC4) / 0x0C` is exactly record 1. See
    /// [`crate::land::herd_labour`].
    pub cattle_farming: usize,
    /// **Not established** - see [`JOB_CASTLE_BUILDING`].
    pub castle_building: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeaponRow {
    pub wood: i32,
    pub iron: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GoodRow {
    pub sell_price: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WageTable {
    pub divisor_human: i32,
    /// By AI difficulty 0..=2.
    pub divisor_ai: [i32; 3],
    pub bankrupt_stage_max: u8,
}

/// The efficiency ramp's two bounds. See [`crate::industry::efficiency_ramp`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EfficiencyTable {
    /// [`EFFICIENCY_MAX`] — the ceiling the compounding ramp clamps to.
    pub max: i32,
    /// [`EFFICIENCY_WITHOUT_ADVANCED_FARMING`] — the flat figure returned when
    /// the option is off, which is the whole ramp in the shipped game.
    pub without_advanced_farming: i32,
}

/// What a barrel of ale is worth. See [`crate::happiness::buy_ale`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AleTable {
    /// [`ALE_HAPPINESS_STEP_PCT`] — one happiness per this percentage of the
    /// county's population, in crowns.
    pub step_pct: i32,
    /// [`ALE_HAPPINESS_MAX`] — the top rung *and* the cumulative cap, which
    /// are one constant in the original and so are one field here.
    pub max: i32,
}

/// One AI lord's personality record, `g_aiPersonality + (lord - 1) * 0xF0`.
///
/// The record is 240 bytes and about a third of it is accounted for. What is
/// carried here is what a rule in this crate reads, plus two fields carried
/// deliberately without a reader and said so. `docs/diplomacy.md` §8.4 lists
/// the eleven fields that hold plausible per-lord values and were never traced
/// (`+0x2C`, `+0x6C`, `+0x70`, `+0x74`, `+0x78`, `+0x7C`, `+0x84`, `+0x88`,
/// `+0x8C`, `+0x9C`, `+0xA0`); they are **not** invented into fields here, and
/// §9 is why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiPersonalityRow {
    /// Record `+0x00`. **Carried, and nothing reads it.** `AI_ManageFields`
    /// copies it into county `+0x1FE` and dispatches into one of three labour
    /// allocators that were never traced, so this crate has no behaviour to
    /// attach to it. It is here because it is half of the table, not because
    /// changing it does anything yet — see [`AI_PERSONALITY_FARM_STYLE`].
    pub farm_style: u8,
    /// Record `+0x04` — which of [`AiTable::tax_ladders`] the lord taxes on.
    pub tax_ladder: usize,
    /// Record `+0x08` — [`AI_PERSONALITY_GIFT_INCREMENT`].
    pub gift_increment: i32,
    /// Record `+0x0C` — [`AI_PERSONALITY_HELP_PRICE`].
    pub help_price: i32,
    /// Record `+0x10` — [`AI_PERSONALITY_GRUDGE_TOLERANCE`].
    pub grudge_tolerance: i32,
    /// Record `+0x14` — [`AI_PERSONALITY_OFFER_INTERVAL`].
    pub offer_interval: i32,
    /// Record `+0x30` — [`AI_PERSONALITY_HELP_POPULATION_FLOOR`]. A population
    /// floor, not a treasury one; the constant's own documentation says why.
    pub help_population_floor: i32,
    /// Record `+0x40` — **carried, and nothing here reads it.**
    /// [`AI_PERSONALITY_MUSTER_PCT`]: raising men is not this crate's, and a
    /// mod that changes this will not change anything until it is.
    pub muster_pct: i32,
    /// Record `+0x90` — [`AI_PERSONALITY_CASTLE_CONCURRENT`].
    pub castle_concurrent: i32,
    /// Record `+0xC8` — [`AI_PERSONALITY_CASTLE_MIN_POPULATION`].
    pub castle_min_population: i32,
    /// Record `+0xCC` … `+0xDC` — [`AI_PERSONALITY_CASTLE_GOLD`], by castle
    /// type 1..=5. A zero means the type is not offered to this lord.
    pub castle_gold: [i32; AI_CASTLE_LADDER_LEN],
}

impl AiPersonalityRow {
    /// The castle type 1..=5 this lord's treasury reaches, or `None`.
    ///
    /// `AI_BuildCastles` (`0x0049EDC7`) walks the ladder **from the top**, so a
    /// realm that can afford a royal castle never builds a palisade. Each rung
    /// is guarded `threshold != 0 && gold >= threshold`, and that guard is the
    /// whole of *"the Baron and the Countess never build a royal castle"*: their
    /// `+0xDC` is 0 and without the guard every one of them would build one for
    /// free.
    pub fn castle_for_gold(&self, gold: i32) -> Option<u8> {
        let mut type_index = self.castle_gold.len();
        while type_index > 0 {
            type_index -= 1;
            let threshold = self.castle_gold[type_index];
            if threshold != 0 && gold >= threshold {
                return Some(type_index as u8 + 1);
            }
        }
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiTable {
    /// `[lord][difficulty]`. Rows 1..=3 are zeroed rather than invented.
    pub gold_grant: [[i32; 4]; 5],
    pub grant_population_per_difficulty: i32,
    pub grant_herd_per_difficulty: i32,
    pub grant_grain_per_difficulty: i32,
    pub grant_min_population: i32,
    pub grant_min_herd: i32,
    pub grant_min_grain: i32,
    /// The ladder unowned counties are taxed on — [`AI_TAX_LADDER_NEUTRAL`].
    pub tax_ladder_neutral: TaxLadder,
    /// The three an AI realm picks between — [`AI_TAX_LADDERS`].
    pub tax_ladders: [TaxLadder; AI_TAX_LADDER_COUNT],
    /// One record per AI lord, indexed `lord - 1`. See
    /// [`AI_PERSONALITY_COUNT`] for why there are four and not five.
    pub personality: [AiPersonalityRow; AI_PERSONALITY_COUNT],
}

/// Everything a unit on the campaign map costs, moves and loses.
///
/// `docs/armies.md` is the source; every field carries the function whose
/// instruction stream the number came from, and `tools/oracle/kingdom.ps1`
/// holds each against `Lords2.exe`.
///
/// The **array sizes** here are not fields, on the same line
/// [`ARMY_HAPPINESS_COST_LEN`] and [`WEAPON_TYPE_COUNT`] draw: a ruleset that
/// gave an army four size classes or eight troop types would be describing a
/// different simulation rather than a different balance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitTable {
    /// [`MOVE_ALLOWANCE_ARMY`] — a season's movement points for a type-1 unit.
    pub move_allowance_army: i32,
    /// [`MOVE_ALLOWANCE_OTHER`] — for the other three types.
    pub move_allowance_other: i32,
    /// [`STEP_COST_ROAD`].
    pub step_cost_road: i32,
    /// [`STEP_COST_OPEN`].
    pub step_cost_open: i32,
    /// [`STEP_COST_FIELD_EXTRA`] — charged *in addition to*
    /// [`UnitTable::step_cost_open`], which is why a field costs six.
    pub step_cost_field_extra: i32,
    /// [`STEP_COST_TRAMPLE`].
    pub step_cost_trample: i32,
    /// [`MOVE_COST_BLOCKED`] — the pathfinder's "go round this".
    pub move_cost_blocked: i32,
    /// [`TRAMPLE_DISABLED_SEASONS`].
    pub trample_disabled_seasons: i32,
    /// [`ARMY_MAX_MEN`] — two armies merge only if their men sum to this or
    /// less.
    pub army_max_men: i32,
    /// [`ARMY_MIN_MEN`].
    pub army_min_men: i32,
    /// [`MOB_DESTROYED_BELOW_MEN`] — a *peasant mob* rule, not an army one.
    pub mob_destroyed_below_men: i32,
    /// [`DESERTION_PCT`].
    pub desertion_pct: i32,
    /// [`DESERTION_MIN_TROOPS`].
    pub desertion_min_troops: i32,
    /// [`STARVATION_LIMIT`].
    pub starvation_limit: i32,
    /// [`LEVY_SURCHARGE`].
    pub levy_surcharge: i32,
    /// [`LEVY_COST_MAX`].
    pub levy_cost_max: i32,
    /// [`LEVY_AUTO_EQUIP_BATCH`].
    pub levy_auto_equip_batch: i32,
    /// [`LEVY_AUTO_EQUIP_ROUNDS`].
    pub levy_auto_equip_rounds: i32,
    /// [`ARMY_SIZE_CLASS_MAX`] — inclusive upper bounds of the first two
    /// classes; anything above the second is the third.
    pub size_class_max: [i32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScoreTable {
    /// `gold >= at_least` gives `points`, richest row first; the last row is 0.
    pub gold_brackets: [(i32, i32); 4],
    /// `(numerator, denominator)` for the six unidentified realm fields.
    pub weights: [(i32, i32); 6],
    pub input_offsets: [u16; 6],
}

impl Tables {
    /// The numbers above, gathered. Assembled *from* the constants rather than
    /// retyped, so there is no second transcription to drift.
    pub const DEFAULT: Tables = Tables {
        food: FoodTable {
            dairy_per_head: DAIRY_PER_HEAD,
            food_per_head: FOOD_PER_HEAD,
            food_per_sack: FOOD_PER_SACK,
        },
        grain: GrainTable {
            yield_per_sack: GRAIN_YIELD_PER_SACK,
            max_sacks_per_field: GRAIN_MAX_SACKS_PER_FIELD,
            labour_divisor_advanced: GRAIN_LABOUR_DIVISOR_ADVANCED,
            labour_divisor_basic: GRAIN_LABOUR_DIVISOR_BASIC,
        },
        field: FieldTable {
            progress_max: FIELD_PROGRESS_MAX,
            reclaim_per_season: FIELD_RECLAIM_PER_SEASON,
        },
        event: EventTable {
            population_cap_pct: EVENT_POPULATION_CAP_PCT,
            first_year: EVENT_FIRST_YEAR,
        },
        season: [
            SeasonRow { death_rate: DEATH_RATE_BY_SEASON[0], dryness: DRYNESS_BY_SEASON[0] },
            SeasonRow { death_rate: DEATH_RATE_BY_SEASON[1], dryness: DRYNESS_BY_SEASON[1] },
            SeasonRow { death_rate: DEATH_RATE_BY_SEASON[2], dryness: DRYNESS_BY_SEASON[2] },
            SeasonRow { death_rate: DEATH_RATE_BY_SEASON[3], dryness: DRYNESS_BY_SEASON[3] },
            SeasonRow { death_rate: DEATH_RATE_BY_SEASON[4], dryness: DRYNESS_BY_SEASON[4] },
        ],
        ration: [
            RationRow {
                divisor: RATION_TABLE[0].0,
                multiplier: RATION_TABLE[0].1,
                health_delta: HEALTH_DELTA[0],
            },
            RationRow {
                divisor: RATION_TABLE[1].0,
                multiplier: RATION_TABLE[1].1,
                health_delta: HEALTH_DELTA[1],
            },
            RationRow {
                divisor: RATION_TABLE[2].0,
                multiplier: RATION_TABLE[2].1,
                health_delta: HEALTH_DELTA[2],
            },
            RationRow {
                divisor: RATION_TABLE[3].0,
                multiplier: RATION_TABLE[3].1,
                health_delta: HEALTH_DELTA[3],
            },
            RationRow {
                divisor: RATION_TABLE[4].0,
                multiplier: RATION_TABLE[4].1,
                health_delta: HEALTH_DELTA[4],
            },
            RationRow {
                divisor: RATION_TABLE[5].0,
                multiplier: RATION_TABLE[5].1,
                health_delta: HEALTH_DELTA[5],
            },
        ],
        tax_happiness_other: TAX_HAPPINESS_OTHER,
        ration_happiness_slope: 3,
        ration_happiness_offset: -8,
        health: [
            HealthBandRow { happiness: HEALTH_HAPPINESS[0], death_rate: DEATH_RATE_BY_HEALTH[0] },
            HealthBandRow { happiness: HEALTH_HAPPINESS[1], death_rate: DEATH_RATE_BY_HEALTH[1] },
            HealthBandRow { happiness: HEALTH_HAPPINESS[2], death_rate: DEATH_RATE_BY_HEALTH[2] },
            HealthBandRow { happiness: HEALTH_HAPPINESS[3], death_rate: DEATH_RATE_BY_HEALTH[3] },
            HealthBandRow { happiness: HEALTH_HAPPINESS[4], death_rate: DEATH_RATE_BY_HEALTH[4] },
        ],
        health_band_ladder: HEALTH_BAND_LADDER,
        population: PopulationTable {
            birth_rate_ladder: BIRTH_RATE_LADDER,
            // The four thresholds `happiness_birth_factor` compares against,
            // plus its `else`. The catch-all threshold is never read; it is
            // written as `i32::MAX` so a reader that does compare it is right.
            happiness_factor_ladder: [(26, 25), (51, 50), (76, 75), (100, 100), (i32::MAX, 120)],
        },
        weather: [
            WeatherRow { herd_pct: HERD_WEATHER_PCT[0] },
            WeatherRow { herd_pct: HERD_WEATHER_PCT[1] },
            WeatherRow { herd_pct: HERD_WEATHER_PCT[2] },
            WeatherRow { herd_pct: HERD_WEATHER_PCT[3] },
            WeatherRow { herd_pct: HERD_WEATHER_PCT[4] },
            WeatherRow { herd_pct: HERD_WEATHER_PCT[5] },
        ],
        herd: HerdTable {
            labour_per_head: HERD_LABOUR_PER_HEAD,
            staffing_max: HERD_STAFFING_MAX,
            understaffing_divisor: HERD_UNDERSTAFFING_DIVISOR,
            crowding: [
                HerdCrowdingRow {
                    density_max: HERD_CROWDING[0].0,
                    level: HERD_CROWDING[0].1,
                    death_rate: HERD_CROWDING[0].2,
                    birth_rate: HERD_CROWDING[0].3,
                },
                HerdCrowdingRow {
                    density_max: HERD_CROWDING[1].0,
                    level: HERD_CROWDING[1].1,
                    death_rate: HERD_CROWDING[1].2,
                    birth_rate: HERD_CROWDING[1].3,
                },
                HerdCrowdingRow {
                    density_max: HERD_CROWDING[2].0,
                    level: HERD_CROWDING[2].1,
                    death_rate: HERD_CROWDING[2].2,
                    birth_rate: HERD_CROWDING[2].3,
                },
                HerdCrowdingRow {
                    density_max: HERD_CROWDING[3].0,
                    level: HERD_CROWDING[3].1,
                    death_rate: HERD_CROWDING[3].2,
                    birth_rate: HERD_CROWDING[3].3,
                },
            ],
            small_bonus: HERD_SMALL_BONUS,
            no_pasture_density: HERD_NO_PASTURE_DENSITY,
            no_pasture_kill_all_below: HERD_NO_PASTURE_KILL_ALL_BELOW,
            no_pasture_divisor: HERD_NO_PASTURE_DIVISOR,
            calving_season: HERD_CALVING_SEASON,
            culling_season: HERD_CULLING_SEASON,
            season_bonus: HERD_SEASON_BONUS,
        },
        castle: CastleTable {
            starting_type: CASTLE_STARTING_TYPE,
            tax_base: CASTLE_TAX_BASE,
            tax_bonus_pct: CASTLE_TAX_BONUS_PCT,
            cost: CASTLE_COST,
            workforce: CASTLE_WORKFORCE,
            garrison_cap: CASTLE_GARRISON_CAP,
            free_archers: CASTLE_FREE_ARCHERS,
        },
        commodity: [
            CommodityRow { job: JOB_WOOD_CUTTING, divisor: 1, base_efficiency: 20 },
            CommodityRow { job: JOB_IRON_MINING, divisor: 1, base_efficiency: 15 },
            CommodityRow { job: JOB_BLACKSMITH, divisor: 4, base_efficiency: 15 },
            CommodityRow { job: JOB_STONE_QUARRYING, divisor: 2, base_efficiency: 15 },
        ],
        job: JobTable {
            count: JOB_COUNT,
            iron_mining: JOB_IRON_MINING,
            stone_quarrying: JOB_STONE_QUARRYING,
            wood_cutting: JOB_WOOD_CUTTING,
            blacksmith: JOB_BLACKSMITH,
            grain_farming: JOB_GRAIN_FARMING,
            cattle_farming: JOB_CATTLE_FARMING,
            castle_building: JOB_CASTLE_BUILDING,
        },
        weapon: [
            WeaponRow { wood: WEAPON_COST[0].0, iron: WEAPON_COST[0].1 },
            WeaponRow { wood: WEAPON_COST[1].0, iron: WEAPON_COST[1].1 },
            WeaponRow { wood: WEAPON_COST[2].0, iron: WEAPON_COST[2].1 },
            WeaponRow { wood: WEAPON_COST[3].0, iron: WEAPON_COST[3].1 },
            WeaponRow { wood: WEAPON_COST[4].0, iron: WEAPON_COST[4].1 },
            WeaponRow { wood: WEAPON_COST[5].0, iron: WEAPON_COST[5].1 },
        ],
        good: [
            GoodRow { sell_price: GOOD_SELL_PRICE[0] },
            GoodRow { sell_price: GOOD_SELL_PRICE[1] },
            GoodRow { sell_price: GOOD_SELL_PRICE[2] },
            GoodRow { sell_price: GOOD_SELL_PRICE[3] },
            GoodRow { sell_price: GOOD_SELL_PRICE[4] },
            GoodRow { sell_price: GOOD_SELL_PRICE[5] },
            GoodRow { sell_price: GOOD_SELL_PRICE[6] },
            GoodRow { sell_price: GOOD_SELL_PRICE[7] },
            GoodRow { sell_price: GOOD_SELL_PRICE[8] },
            GoodRow { sell_price: GOOD_SELL_PRICE[9] },
            GoodRow { sell_price: GOOD_SELL_PRICE[10] },
            GoodRow { sell_price: GOOD_SELL_PRICE[11] },
            GoodRow { sell_price: GOOD_SELL_PRICE[12] },
            GoodRow { sell_price: GOOD_SELL_PRICE[13] },
            GoodRow { sell_price: GOOD_SELL_PRICE[14] },
        ],
        wages: WageTable {
            divisor_human: WAGE_DIVISOR_HUMAN,
            divisor_ai: WAGE_DIVISOR_AI,
            bankrupt_stage_max: BANKRUPT_STAGE_MAX,
        },
        efficiency: EfficiencyTable {
            max: EFFICIENCY_MAX,
            without_advanced_farming: EFFICIENCY_WITHOUT_ADVANCED_FARMING,
        },
        ale: AleTable { step_pct: ALE_HAPPINESS_STEP_PCT, max: ALE_HAPPINESS_MAX },
        army_happiness_cost: ARMY_HAPPINESS_COST,
        unit: UnitTable {
            move_allowance_army: MOVE_ALLOWANCE_ARMY,
            move_allowance_other: MOVE_ALLOWANCE_OTHER,
            step_cost_road: STEP_COST_ROAD,
            step_cost_open: STEP_COST_OPEN,
            step_cost_field_extra: STEP_COST_FIELD_EXTRA,
            step_cost_trample: STEP_COST_TRAMPLE,
            move_cost_blocked: MOVE_COST_BLOCKED,
            trample_disabled_seasons: TRAMPLE_DISABLED_SEASONS,
            army_max_men: ARMY_MAX_MEN,
            army_min_men: ARMY_MIN_MEN,
            mob_destroyed_below_men: MOB_DESTROYED_BELOW_MEN,
            desertion_pct: DESERTION_PCT,
            desertion_min_troops: DESERTION_MIN_TROOPS,
            starvation_limit: STARVATION_LIMIT,
            levy_surcharge: LEVY_SURCHARGE,
            levy_cost_max: LEVY_COST_MAX,
            levy_auto_equip_batch: LEVY_AUTO_EQUIP_BATCH,
            levy_auto_equip_rounds: LEVY_AUTO_EQUIP_ROUNDS,
            size_class_max: ARMY_SIZE_CLASS_MAX,
        },
        ai: AiTable {
            gold_grant: AI_GOLD_GRANT,
            grant_population_per_difficulty: AI_GRANT_POPULATION_PER_DIFFICULTY,
            grant_herd_per_difficulty: AI_GRANT_HERD_PER_DIFFICULTY,
            grant_grain_per_difficulty: AI_GRANT_GRAIN_PER_DIFFICULTY,
            grant_min_population: AI_GRANT_MIN_POPULATION,
            grant_min_herd: AI_GRANT_MIN_HERD,
            grant_min_grain: AI_GRANT_MIN_GRAIN,
            tax_ladder_neutral: AI_TAX_LADDER_NEUTRAL,
            tax_ladders: AI_TAX_LADDERS,
            personality: [
                AiPersonalityRow {
                    farm_style: AI_PERSONALITY_FARM_STYLE[0],
                    tax_ladder: AI_PERSONALITY_TAX_LADDER[0],
                    gift_increment: AI_PERSONALITY_GIFT_INCREMENT[0],
                    help_price: AI_PERSONALITY_HELP_PRICE[0],
                    grudge_tolerance: AI_PERSONALITY_GRUDGE_TOLERANCE[0],
                    offer_interval: AI_PERSONALITY_OFFER_INTERVAL[0],
                    help_population_floor: AI_PERSONALITY_HELP_POPULATION_FLOOR[0],
                    muster_pct: AI_PERSONALITY_MUSTER_PCT[0],
                    castle_concurrent: AI_PERSONALITY_CASTLE_CONCURRENT[0],
                    castle_min_population: AI_PERSONALITY_CASTLE_MIN_POPULATION[0],
                    castle_gold: AI_PERSONALITY_CASTLE_GOLD[0],
                },
                AiPersonalityRow {
                    farm_style: AI_PERSONALITY_FARM_STYLE[1],
                    tax_ladder: AI_PERSONALITY_TAX_LADDER[1],
                    gift_increment: AI_PERSONALITY_GIFT_INCREMENT[1],
                    help_price: AI_PERSONALITY_HELP_PRICE[1],
                    grudge_tolerance: AI_PERSONALITY_GRUDGE_TOLERANCE[1],
                    offer_interval: AI_PERSONALITY_OFFER_INTERVAL[1],
                    help_population_floor: AI_PERSONALITY_HELP_POPULATION_FLOOR[1],
                    muster_pct: AI_PERSONALITY_MUSTER_PCT[1],
                    castle_concurrent: AI_PERSONALITY_CASTLE_CONCURRENT[1],
                    castle_min_population: AI_PERSONALITY_CASTLE_MIN_POPULATION[1],
                    castle_gold: AI_PERSONALITY_CASTLE_GOLD[1],
                },
                AiPersonalityRow {
                    farm_style: AI_PERSONALITY_FARM_STYLE[2],
                    tax_ladder: AI_PERSONALITY_TAX_LADDER[2],
                    gift_increment: AI_PERSONALITY_GIFT_INCREMENT[2],
                    help_price: AI_PERSONALITY_HELP_PRICE[2],
                    grudge_tolerance: AI_PERSONALITY_GRUDGE_TOLERANCE[2],
                    offer_interval: AI_PERSONALITY_OFFER_INTERVAL[2],
                    help_population_floor: AI_PERSONALITY_HELP_POPULATION_FLOOR[2],
                    muster_pct: AI_PERSONALITY_MUSTER_PCT[2],
                    castle_concurrent: AI_PERSONALITY_CASTLE_CONCURRENT[2],
                    castle_min_population: AI_PERSONALITY_CASTLE_MIN_POPULATION[2],
                    castle_gold: AI_PERSONALITY_CASTLE_GOLD[2],
                },
                AiPersonalityRow {
                    farm_style: AI_PERSONALITY_FARM_STYLE[3],
                    tax_ladder: AI_PERSONALITY_TAX_LADDER[3],
                    gift_increment: AI_PERSONALITY_GIFT_INCREMENT[3],
                    help_price: AI_PERSONALITY_HELP_PRICE[3],
                    grudge_tolerance: AI_PERSONALITY_GRUDGE_TOLERANCE[3],
                    offer_interval: AI_PERSONALITY_OFFER_INTERVAL[3],
                    help_population_floor: AI_PERSONALITY_HELP_POPULATION_FLOOR[3],
                    muster_pct: AI_PERSONALITY_MUSTER_PCT[3],
                    castle_concurrent: AI_PERSONALITY_CASTLE_CONCURRENT[3],
                    castle_min_population: AI_PERSONALITY_CASTLE_MIN_POPULATION[3],
                    castle_gold: AI_PERSONALITY_CASTLE_GOLD[3],
                },
            ],
        },
        score: ScoreTable {
            // `score_gold_bracket` is written as `> 10_000`, `>= 5_001`,
            // `>= 2_001`; as inclusive lower bounds that is 10_001 / 5_001 /
            // 2_001, the same function with one fewer comparison rule for a
            // mod author to learn.
            gold_brackets: [(10_001, 200), (5_001, 100), (2_001, 50), (0, 0)],
            weights: SCORE_WEIGHTS,
            input_offsets: SCORE_INPUT_OFFSETS,
        },
    };

    /// [`ration_happiness`], from this table rather than from the constant.
    pub const fn ration_happiness(&self, level: i32) -> i32 {
        self.ration_happiness_slope * level + self.ration_happiness_offset
    }

    /// [`health_band`], from this table.
    ///
    /// Walks the `{bound, band}` pairs and returns the band of the first bound
    /// the meter falls within — the binary's own shape, so the band is read out
    /// of the table rather than inferred from the loop counter. The last pair
    /// is the catch-all.
    pub fn health_band(&self, meter: i32) -> u8 {
        for &(up_to, band) in &self.health_band_ladder {
            if meter <= up_to {
                return band as u8;
            }
        }
        self.health_band_ladder[self.health_band_ladder.len() - 1].1 as u8
    }

    /// [`birth_rate`], from this table.
    pub fn birth_rate(&self, population: i32) -> i32 {
        for &(up_to, percent) in &self.population.birth_rate_ladder {
            if population <= up_to {
                return percent;
            }
        }
        self.population.birth_rate_ladder[self.population.birth_rate_ladder.len() - 1].1
    }

    /// [`happiness_birth_factor`], from this table.
    pub fn happiness_birth_factor(&self, happiness: i32) -> i32 {
        let ladder = &self.population.happiness_factor_ladder;
        for &(below, percent) in &ladder[..ladder.len() - 1] {
            if happiness < below {
                return percent;
            }
        }
        ladder[ladder.len() - 1].1
    }

    /// [`army_happiness_cost`], from this table.
    ///
    /// Clamps rather than reading past the end, for the reason the free
    /// function gives: the original walks off into the merchant price table,
    /// and reproducing that would hard-code that the two are adjacent — which
    /// a ruleset that rebalances either has already made untrue.
    pub fn army_happiness_cost(&self, pct: i32) -> i32 {
        if pct <= 0 {
            return self.army_happiness_cost[0];
        }
        self.army_happiness_cost[(pct as usize).min(self.army_happiness_cost.len() - 1)]
    }

    /// [`ai_tax_ladder`], from this table: the ladder an AI lord taxes on, or
    /// `None` when the lord byte names no personality record.
    pub fn ai_tax_ladder(&self, lord: u8) -> Option<&TaxLadder> {
        let index = (lord as usize).checked_sub(1)?;
        let row = self.ai.personality.get(index)?;
        self.ai.tax_ladders.get(row.tax_ladder)
    }

    /// The personality record a `lord` byte names, or `None`.
    ///
    /// `None` for **0 (the human), 5, and 6 (eliminated)** — see
    /// [`AI_PERSONALITY_COUNT`] for why there is no fifth record. Every
    /// diplomacy rule that needs a number out of the record refuses to act
    /// rather than substituting one, which is the same choice
    /// [`crate::ai::set_tax_rates`] already makes.
    ///
    /// **Indexed by the lord byte and never by the realm id.** The two are
    /// unrelated except through the new-game draw; `docs/diplomacy.md` §0.1 is
    /// the trap, and it is the shape of `docs/decisions.md` C3.
    pub fn ai_personality(&self, lord: u8) -> Option<&AiPersonalityRow> {
        self.ai.personality.get((lord as usize).checked_sub(1)?)
    }

    /// [`score_gold_bracket`], from this table.
    pub fn score_gold_bracket(&self, gold: i32) -> i32 {
        for &(at_least, points) in &self.score.gold_brackets {
            if gold >= at_least {
                return points;
            }
        }
        0
    }
}

impl Default for Tables {
    fn default() -> Self {
        Tables::DEFAULT
    }
}

#[cfg(test)]
mod gathered_tests {
    use super::*;

    /// The gathered value and the free functions must agree over their whole
    /// domain. That is what makes it safe for one caller to hold a `Tables`
    /// while another keeps reading the constants: today they are the same
    /// numbers, and this test is what keeps them so.
    #[test]
    fn the_gathered_tables_agree_with_the_free_functions_everywhere() {
        let t = Tables::DEFAULT;
        for meter in -50..=200 {
            assert_eq!(t.health_band(meter), health_band(meter), "meter {meter}");
        }
        for level in 0..RATION_LEVEL_COUNT as i32 {
            assert_eq!(t.ration_happiness(level), ration_happiness(level));
        }
        for pop in [0, 1, 39, 40, 41, 417, 500, 501, 1999, 2000, 3000, 50_000] {
            assert_eq!(t.birth_rate(pop), birth_rate(pop), "pop {pop}");
        }
        for h in -20..=150 {
            assert_eq!(t.happiness_birth_factor(h), happiness_birth_factor(h), "happiness {h}");
        }
        for gold in [0, 2000, 2001, 5000, 5001, 10_000, 10_001, 1_000_000] {
            assert_eq!(t.score_gold_bracket(gold), score_gold_bracket(gold), "gold {gold}");
        }
        for pct in -10..=200 {
            assert_eq!(t.army_happiness_cost(pct), army_happiness_cost(pct), "pct {pct}");
        }
        for lord in 0..=8u8 {
            assert_eq!(t.ai_tax_ladder(lord), ai_tax_ladder(lord), "lord {lord}");
        }
    }

    /// The two scalars that were the last constants the industry and ale rules
    /// read directly.
    #[test]
    fn the_gathered_tables_carry_the_ramp_and_ale_bounds() {
        let t = Tables::DEFAULT;
        assert_eq!(t.efficiency.max, EFFICIENCY_MAX);
        assert_eq!(t.efficiency.without_advanced_farming, EFFICIENCY_WITHOUT_ADVANCED_FARMING);
        assert_eq!(t.ale.step_pct, ALE_HAPPINESS_STEP_PCT);
        assert_eq!(t.ale.max, ALE_HAPPINESS_MAX);
        assert_eq!(t.ai.tax_ladder_neutral, AI_TAX_LADDER_NEUTRAL);
        assert_eq!(t.ai.tax_ladders, AI_TAX_LADDERS);
        for (i, row) in t.ai.personality.iter().enumerate() {
            assert_eq!(row.farm_style, AI_PERSONALITY_FARM_STYLE[i], "lord {}", i + 1);
            assert_eq!(row.tax_ladder, AI_PERSONALITY_TAX_LADDER[i], "lord {}", i + 1);
        }
    }

    /// The industry columns, restated as rows, must still be the columns.
    #[test]
    fn the_commodity_rows_match_the_commodity_methods() {
        for c in Commodity::ALL {
            let row = Tables::DEFAULT.commodity[c.index()];
            assert_eq!(row.job, c.job(), "{c:?} job");
            assert_eq!(row.divisor, c.divisor(), "{c:?} divisor");
            assert_eq!(row.base_efficiency, c.base_efficiency(), "{c:?} efficiency");
        }
    }
}
