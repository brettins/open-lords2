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
/// `docs/kingdom.md` §7.4 says so, and the shipped save has the option **off**,
/// so the FAQ's *"30 serfs working at 15% efficiency"* describes the advanced
/// game only.
pub const EFFICIENCY_WITHOUT_ADVANCED_FARMING: i32 = 80;

/// The efficiency ceiling in `FUN_0044F248`.
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

/// A tax ladder: `(happiness_below, rate)` rows, walked in order, taking the
/// first row the county's happiness is strictly below. The last row is the
/// fallback and its threshold is never tested.
pub type TaxLadder = [(i32, i32); 8];

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
pub const AI_TAX_LADDERS: [TaxLadder; 3] = [
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

/// `g_aiPersonality` (`0x004D8A58`) is five records of this many bytes, one per
/// AI lord 1..=5. `[I]` on the base and the stride: the code addresses it as
/// `base + (lord * 3 - 3) * 0x50`, i.e. three 0x50-byte rows per lord, and only
/// ever uses the first row. The arithmetic closes on the low side -
/// `0x004D8A40 + 24` (the free-archer table) is exactly `0x004D8A58`.
pub const AI_PERSONALITY_STRIDE: usize = 0xF0;

/// The personality field at record `+0x04` that selects an [`AI_TAX_LADDERS`]
/// row, indexed by lord 1..=5 with index 0 unused. `[V]` on the four values
/// read; **lord 5's is not established** - the record would sit at
/// `0x004D8E1C`, which was not dumped, so it is left as ladder 2 (what lords
/// 1..3 use) and marked here rather than silently defaulted.
pub const AI_PERSONALITY_TAX_LADDER: [usize; 6] = [2, 2, 2, 2, 1, 2];

/// The personality field at record `+0x00` that `AI_ManageFields` copies into
/// county `+0x1FE` and dispatches on, indexed by lord 1..=5. `[V]` for lords
/// 1..=4; the values seen are 1, 1, 0 and 9, and 0/1/9 are the three the
/// dispatch tests. **Lord 5's is not established**, for the same reason as
/// above. What each style *does* was not traced.
pub const AI_PERSONALITY_FARM_STYLE: [u8; 6] = [0, 1, 1, 0, 9, 1];

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

/// The bare offsets of the six unidentified score inputs, kept so the
/// unknowns stay visibly unknown rather than being given invented names.
pub const SCORE_INPUT_OFFSETS: [u16; 6] = [0x60, 0x10, 0x0C, 0x58, 0x54, 0x4C];

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
/// the source of truth and keep their addresses and their evidence in their
/// own doc comments. The simulation modules in this crate still read those
/// constants directly. Handing them a `Tables` instead is a mechanical change
/// that has not been made yet - `docs/modding.md` says so in the same words.
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
    pub population: PopulationTable,
    /// Indexed by [`Weather`].
    pub weather: [WeatherRow; 6],
    pub castle: CastleTable,
    /// Indexed by [`Commodity`].
    pub commodity: [CommodityRow; 4],
    pub job: JobTable,
    /// Indexed by weapon type 0..=5.
    pub weapon: [WeaponRow; WEAPON_TYPE_COUNT],
    /// Indexed by `L2.eng` group 6 good id 0..=14; index 0 is unused.
    pub good: [GoodRow; 15],
    pub wages: WageTable,
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
        ai: AiTable {
            gold_grant: AI_GOLD_GRANT,
            grant_population_per_difficulty: AI_GRANT_POPULATION_PER_DIFFICULTY,
            grant_herd_per_difficulty: AI_GRANT_HERD_PER_DIFFICULTY,
            grant_grain_per_difficulty: AI_GRANT_GRAIN_PER_DIFFICULTY,
            grant_min_population: AI_GRANT_MIN_POPULATION,
            grant_min_herd: AI_GRANT_MIN_HERD,
            grant_min_grain: AI_GRANT_MIN_GRAIN,
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
