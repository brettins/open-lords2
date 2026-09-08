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

/// `g_healthBandLadder` (`0x004D6520`): `<=10 -> 0, <=35 -> 1, <=65 -> 2,
/// <=90 -> 3, else 4`.
///
/// The comparison sense is *inclusive*, and that is not a guess:
/// `docs/kingdom.md` §9 needs a starting meter of 65 to band as **2**, which
/// only happens if 65 is `<= 65`.
pub const HEALTH_BAND_LADDER: [i32; 4] = [10, 35, 65, 90];

/// Band a health meter through [`HEALTH_BAND_LADDER`].
#[inline]
pub fn health_band(meter: i32) -> u8 {
    let mut band = 0u8;
    while (band as usize) < HEALTH_BAND_LADDER.len() && meter > HEALTH_BAND_LADDER[band as usize] {
        band += 1;
    }
    band
}

/// `g_healthHappiness` (`0x004D6548`) - the happiness a health band is worth
/// per season, indexed by band 0..=4.
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
/// statement of [`CASTLE_TAX_BASE`].
pub const CASTLE_TAX_BONUS_PCT: [i32; 5] = [50, 75, 100, 125, 150];

/// `0x004D89C0` - `(wood, stone)` to build, by castle type 1..=5.
pub const CASTLE_COST: [(i32, i32); 5] =
    [(400, 40), (800, 80), (200, 1000), (400, 2000), (800, 3000)];

/// `0x004D89E8` - the workforce a build consumes, by castle type 1..=5.
pub const CASTLE_WORKFORCE: [i32; 5] = [200, 400, 800, 1500, 2500];

/// `0x004D8A10` - garrison cap, by castle type 1..=5.
pub const CASTLE_GARRISON_CAP: [i32; 5] = [150, 200, 200, 400, 600];

/// `0x004D8A40` - free archers the castle comes with, by castle type 1..=5.
/// The manual: *"A new castle will automatically include a garrison. Its size
/// will vary according to the size of the castle."*
pub const CASTLE_FREE_ARCHERS: [i32; 5] = [50, 150, 150, 200, 300];

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

    /// Base efficiency percentage. The 15% also appears verbatim in a published
    /// FAQ (*"30 serfs working at 15% efficiency"*).
    ///
    /// The *ramp* - `FUN_0044F248`, which recomputes county `+0x294` - was
    /// never traced (`docs/kingdom.md` §12), so this is the whole model here.
    pub fn base_efficiency(self) -> i32 {
        match self {
            Commodity::Wood => 20,
            _ => 15,
        }
    }
}

/// `L2.eng` group 74 job slots. Only the four the industry table names are
/// established; the other six of the ten `+0xC4 + job*0x0C` slots are not
/// identified in `docs/kingdom.md`.
pub const JOB_COUNT: usize = 10;
pub const JOB_IRON_MINING: usize = 5;
pub const JOB_STONE_QUARRYING: usize = 6;
pub const JOB_WOOD_CUTTING: usize = 7;
pub const JOB_BLACKSMITH: usize = 8;

/// **Not established.** `Grain_Sow` tests a labour figure against
/// `12 * fields * sacks / divisor`, but `docs/kingdom.md` §7.1 never says which
/// of the ten job slots holds it, and §7.4's table only names jobs 5..=8. Slot
/// 0 is this crate's placeholder so the rule can be written and tested at all.
pub const JOB_GRAIN_FARMING: usize = 0;

/// **Not established**, for the same reason as [`JOB_GRAIN_FARMING`]: the
/// castle build pass has to draw its workforce from somewhere.
pub const JOB_CASTLE_BUILDING: usize = 1;

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

/// `g_aiGoldGrant` (`0x004DC1E0`) - `int[5][4]`, indexed `[lord][difficulty]`.
///
/// **Only two of the five rows are documented.** `docs/kingdom.md` §8.2 gives
/// the endpoints - *"from all zeros for lord 0 up to `250, 600, 1100, 1800`"* -
/// and nothing in between. Rows 1..=3 are zeroed here rather than invented, so
/// an AI lord 1..3 currently gets no gold grant. That is a known gap, not a
/// rule.
///
/// Row 0 being all zeros is the load-bearing part: **the human's `lord` byte is
/// 0, so the human gets nothing.**
pub const AI_GOLD_GRANT: [[i32; 4]; 5] = [
    [0, 0, 0, 0],           // lord 0 - the human
    [0, 0, 0, 0],           // lord 1 - not documented
    [0, 0, 0, 0],           // lord 2 - not documented
    [0, 0, 0, 0],           // lord 3 - not documented
    [250, 600, 1100, 1800], // lord 4 - the top row docs/kingdom.md quotes
];

/// The AI's free population, herd and grain per county per season:
/// `difficulty * 20` people, `difficulty * 5` head, `difficulty * 40` sacks.
/// `docs/kingdom.md` §8.2.
pub const AI_GRANT_POPULATION_PER_DIFFICULTY: i32 = 20;
pub const AI_GRANT_HERD_PER_DIFFICULTY: i32 = 5;
pub const AI_GRANT_GRAIN_PER_DIFFICULTY: i32 = 40;

/// The grant is gated on the county *already having some*, so it compounds
/// rather than rescues.
pub const AI_GRANT_MIN_POPULATION: i32 = 20;
pub const AI_GRANT_MIN_HERD: i32 = 10;
pub const AI_GRANT_MIN_GRAIN: i32 = 50;

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
