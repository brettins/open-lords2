//! `docs/decisions.md` C11 is the reason this file exists as data.
//!
//! * **Season-indexed arrays are 1-based**, with index 0 unused, because the
//!   original's are: `docs/kingdom.md` §9 pins `g_deathRateBySeason[4]
//!   = 8` for Winter, and `g_season` is used directly as an `L2.eng` group 29
//!   index where 0 is the string `No Season`.

mod constants;
pub use constants::*;
mod ai;
pub use ai::*;
mod methods;
pub use methods::*;

/// `g_season` is 1..=4 and indexes `L2.eng` group 29 directly:
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

/// `L2.eng` group 66, six name/effect pairs, in this order.
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
    pub fn base_efficiency(self) -> i32 {
        match self {
            Commodity::Wood => 20,
            _ => 15,
        }
    }
}

/// `docs/decisions.md` C11: **no kingdom rule is loaded from a game data
/// file.** Every economic constant lives in `Lords2.exe`, so modding
/// the original means patching a binary, and why an open engine is worth
/// building at all. Our engine therefore has to carry the whole ruleset
/// itself - and carrying it as `const` items makes it
/// as the 1996 binary made it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tables {
    pub food: FoodTable,
    pub grain: GrainTable,
    pub field: FieldTable,
    pub event: EventTable,
    pub season: [SeasonRow; 5],
    pub ration: [RationRow; RATION_LEVEL_COUNT],
    /// `ration_happiness(level) = slope * level + offset`. In the original this
/// is the expression `3L - 8` at the end of
    /// `Ration_Apply` - an instruction, not data, which is the whole of C11 in
    /// one line.
    pub ration_happiness_slope: i32,
    pub ration_happiness_offset: i32,
    pub health: [HealthBandRow; HEALTH_BAND_COUNT],
    /// `{inclusive upper bound, band}` pairs, in the binary's own layout at
    /// `0x004D6520`. Five of them, not four: the top band is an explicit
/// `(100, 4)` entry. Verified against the executable
    /// by `tools/oracle/kingdom.ps1`.
    pub health_band_ladder: [(i32, i32); HEALTH_BAND_COUNT],
    pub tax_happiness_other: [i32; MAX_TAX_RATE as usize + 1],
    pub population: PopulationTable,
    pub weather: [WeatherRow; 6],
    pub herd: HerdTable,
    pub castle: CastleTable,
    pub commodity: [CommodityRow; 4],
    pub job: JobTable,
    pub weapon: [WeaponRow; WEAPON_TYPE_COUNT],
    /// Indexed by `L2.eng` group 6 good id 0..=14; index 0 is unused.
    pub good: [GoodRow; 15],
    pub wages: WageTable,
    pub efficiency: EfficiencyTable,
    pub ale: AleTable,
    pub army_happiness_cost: [i32; ARMY_HAPPINESS_COST_LEN],
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
    pub grow_per_worker_advanced: i32,
    pub harvest_per_worker_advanced: i32,
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
    pub health_delta: [i32; HEALTH_BAND_COUNT],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HealthBandRow {
    pub happiness: i32,
    pub death_rate: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PopulationTable {
    pub birth_rate_ladder: [(i32, i32); 20],
    pub happiness_factor_ladder: [(i32, i32); 5],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeatherRow {
    pub herd_pct: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HerdCrowdingRow {
    pub density_max: i32,
    /// The value stored in county `+0x25C` and shown by `L2.eng` group 77.
    pub level: i32,
    pub death_rate: i32,
    pub birth_rate: i32,
}

/// Everything `FUN_0044DA99` and `FUN_0044D913` read — the rule that a herd
/// needs tending, and the rule that says how crowded it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HerdTable {
    pub labour_per_head: i32,
    pub staffing_max: i32,
    pub understaffing_divisor: i32,
    pub crowding: [HerdCrowdingRow; HERD_CROWDING_COUNT],
    pub small_bonus: [(i32, i32); 3],
    pub no_pasture_density: i32,
    pub no_pasture_kill_all_below: i32,
    pub no_pasture_divisor: i32,
    pub calving_season: u8,
    pub culling_season: u8,
    pub season_bonus: (i32, i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CastleTable {
    pub starting_type: u8,
    pub tax_base: [i32; CASTLE_TYPE_COUNT],
/// Six slots, throughout. `tools/oracle/kingdom.ps1`
    /// confirmed the layout: `g_castleGarrisonCap` at `0x004D8A10` holds
    /// `150, 200, 200, 400, 600, 0`, and it is that 24-byte stride that puts
    /// the tax bonuses at `0x004D8A28` and the free archers at `0x004D8A40`.
    ///.
    pub tax_bonus_pct: [i32; 6],
    pub cost: [(i32, i32); 5],
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
    pub grain_farming: usize,
    /// **`[V]`** - `Herd_SeasonTick` (`0x0044D60D`) passes county `+0xD0` as
    /// the herd's labour, and `(0xD0 - 0xC4) / 0x0C` is exactly record 1. See
    /// [`crate::land::herd_labour`].
    pub cattle_farming: usize,
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
    pub divisor_ai: [i32; 3],
    pub bankrupt_stage_max: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EfficiencyTable {
    pub max: i32,
    pub without_advanced_farming: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AleTable {
    pub step_pct: i32,
    pub max: i32,
}

/// The record is 240 bytes and rather over half of it is accounted for. What
/// is carried here is what a rule in this crate reads. `docs/diplomacy.md`
/// §8.4 listed **eleven** fields that hold plausible per-lord values and were
/// never traced (`+0x2C`, `+0x6C`, `+0x70`, `+0x74`, `+0x78`, `+0x7C`,
/// `+0x84`, `+0x88`, `+0x8C`, `+0x9C`, `+0xA0`). Eight of the eleven have a
/// reader now — `+0xA0` is the siege doctrine, `+0x78`/`+0x7C` and the three
/// reserves `+0x84`/`+0x88`/`+0x8C` are `Ai_TradeForCounty`'s, and
/// `+0x70`, `+0x74` and `+0x9C` are AI steps 7 and 10's, traced by
/// [`crate::ai_army`]. **Two remain**: `+0x2C`, a flat 100 in all four
/// records, and `+0x6C`, which runs 2, 3, 4, 5. They are **not** invented into
/// fields here, and §9 is why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiPersonalityRow {
    /// Record `+0x00` — the lord's farming style, copied into county `+0x1FE`
    /// by AI step 5 and dispatched on by [`crate::ai_farm`].
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
    /// Record `+0x40` — [`AI_PERSONALITY_MUSTER_PCT`], the share of a county's
    /// people one muster conscripts. Read by
    /// [`crate::Kingdom::run_ai_raise_army`].
    pub muster_pct: i32,
    /// Record `+0x28` — [`AI_PERSONALITY_MUSTER_PATIENCE`].
    pub muster_patience: i32,
    /// Record `+0x68` — [`AI_PERSONALITY_MUSTER_ARMS`].
    pub muster_arms: i32,
    /// Record `+0x70` — [`AI_PERSONALITY_GARRISON_MIN_POPULATION`].
    pub garrison_min_population: i32,
    /// Record `+0x74` — [`AI_PERSONALITY_RAID_INTERVAL`].
    pub raid_interval: i32,
    /// Record `+0x9C` — [`AI_PERSONALITY_ABANDON_TAX_RATE`].
    pub abandon_tax_rate: i32,
    /// Record `+0x50` … `+0x64` — [`AI_PERSONALITY_WEAPON_ROTA`].
    pub weapon_rota: [usize; 6],
    /// Record `+0x90` — [`AI_PERSONALITY_CASTLE_CONCURRENT`].
    pub castle_concurrent: i32,
    /// Record `+0xC8` — [`AI_PERSONALITY_CASTLE_MIN_POPULATION`].
    pub castle_min_population: i32,
    /// Record `+0xCC` … `+0xDC` — [`AI_PERSONALITY_CASTLE_GOLD`], by castle
    /// type 1..=5. A zero means the type is not offered to this lord.
    pub castle_gold: [i32; AI_CASTLE_LADDER_LEN],
    /// Record `+0xA0` — **the siege-engine doctrine**,
    /// [`AI_PERSONALITY_SIEGE_DOCTRINE`]. `crate::siege::prepare` is its only
    /// reader.
    pub siege_doctrine: i32,
    /// Record `+0x78` — [`AI_PERSONALITY_TRADE_GOLD_FLOOR`].
    pub trade_gold_floor: i32,
    /// Record `+0x7C` — [`AI_PERSONALITY_WEAPON_BUY_QTY`].
    pub weapon_buy_qty: i32,
    /// Record `+0x84` — [`AI_PERSONALITY_RESERVE_WOOD`].
    pub reserve_wood: i32,
    /// Record `+0x88` — [`AI_PERSONALITY_RESERVE_STONE`].
    pub reserve_stone: i32,
    /// Record `+0x8C` — [`AI_PERSONALITY_RESERVE_IRON`].
    pub reserve_iron: i32,
}

impl AiPersonalityRow {
    /// `AI_BuildCastles` (`0x0049EDC7`) walks the ladder **from the top**,
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
    pub gold_grant: [[i32; 4]; 5],
    pub grant_population_per_difficulty: i32,
    pub grant_herd_per_difficulty: i32,
    pub grant_grain_per_difficulty: i32,
    pub grant_min_population: i32,
    pub grant_min_herd: i32,
    pub grant_min_grain: i32,
    pub tax_ladder_neutral: TaxLadder,
    pub tax_ladders: [TaxLadder; AI_TAX_LADDER_COUNT],
    pub personality: [AiPersonalityRow; AI_PERSONALITY_COUNT],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitTable {
    pub move_allowance_army: i32,
    pub move_allowance_other: i32,
    pub step_cost_road: i32,
    pub step_cost_open: i32,
    pub step_cost_field_extra: i32,
    pub step_cost_trample: i32,
    pub move_cost_blocked: i32,
    pub trample_disabled_seasons: i32,
    pub army_max_men: i32,
    pub army_min_men: i32,
    pub mob_destroyed_below_men: i32,
    pub desertion_pct: i32,
    pub desertion_min_troops: i32,
    pub starvation_limit: i32,
    pub levy_surcharge: i32,
    pub levy_cost_max: i32,
    pub levy_auto_equip_batch: i32,
    pub levy_auto_equip_rounds: i32,
    pub size_class_max: [i32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScoreTable {
    pub gold_brackets: [(i32, i32); 4],
    pub weights: [(i32, i32); 6],
    pub input_offsets: [u16; 6],
}

impl Default for Tables {
    fn default() -> Self {
        Tables::DEFAULT
    }
}

#[cfg(test)]
mod gathered_tests {
    use super::*;

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

