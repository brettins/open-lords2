//! `kingdom.*` — the economic constants, as rules.
//!
//! `docs/decisions.md` C11 in one sentence: *every* economic constant — tax,
//! happiness, rations, births, deaths, yields, wages — lives in `Lords2.exe`,
//! clustered around `0x004D6300` and `0x004D8900`, and nowhere else. There is
//! no data file to edit. Modding the 1996 game's economy means patching a
//! binary, which is the strongest single argument for building an open engine
//! at all.
//!
//! This module is the other end of that argument. It reads the same numbers
//! out of a rule document into an [`l2_kingdom::tables::Tables`], which is
//! plain data with no loader, no I/O and no knowledge that mods exist.
//!
//! # What is and is not wired up
//!
//! Loading works, is validated, round-trips against
//! [`Tables::DEFAULT`](l2_kingdom::tables::Tables::DEFAULT) in a test — **and
//! is consumed.** `l2_kingdom::Kingdom::with_tables` runs the season pipeline
//! on the table it is handed, and `tests/simulation.rs` asserts that a `.toml`
//! in a mod directory changes how many sacks a county brings in over a year.
//! That is the same standard the battle side is held to by
//! `l2_sim::Battle::with_troops`, and for a long time this half did not meet
//! it: the rules were loaded, checked, reported and then ignored.
//!
//! The ale ladder, the army-raising cost table, the efficiency ramp's bounds
//! and the AI's four tax ladders and personality table were the last rules with
//! no field in [`Tables`], and they have one now. `tests/simulation.rs` proves
//! each of them by running the rule and reading a different answer out of the
//! simulation, not by reading the field back.
//!
//! What is *not* covered is worth naming rather than leaving to be discovered:
//!
//! * **`kingdom.ai.personality.*.farm_style` loads and does nothing.**
//!   `AI_ManageFields` dispatches on it into three labour allocators that were
//!   never traced, so `l2-kingdom` has no behaviour to attach to it. It is in
//!   the schema because it is half of the personality record, and it is
//!   labelled in the rendered document so an author is not left guessing.
//! * **Array sizes are structure, not balance**: nine job slots, six ration
//!   levels, six weapon types, 102 army-cost rows, eight tax-ladder rungs, four
//!   personality records. A ruleset that changed one would be describing a
//!   different simulation.
//!
//! `docs/modding.md` §11 has the full division.

use crate::ruleset::{RuleError, Ruleset};
use l2_kingdom::tables::{
    AiPersonalityRow, AiTable, AleTable, CastleTable, CommodityRow, EfficiencyTable, EventTable,
    FieldTable, FoodTable, GoodRow, GrainTable, HealthBandRow, HerdCrowdingRow, HerdTable,
    JobTable, PopulationTable, RationRow, ScoreTable, SeasonRow, Tables, TaxLadder, WageTable,
    WeaponRow, WeatherRow, AI_PERSONALITY_COUNT, AI_TAX_LADDER_COUNT, ARMY_HAPPINESS_COST_LEN,
    HERD_CROWDING_COUNT, TAX_LADDER_RUNGS,
};

/// Season ids, by `g_season` index. Index 0 is the original's `No Season`,
/// which is never a real season but is a real array slot.
pub const SEASON_IDS: [&str; 5] = ["none", "spring", "summer", "autumn", "winter"];

/// Ration level ids, `L2.eng` group 21.
pub const RATION_IDS: [&str; 6] = ["none", "quarter", "half", "normal", "double", "triple"];

/// Health band ids, `L2.eng` group 20.
pub const HEALTH_BAND_IDS: [&str; 5] = ["diseased", "sick", "average", "good", "perfect"];

/// Weather ids, `L2.eng` group 66, in index order.
pub const WEATHER_IDS: [&str; 6] =
    ["frost", "drought", "sunny", "cloudy", "storms", "flooding"];

/// Castle type ids, `L2.eng` group 71.
pub const CASTLE_IDS: [&str; 6] = [
    "none",
    "wooden_palisade",
    "motte_and_bailey",
    "norman_keep",
    "stone_castle",
    "royal_castle",
];

/// The four passes `Industry_Produce` makes, in the order it makes them.
pub const COMMODITY_IDS: [&str; 4] = ["wood", "iron", "weapons", "stone"];

/// Weapon ids, in the order `g_weaponCost` stores them.
pub const WEAPON_IDS: [&str; 6] = ["crossbow", "mace", "sword", "pike", "bow", "armour"];

/// Tradeable good ids, `L2.eng` group 6. Index 0 is not a good.
pub const GOOD_IDS: [&str; 15] = [
    "none", "grain", "cattle", "sheep", "ale", "wool", "iron", "stone", "timber", "pikes", "bows",
    "maces", "crossbows", "swords", "mail",
];

const BIRTH_ROWS: usize = 20;
const HAPPINESS_ROWS: usize = 5;
const GOLD_BRACKETS: usize = 4;
const SCORE_WEIGHTS: usize = 6;
const AI_LORDS: usize = 5;
const AI_DIFFICULTIES: usize = 4;

/// Build a [`Tables`] out of a merged ruleset.
///
/// Every value is required. There are no defaults here on purpose: a kingdom
/// rule that silently falls back to a built-in number is a rule a mod cannot
/// tell it failed to set, and the whole point of the platform is that a mod
/// author can see what took effect.
pub fn tables(rs: &Ruleset) -> Result<Tables, RuleError> {
    let mut t = Tables::DEFAULT;

    t.food = FoodTable {
        dairy_per_head: int(rs, "kingdom.food.dairy_per_head", 1, 10_000)?,
        food_per_head: int(rs, "kingdom.food.food_per_head", 1, 10_000)?,
        food_per_sack: int(rs, "kingdom.food.food_per_sack", 1, 10_000)?,
    };
    t.grain = GrainTable {
        yield_per_sack: int(rs, "kingdom.grain.yield_per_sack", 0, 10_000)?,
        max_sacks_per_field: int(rs, "kingdom.grain.max_sacks_per_field", 0, 10_000)?,
        // Divisors. Zero is a division by zero in `Grain_Sow`, not a rebalance.
        labour_divisor_advanced: int(rs, "kingdom.grain.labour_divisor_advanced", 1, 10_000)?,
        labour_divisor_basic: int(rs, "kingdom.grain.labour_divisor_basic", 1, 10_000)?,
    };
    t.field = FieldTable {
        progress_max: int(rs, "kingdom.field.progress_max", 1, 1_000_000)?,
        reclaim_per_season: int(rs, "kingdom.field.reclaim_per_season", 1, 1_000_000)?,
    };
    t.event = EventTable {
        population_cap_pct: int(rs, "kingdom.event.population_cap_pct", 0, 100)?,
        first_year: int(rs, "kingdom.event.first_year", 0, 10_000)?,
    };

    for (index, id) in SEASON_IDS.iter().enumerate() {
        let base = format!("kingdom.season.{id}");
        check_index(rs, &base, index)?;
        t.season[index] = SeasonRow {
            death_rate: int(rs, &format!("{base}.death_rate"), -100, 100)?,
            dryness: int(rs, &format!("{base}.dryness"), -1000, 1000)?,
        };
    }

    t.ration_happiness_slope = int(rs, "kingdom.happiness.ration_slope", -100, 100)?;
    t.ration_happiness_offset = int(rs, "kingdom.happiness.ration_offset", -100, 100)?;

    t.ale = AleTable {
        // `buy_ale` divides the population by this, so zero is a division by
        // zero rather than "ale is free".
        step_pct: int(rs, "kingdom.happiness.ale_step_pct", 1, 10_000)?,
        // The rung count and the cumulative cap are one number, as they are in
        // the original. Zero means ale buys nothing, which is a rebalance and
        // not a crash, so it is allowed.
        max: int(rs, "kingdom.happiness.ale_max", 0, 100)?,
    };

    let army = rs.integer_array("kingdom.happiness.army_cost", ARMY_HAPPINESS_COST_LEN)?;
    for (pct, &value) in army.iter().enumerate() {
        t.army_happiness_cost[pct] =
            narrow(rs, "kingdom.happiness.army_cost", value, 0, 10_000)?;
    }

    for (index, id) in RATION_IDS.iter().enumerate() {
        let base = format!("kingdom.ration.{id}");
        check_index(rs, &base, index)?;
        let delta = rs.integer_array(&format!("{base}.health_delta"), HEALTH_BAND_IDS.len())?;
        let mut health_delta = [0i32; 5];
        for (band, &value) in delta.iter().enumerate() {
            health_delta[band] = narrow(rs, &format!("{base}.health_delta"), value, -1000, 1000)?;
        }
        t.ration[index] = RationRow {
            // A divisor of zero divides by zero in `Ration_Apply`.
            divisor: int(rs, &format!("{base}.divisor"), 1, 10_000)?,
            multiplier: int(rs, &format!("{base}.multiplier"), 0, 10_000)?,
            health_delta,
        };
    }

    // Five bounds, not four. The table is `{bound, band}` pairs in the binary's
    // own layout, and the top band is an explicit entry rather than an `else` —
    // `tools/oracle/kingdom.ps1` checks that against the executable.
    //
    // The ruleset carries only the bounds, because in the shipped table the band
    // column always equals its own row index (10,0  35,1  65,2  90,3  100,4).
    // That is a property of the data, not a rule, so a mod wanting non-identity
    // bands would need the format to grow a column rather than get them by
    // accident.
    let bands = t.health_band_ladder.len();
    let ladder = rs.integer_array("kingdom.health.band_ladder", bands)?;
    for i in 0..bands {
        let bound = narrow(rs, "kingdom.health.band_ladder", ladder[i], -1000, 1000)?;
        t.health_band_ladder[i] = (bound, i as i32);
        if i > 0 && bound <= t.health_band_ladder[i - 1].0 {
            return Err(range(
                rs,
                "kingdom.health.band_ladder",
                format!(
                    "step {i} ({bound}) must be above step {} ({}); the ladder is ascending",
                    i - 1,
                    t.health_band_ladder[i - 1].0
                ),
            ));
        }
    }
    for (index, id) in HEALTH_BAND_IDS.iter().enumerate() {
        let base = format!("kingdom.health.band.{id}");
        check_index(rs, &base, index)?;
        t.health[index] = HealthBandRow {
            happiness: int(rs, &format!("{base}.happiness"), -100, 100)?,
            death_rate: int(rs, &format!("{base}.death_rate"), 0, 100)?,
        };
    }

    let mut birth_rate_ladder = [(0i32, 0i32); BIRTH_ROWS];
    expect_rows(rs, "kingdom.population.birth_rate", BIRTH_ROWS)?;
    for (i, slot) in birth_rate_ladder.iter_mut().enumerate() {
        let base = format!("kingdom.population.birth_rate.{i}");
        *slot = (
            int(rs, &format!("{base}.up_to"), 0, i32::MAX as i64)?,
            int(rs, &format!("{base}.percent"), 0, 10_000)?,
        );
    }
    let mut happiness_factor_ladder = [(0i32, 0i32); HAPPINESS_ROWS];
    expect_rows(rs, "kingdom.population.happiness_factor", HAPPINESS_ROWS)?;
    for (i, slot) in happiness_factor_ladder.iter_mut().enumerate() {
        let base = format!("kingdom.population.happiness_factor.{i}");
        *slot = (
            int(rs, &format!("{base}.below"), i32::MIN as i64, i32::MAX as i64)?,
            int(rs, &format!("{base}.percent"), 0, 10_000)?,
        );
    }
    t.population = PopulationTable { birth_rate_ladder, happiness_factor_ladder };

    for (index, id) in WEATHER_IDS.iter().enumerate() {
        let base = format!("kingdom.weather.{id}");
        check_index(rs, &base, index)?;
        t.weather[index] =
            WeatherRow { herd_pct: int(rs, &format!("{base}.herd_pct"), -100, 100)? };
    }

    // The empire tax term, one row per rate. The length is the tax ceiling and
    // is therefore structure rather than balance: a document with a different
    // number of rows is describing a game whose tax panel stops somewhere else.
    let tax = rs.integer_array("kingdom.tax.happiness_other", t.tax_happiness_other.len())?;
    for (rate, &value) in tax.iter().enumerate() {
        t.tax_happiness_other[rate] =
            narrow(rs, "kingdom.tax.happiness_other", value, -100, 100)?;
    }

    let mut crowding = t.herd.crowding;
    expect_rows(rs, "kingdom.herd.crowding", HERD_CROWDING_COUNT)?;
    for (i, row) in crowding.iter_mut().enumerate() {
        let base = format!("kingdom.herd.crowding.{i}");
        *row = HerdCrowdingRow {
            density_max: int(rs, &format!("{base}.density_max"), 0, i32::MAX as i64)?,
            level: int(rs, &format!("{base}.level"), 0, 1_000_000)?,
            death_rate: int(rs, &format!("{base}.death_rate"), 0, 10_000)?,
            birth_rate: int(rs, &format!("{base}.birth_rate"), 0, 1_000_000)?,
        };
    }
    let mut small_bonus = t.herd.small_bonus;
    expect_rows(rs, "kingdom.herd.small_bonus", small_bonus.len())?;
    for (i, row) in small_bonus.iter_mut().enumerate() {
        let base = format!("kingdom.herd.small_bonus.{i}");
        *row = (
            int(rs, &format!("{base}.below"), 0, 1_000_000)?,
            int(rs, &format!("{base}.bonus"), 0, 1_000_000)?,
        );
    }
    t.herd = HerdTable {
        // Zero is not a division by zero - `PctOf` guards it - but it does mean
        // "a herd needs no tending at all", which is a rebalance and allowed.
        labour_per_head: int(rs, "kingdom.herd.labour_per_head", 0, 10_000)?,
        staffing_max: int(rs, "kingdom.herd.staffing_max", 0, 10_000)?,
        // The shortfall is divided by this, so zero really would divide by zero.
        understaffing_divisor: int(rs, "kingdom.herd.understaffing_divisor", 1, 10_000)?,
        crowding,
        small_bonus,
        no_pasture_density: int(rs, "kingdom.herd.no_pasture_density", 0, i32::MAX as i64)?,
        no_pasture_kill_all_below: int(
            rs,
            "kingdom.herd.no_pasture_kill_all_below",
            0,
            1_000_000,
        )?,
        no_pasture_divisor: int(rs, "kingdom.herd.no_pasture_divisor", 1, 1_000_000)?,
        // 0 is the original's `No Season` and disables the bonus, which is how
        // a ruleset says "calves arrive evenly all year".
        calving_season: int(rs, "kingdom.herd.calving_season", 0, 4)? as u8,
        culling_season: int(rs, "kingdom.herd.culling_season", 0, 4)? as u8,
        season_bonus: (
            int(rs, "kingdom.herd.season_bonus_numerator", 0, 10_000)?,
            int(rs, "kingdom.herd.season_bonus_denominator", 1, 10_000)?,
        ),
    };

    let mut castle = CastleTable {
        starting_type: int(rs, "kingdom.castle.starting_type", 0, CASTLE_IDS.len() as i64 - 1)?
            as u8,
        tax_base: [0; 6],
        // Six slots, five used: the binary stores a trailing zero after each of
        // these tables, and that 24-byte stride is what places the next one.
        // The shape is kept rather than tidied so the arrays still match the
        // addresses `tools/oracle/kingdom.ps1` reads.
        tax_bonus_pct: [0; 6],
        cost: [(0, 0); 5],
        workforce: [(0, 0); 5],
        garrison_cap: [0; 6],
        free_archers: [0; 6],
    };
    for (index, id) in CASTLE_IDS.iter().enumerate() {
        let base = format!("kingdom.castle.type.{id}");
        check_index(rs, &base, index)?;
        castle.tax_base[index] = int(rs, &format!("{base}.tax_base"), 0, 1_000_000)?;
        if index == 0 {
            // Type 0 is "no castle": it has a tax base and nothing else to
            // build, garrison or pay for.
            continue;
        }
        let b = index - 1;
        castle.tax_bonus_pct[b] = int(rs, &format!("{base}.tax_bonus_pct"), 0, 10_000)?;
        castle.cost[b] = (
            int(rs, &format!("{base}.cost_wood"), 0, 1_000_000)?,
            int(rs, &format!("{base}.cost_stone"), 0, 1_000_000)?,
        );
        // Both columns take the mod's single value, matching the shipped table
        // where the two are always equal. The second column's meaning has never
        // been traced, so it is carried rather than invented a purpose for.
        let workforce = int(rs, &format!("{base}.workforce"), 0, 1_000_000)?;
        castle.workforce[b] = (workforce, workforce);
        castle.garrison_cap[b] = int(rs, &format!("{base}.garrison_cap"), 0, 1_000_000)?;
        castle.free_archers[b] = int(rs, &format!("{base}.free_archers"), 0, 1_000_000)?;
    }
    t.castle = castle;

    let job_count = int(rs, "kingdom.job.count", 1, 64)? as usize;
    let slot = |name: &str| -> Result<usize, RuleError> {
        Ok(int(rs, &format!("kingdom.job.{name}"), 0, job_count as i64 - 1)? as usize)
    };
    t.job = JobTable {
        count: job_count,
        iron_mining: slot("iron_mining")?,
        stone_quarrying: slot("stone_quarrying")?,
        wood_cutting: slot("wood_cutting")?,
        blacksmith: slot("blacksmith")?,
        grain_farming: slot("grain_farming")?,
        cattle_farming: slot("cattle_farming")?,
        castle_building: slot("castle_building")?,
    };

    t.efficiency = EfficiencyTable {
        max: int(rs, "kingdom.efficiency.max", 0, 10_000)?,
        without_advanced_farming: int(
            rs,
            "kingdom.efficiency.without_advanced_farming",
            0,
            10_000,
        )?,
    };

    for (index, id) in COMMODITY_IDS.iter().enumerate() {
        let base = format!("kingdom.commodity.{id}");
        check_index(rs, &base, index)?;
        t.commodity[index] = CommodityRow {
            job: int(rs, &format!("{base}.job"), 0, t.job.count as i64 - 1)? as usize,
            // A divisor of zero divides by zero in `Industry_Produce`.
            divisor: int(rs, &format!("{base}.divisor"), 1, 10_000)?,
            base_efficiency: int(rs, &format!("{base}.base_efficiency"), 0, 10_000)?,
        };
    }

    for (index, id) in WEAPON_IDS.iter().enumerate() {
        let base = format!("kingdom.weapon.{id}");
        check_index(rs, &base, index)?;
        t.weapon[index] = WeaponRow {
            wood: int(rs, &format!("{base}.wood"), 0, 1_000_000)?,
            iron: int(rs, &format!("{base}.iron"), 0, 1_000_000)?,
        };
    }

    for (index, id) in GOOD_IDS.iter().enumerate() {
        let base = format!("kingdom.good.{id}");
        check_index(rs, &base, index)?;
        t.good[index] =
            GoodRow { sell_price: int(rs, &format!("{base}.sell_price"), 0, 1_000_000)? };
    }

    let ai_divisors = rs.integer_array("kingdom.wages.divisor_ai", 3)?;
    let mut divisor_ai = [0i32; 3];
    for (i, &v) in ai_divisors.iter().enumerate() {
        // Wages are `men / divisor`.
        divisor_ai[i] = narrow(rs, "kingdom.wages.divisor_ai", v, 1, 10_000)?;
    }
    t.wages = WageTable {
        divisor_human: int(rs, "kingdom.wages.divisor_human", 1, 10_000)?,
        divisor_ai,
        bankrupt_stage_max: int(rs, "kingdom.wages.bankrupt_stage_max", 0, 255)? as u8,
    };

    let mut gold_grant = [[0i32; AI_DIFFICULTIES]; AI_LORDS];
    expect_rows(rs, "kingdom.ai.gold_grant", AI_LORDS)?;
    for (lord, row) in gold_grant.iter_mut().enumerate() {
        let base = format!("kingdom.ai.gold_grant.{lord}");
        let declared = int(rs, &format!("{base}.lord"), 0, AI_LORDS as i64 - 1)?;
        if declared as usize != lord {
            return Err(range(
                rs,
                &format!("{base}.lord"),
                format!("row {lord} declares lord {declared}; the rows are in lord order"),
            ));
        }
        let by = rs.integer_array(&format!("{base}.by_difficulty"), AI_DIFFICULTIES)?;
        for (d, &v) in by.iter().enumerate() {
            row[d] = narrow(rs, &format!("{base}.by_difficulty"), v, 0, 1_000_000)?;
        }
    }
    let tax_ladder_neutral = tax_ladder(rs, "kingdom.ai.tax_ladder_neutral")?;
    let mut tax_ladders = [tax_ladder_neutral; AI_TAX_LADDER_COUNT];
    for (i, slot) in tax_ladders.iter_mut().enumerate() {
        *slot = tax_ladder(rs, &format!("kingdom.ai.tax_ladder.{i}"))?;
    }

    let mut personality = [AiPersonalityRow { farm_style: 0, tax_ladder: 0 }; AI_PERSONALITY_COUNT];
    expect_rows(rs, "kingdom.ai.personality", AI_PERSONALITY_COUNT)?;
    for (i, slot) in personality.iter_mut().enumerate() {
        let base = format!("kingdom.ai.personality.{i}");
        let declared = int(rs, &format!("{base}.lord"), 1, AI_PERSONALITY_COUNT as i64)?;
        if declared as usize != i + 1 {
            return Err(range(
                rs,
                &format!("{base}.lord"),
                format!("row {i} is lord {}; the rows are in lord order", i + 1),
            ));
        }
        *slot = AiPersonalityRow {
            farm_style: int(rs, &format!("{base}.farm_style"), 0, 255)? as u8,
            tax_ladder: int(rs, &format!("{base}.tax_ladder"), 0, AI_TAX_LADDER_COUNT as i64 - 1)?
                as usize,
        };
    }

    t.ai = AiTable {
        gold_grant,
        grant_population_per_difficulty: int(
            rs,
            "kingdom.ai.grant_population_per_difficulty",
            0,
            1_000_000,
        )?,
        grant_herd_per_difficulty: int(rs, "kingdom.ai.grant_herd_per_difficulty", 0, 1_000_000)?,
        grant_grain_per_difficulty: int(rs, "kingdom.ai.grant_grain_per_difficulty", 0, 1_000_000)?,
        grant_min_population: int(rs, "kingdom.ai.grant_min_population", 0, 1_000_000)?,
        grant_min_herd: int(rs, "kingdom.ai.grant_min_herd", 0, 1_000_000)?,
        grant_min_grain: int(rs, "kingdom.ai.grant_min_grain", 0, 1_000_000)?,
        tax_ladder_neutral,
        tax_ladders,
        personality,
    };

    let mut gold_brackets = [(0i32, 0i32); GOLD_BRACKETS];
    expect_rows(rs, "kingdom.score.gold_bracket", GOLD_BRACKETS)?;
    for (i, slot) in gold_brackets.iter_mut().enumerate() {
        let base = format!("kingdom.score.gold_bracket.{i}");
        *slot = (
            int(rs, &format!("{base}.at_least"), 0, i32::MAX as i64)?,
            int(rs, &format!("{base}.points"), 0, 1_000_000)?,
        );
    }
    let mut weights = [(0i32, 0i32); SCORE_WEIGHTS];
    let mut input_offsets = [0u16; SCORE_WEIGHTS];
    expect_rows(rs, "kingdom.score.weight", SCORE_WEIGHTS)?;
    for i in 0..SCORE_WEIGHTS {
        let base = format!("kingdom.score.weight.{i}");
        input_offsets[i] = int(rs, &format!("{base}.offset"), 0, u16::MAX as i64)? as u16;
        weights[i] = (
            int(rs, &format!("{base}.numerator"), -1_000_000, 1_000_000)?,
            // A denominator of zero divides by zero in `Score_RankRealms`.
            int(rs, &format!("{base}.denominator"), 1, 1_000_000)?,
        );
    }
    t.score = ScoreTable { gold_brackets, weights, input_offsets };

    Ok(t)
}

fn int(rs: &Ruleset, path: &str, lo: i64, hi: i64) -> Result<i32, RuleError> {
    Ok(rs.integer_in(path, lo, hi)? as i32)
}

/// One `AI_SetTaxRates` ladder: `{below, rate}` rows, walked in order, taking
/// the first row the county's happiness is strictly below.
///
/// The simulation's ladder is a fixed [`TAX_LADDER_RUNGS`]-row array because
/// the neutral ladder needs all eight, but the three personality ladders use
/// five, five and six. Rather than make an author write three padding rows of
/// `below = 2147483647`, a document may give **1 to 8** rows and the last one
/// is repeated to fill. That is lossless in both directions: a fall-through
/// returns the last row's rate whether the padding is there or not, and
/// [`render_toml`] trims exactly the rows this adds back.
fn tax_ladder(rs: &Ruleset, path: &str) -> Result<TaxLadder, RuleError> {
    let rows = rs.array_len(path)?;
    if rows == 0 || rows > TAX_LADDER_RUNGS {
        return Err(range(
            rs,
            path,
            format!("a tax ladder has 1 to {TAX_LADDER_RUNGS} rungs, found {rows}"),
        ));
    }
    let mut ladder = [(0i32, 0i32); TAX_LADDER_RUNGS];
    for i in 0..rows {
        let base = format!("{path}.{i}");
        ladder[i] = (
            int(rs, &format!("{base}.below"), i32::MIN as i64, i32::MAX as i64)?,
            int(rs, &format!("{base}.rate"), 0, 100)?,
        );
    }
    for i in rows..TAX_LADDER_RUNGS {
        ladder[i] = ladder[rows - 1];
    }
    Ok(ladder)
}

/// The rows of a ladder that a document has to state: everything up to the
/// last one that differs from its predecessor. The inverse of the padding
/// [`tax_ladder`] applies.
fn tax_ladder_rows(ladder: &TaxLadder) -> &[(i32, i32)] {
    let mut len = ladder.len();
    while len > 1 && ladder[len - 1] == ladder[len - 2] {
        len -= 1;
    }
    &ladder[..len]
}

/// Range-check one element of an array against the array's own origin, since
/// an element has no path of its own that a mod author would recognise.
fn narrow(rs: &Ruleset, path: &str, value: i64, lo: i64, hi: i64) -> Result<i32, RuleError> {
    if value < lo || value > hi {
        return Err(range(rs, path, format!("{value} is outside {lo}..={hi}")));
    }
    Ok(value as i32)
}

fn range(rs: &Ruleset, path: &str, message: String) -> RuleError {
    RuleError::Range {
        path: path.to_string(),
        message,
        origin: rs
            .origin(path)
            .cloned()
            .unwrap_or_else(|| crate::value::Origin::synthetic("kingdom rules")),
    }
}

/// Every named row carries the array index it stands for, and it is checked.
///
/// The ids are readable; the indices are what the simulation uses. Letting a
/// mod rename `winter` would be fine, letting it move winter to slot 2 would
/// not, and this is the line between the two.
fn check_index(rs: &Ruleset, base: &str, expected: usize) -> Result<(), RuleError> {
    let path = format!("{base}.index");
    let found = rs.integer_in(&path, 0, 255)?;
    if found as usize != expected {
        return Err(range(
            rs,
            &path,
            format!("'{base}' is slot {expected}, so its index must be {expected}, not {found}"),
        ));
    }
    Ok(())
}

fn expect_rows(rs: &Ruleset, path: &str, want: usize) -> Result<(), RuleError> {
    let found = rs.array_len(path)?;
    if found != want {
        return Err(range(rs, path, format!("expected {want} rows, found {found}")));
    }
    Ok(())
}

/// Render a [`Tables`] as the document [`tables`] reads back.
///
/// This is how `rulesets/core/rules/kingdom.toml` is produced, and a test
/// asserts the shipped file is exactly this text.
pub fn render_toml(t: &Tables) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    out.push_str(
        "# The kingdom economy: tax, happiness, rations, health, births,\n\
         # deaths, yields, industry, castles, wages, trade and score.\n\
         #\n\
         # GENERATED, NOT AUTHORED. Rendered from l2_kingdom::tables::Tables\n\
         # ::DEFAULT, with a test asserting the two agree. Editing it here\n\
         # will fail the build; put your change in a mod instead.\n\
         #\n\
         # WHY THIS FILE IS THE POINT OF THE PROJECT\n\
         #\n\
         # docs/decisions.md C11: every one of these numbers lives inside\n\
         # Lords2.exe, clustered around 0x004D6300 and 0x004D8900, and in no\n\
         # data file anywhere. TROOPS*.ENG set an expectation that rules would\n\
         # be reachable as data; they are not. Modding the original's economy\n\
         # means patching a 1996 binary. This file is the first time these\n\
         # numbers have been text.\n\
         #\n\
         # Two conventions run through it:\n\
         #\n\
         #   * Named rows carry the array index they stand for, and it is\n\
         #     checked. The names are for you; the indices are what the\n\
         #     simulation uses, and they cannot be reordered.\n\
         #   * Ladders are arrays of tables, tried in order, and the last row\n\
         #     is the catch-all. Arrays replace whole on merge, so a mod that\n\
         #     changes one rung restates the ladder.\n\
         #\n\
         # HONEST SCOPE: every rule in this file is read by the simulation. A\n\
         # mod that changes one changes what a county harvests, mines, pays or\n\
         # feels, and crates/l2-mods/tests/simulation.rs proves that rule by\n\
         # rule by running the season pipeline twice. The one exception is\n\
         # named where it appears: kingdom.ai.personality.*.farm_style loads\n\
         # and nothing reads it. See docs/modding.md sec 11.\n",
    );

    let _ = write!(
        out,
        "\n# --- food ------------------------------------------------------------\n\
         # A standing herd feeds five per head per season without being\n\
         # slaughtered; a slaughtered animal feeds ten; a sack feeds six.\n\
         \n[kingdom.food]\n\
         dairy_per_head = {}\n\
         food_per_head = {}\n\
         food_per_sack = {}\n",
        t.food.dairy_per_head, t.food.food_per_head, t.food.food_per_sack
    );

    let _ = write!(
        out,
        "\n# --- grain and fields ------------------------------------------------\n\
         # max_sacks_per_field is 10. The printed manual says 5, twice, and the\n\
         # manual is wrong - docs/decisions.md C10.\n\
         # The labour divisors run the other way round than they look: the\n\
         # SMALLER divisor demands MORE labour, so turning Advanced Farming off\n\
         # makes sowing harder.\n\
         \n[kingdom.grain]\n\
         yield_per_sack = {}\n\
         max_sacks_per_field = {}\n\
         labour_divisor_advanced = {}\n\
         labour_divisor_basic = {}\n\
         \n[kingdom.field]\n\
         progress_max = {}\n\
         reclaim_per_season = {}\n",
        t.grain.yield_per_sack,
        t.grain.max_sacks_per_field,
        t.grain.labour_divisor_advanced,
        t.grain.labour_divisor_basic,
        t.field.progress_max,
        t.field.reclaim_per_season
    );

    let _ = write!(
        out,
        "\n# --- random events ---------------------------------------------------\n\
         \n[kingdom.event]\n\
         population_cap_pct = {}\n\
         first_year = {}\n",
        t.event.population_cap_pct, t.event.first_year
    );

    out.push_str(
        "\n# --- seasons ---------------------------------------------------------\n\
         # death_rate is a percentage added to the health death rate; dryness is\n\
         # the seasonal push on the per-county dryness accumulator. Slot 0 is\n\
         # the original's \"No Season\" and is never used.\n",
    );
    for (index, id) in SEASON_IDS.iter().enumerate() {
        let row = t.season[index];
        let _ = write!(
            out,
            "\n[kingdom.season.{id}]\nindex = {index}\ndeath_rate = {}\ndryness = {}\n",
            row.death_rate, row.dryness
        );
    }

    let _ = write!(
        out,
        "\n# --- rations ---------------------------------------------------------\n\
         # Food needed is ceil(people / divisor) * multiplier.\n\
         # health_delta is added to the health meter each season, by health\n\
         # band, in the band order below. Its last column is the rule players\n\
         # feel: perfect health decays under anything less than Double.\n\
         #\n\
         # ration_slope and ration_offset give the happiness a ration level is\n\
         # worth, as slope * level + offset. In the original this is not a\n\
         # table at all - it is the expression 3L - 8 at the end of\n\
         # Ration_Apply.\n\
         #\n\
         # ale_step_pct and ale_max are the ale ladder: one happiness per that\n\
         # percentage of the county's population in crowns, up to ale_max. The\n\
         # published figure is +1 per 20%; the binary divides by 10, so it is\n\
         # +1 per 10%. ale_max is BOTH the top rung and a cumulative cap that\n\
         # nothing ever resets - five happiness from ale per county for the\n\
         # whole game, not five a season.\n\
         #\n\
         # army_cost is what raising men costs the county, indexed by the\n\
         # PERCENTAGE of its population taken, not by the number of men. 102\n\
         # rows, 0..=101, and the count is fixed: in the original the row after\n\
         # the last is the first merchant price.\n\
         \n[kingdom.happiness]\n\
         ration_slope = {}\n\
         ration_offset = {}\n\
         ale_step_pct = {}\n\
         ale_max = {}\n\
         army_cost = [\n{}\n]\n",
        t.ration_happiness_slope,
        t.ration_happiness_offset,
        t.ale.step_pct,
        t.ale.max,
        wrap_i32(&t.army_happiness_cost, 16)
    );
    for (index, id) in RATION_IDS.iter().enumerate() {
        let row = t.ration[index];
        let _ = write!(
            out,
            "\n[kingdom.ration.{id}]\nindex = {index}\ndivisor = {}\nmultiplier = {}\n\
             health_delta = [{}]\n",
            row.divisor,
            row.multiplier,
            join_i32(&row.health_delta)
        );
    }

    let _ = write!(
        out,
        "\n# --- health ----------------------------------------------------------\n\
         # band_ladder holds the INCLUSIVE upper bound of each band, one entry\n\
         # per band. A meter of 65 bands as 2, which is what pins the comparison\n\
         # as <= rather than <. The last entry is a real bound in the game's own\n\
         # table rather than an `else`: five bands, five bounds.\n\
         \n[kingdom.health]\n\
         band_ladder = [{}]\n",
        join_i32(&t.health_band_ladder.map(|(bound, _)| bound))
    );
    for (index, id) in HEALTH_BAND_IDS.iter().enumerate() {
        let row = t.health[index];
        let _ = write!(
            out,
            "\n[kingdom.health.band.{id}]\nindex = {index}\nhappiness = {}\ndeath_rate = {}\n",
            row.happiness, row.death_rate
        );
    }

    out.push_str(
        "\n# --- population ------------------------------------------------------\n\
         # birth_rate: the first row whose up_to is at least the county's\n\
         # population gives the base birth rate. This is the soft population\n\
         # cap players describe - at 2,000 people the 2% birth rate cannot keep\n\
         # up with winter's 8% death rate.\n",
    );
    for &(up_to, percent) in &t.population.birth_rate_ladder {
        let _ = write!(
            out,
            "\n[[kingdom.population.birth_rate]]\nup_to = {up_to}\npercent = {percent}\n"
        );
    }
    out.push_str(
        "\n# happiness_factor scales the birth rate. The last row is the\n\
         # catch-all and its threshold is never read; it is written as i32::MAX\n\
         # so that a reader which does compare it still gets the right answer.\n",
    );
    for &(below, percent) in &t.population.happiness_factor_ladder {
        let _ = write!(
            out,
            "\n[[kingdom.population.happiness_factor]]\nbelow = {below}\npercent = {percent}\n"
        );
    }

    out.push_str(
        "\n# --- weather ---------------------------------------------------------\n\
         # The percentage swing weather puts on the herd. The signs match\n\
         # L2.eng group 66's own descriptions one for one: only Sunny boosts\n\
         # crops, only Cloudy is neutral.\n",
    );
    for (index, id) in WEATHER_IDS.iter().enumerate() {
        let _ = write!(
            out,
            "\n[kingdom.weather.{id}]\nindex = {index}\nherd_pct = {}\n",
            t.weather[index].herd_pct
        );
    }

    let _ = write!(
        out,
        "\n# --- tax -------------------------------------------------------------\n\
         # happiness_other is county +0x16, the \"Other counties\" term: what one\n\
         # county's tax rate does to the mood of every OTHER county in the same\n\
         # realm. One row per tax rate, so the array's length IS the tax\n\
         # ceiling - 51 rows means 0 to 50, and the panel's up arrow stops\n\
         # there. Flat zero to rate 19: taxing at 19%% costs the rest of the\n\
         # realm nothing at all.\n\
         # The local half of the term, 5 - rate, is arithmetic rather than a\n\
         # table and is not here.\n\
         \n[kingdom.tax]\n\
         happiness_other = [\n{}\n]\n",
        wrap_i32(&t.tax_happiness_other, 17)
    );

    let _ = write!(
        out,
        "\n# --- the herd --------------------------------------------------------\n\
         # Cattle need tending. labour_per_head is 3: staffing is\n\
         # PctOf(labour, herd * 3), capped at staffing_max, and every point\n\
         # BELOW 100 adds a third of a point to the death rate. A herd nobody\n\
         # works loses about a third of a percent of its head a season on top\n\
         # of crowding.\n\
         # A county with no pasture at all loses half its herd - or all of it\n\
         # below no_pasture_kill_all_below head - and nothing else applies.\n\
         # calving_season multiplies births by season_bonus and culling_season\n\
         # multiplies deaths; 0 is \"No Season\" and turns the bonus off.\n\
         \n[kingdom.herd]\n\
         labour_per_head = {}\n\
         staffing_max = {}\n\
         understaffing_divisor = {}\n\
         no_pasture_density = {}\n\
         no_pasture_kill_all_below = {}\n\
         no_pasture_divisor = {}\n\
         calving_season = {}\n\
         culling_season = {}\n\
         season_bonus_numerator = {}\n\
         season_bonus_denominator = {}\n",
        t.herd.labour_per_head,
        t.herd.staffing_max,
        t.herd.understaffing_divisor,
        t.herd.no_pasture_density,
        t.herd.no_pasture_kill_all_below,
        t.herd.no_pasture_divisor,
        t.herd.calving_season,
        t.herd.culling_season,
        t.herd.season_bonus.0,
        t.herd.season_bonus.1
    );
    out.push_str(
        "\n# Crowding is head per pasture field, banded. level is what the\n\
         # county stores and what L2.eng group 77 names - \"Low herd crowding.\"\n\
         # through \"Massive overcrowding!!\" - and the two rates are per ten\n\
         # thousand head. An overcrowded herd dies seven times as fast AND\n\
         # breeds a seventh as often, which is why a county cannot simply keep\n\
         # buying cattle. The last row is the catch-all.\n",
    );
    for row in &t.herd.crowding {
        let _ = write!(
            out,
            "\n[[kingdom.herd.crowding]]\ndensity_max = {}\nlevel = {}\n\
             death_rate = {}\nbirth_rate = {}\n",
            row.density_max, row.level, row.death_rate, row.birth_rate
        );
    }
    out.push_str(
        "\n# A small, FULLY STAFFED herd breeds faster, which is what lets a\n\
         # county that has lost almost everything recover. Tried in order.\n",
    );
    for &(below, bonus) in &t.herd.small_bonus {
        let _ = write!(out, "\n[[kingdom.herd.small_bonus]]\nbelow = {below}\nbonus = {bonus}\n");
    }

    let _ = write!(
        out,
        "\n# --- castles ---------------------------------------------------------\n\
         # tax_base is what Tax_CollectAll multiplies the population by. In the\n\
         # original these six are immediates in the instruction stream, not a\n\
         # table. Type 0 is \"no castle\" and has nothing to build.\n\
         \n[kingdom.castle]\n\
         starting_type = {}\n",
        t.castle.starting_type
    );
    for (index, id) in CASTLE_IDS.iter().enumerate() {
        let _ = write!(
            out,
            "\n[kingdom.castle.type.{id}]\nindex = {index}\ntax_base = {}\n",
            t.castle.tax_base[index]
        );
        if index == 0 {
            continue;
        }
        let b = index - 1;
        let _ = write!(
            out,
            "tax_bonus_pct = {}\ncost_wood = {}\ncost_stone = {}\nworkforce = {}\n\
             garrison_cap = {}\nfree_archers = {}\n",
            t.castle.tax_bonus_pct[b],
            t.castle.cost[b].0,
            t.castle.cost[b].1,
            // The first of the two columns; both hold the same number.
            t.castle.workforce[b].0,
            t.castle.garrison_cap[b],
            t.castle.free_archers[b]
        );
    }

    let _ = write!(
        out,
        "\n# --- jobs and industry -----------------------------------------------\n\
         # Six of the nine job slots are established; castle_building is NOT.\n\
         # docs/kingdom.md never says which slot holds it, and the value is\n\
         # this engine's placeholder so the rule can be written and tested at\n\
         # all. cattle_farming IS established: Herd_SeasonTick passes county\n\
         # +0xD0, which is record 1, and the shipped save's nine labour records\n\
         # sum to the county's population in all fourteen counties.\n\
         \n[kingdom.job]\n\
         count = {}\n\
         iron_mining = {}\n\
         stone_quarrying = {}\n\
         wood_cutting = {}\n\
         blacksmith = {}\n\
         grain_farming = {}\n\
         cattle_farming = {}\n\
         castle_building = {}\n",
        t.job.count,
        t.job.iron_mining,
        t.job.stone_quarrying,
        t.job.wood_cutting,
        t.job.blacksmith,
        t.job.grain_farming,
        t.job.cattle_farming,
        t.job.castle_building
    );
    let _ = write!(
        out,
        "\n# The efficiency ramp's two bounds. Efficiency COMPOUNDS season on\n\
         # season towards max, so an industry is worth more the longer it has\n\
         # run and a county that is conquered and restarted is not. With\n\
         # Advanced Farming off none of that happens and every industry sits\n\
         # flat at without_advanced_farming - which is the shipped save's\n\
         # setting, so it is the ramp most games actually see.\n\
         \n[kingdom.efficiency]\n\
         max = {}\n\
         without_advanced_farming = {}\n",
        t.efficiency.max, t.efficiency.without_advanced_farming
    );

    out.push_str(
        "\n# The four passes Industry_Produce makes per county per season, in\n\
         # the order it makes them. divisor is applied to the worker count\n\
         # before the efficiency percentage - iron and wood harvest at twice\n\
         # the quantity of stone, which is exactly this column.\n",
    );
    for (index, id) in COMMODITY_IDS.iter().enumerate() {
        let row = t.commodity[index];
        let _ = write!(
            out,
            "\n[kingdom.commodity.{id}]\nindex = {index}\njob = {}\ndivisor = {}\n\
             base_efficiency = {}\n",
            row.job, row.divisor, row.base_efficiency
        );
    }

    out.push_str(
        "\n# --- weapons ---------------------------------------------------------\n\
         # Wood and iron to forge one. A bow needs no iron; armour costs the\n\
         # most of anything.\n",
    );
    for (index, id) in WEAPON_IDS.iter().enumerate() {
        let row = t.weapon[index];
        let _ = write!(
            out,
            "\n[kingdom.weapon.{id}]\nindex = {index}\nwood = {}\niron = {}\n",
            row.wood, row.iron
        );
    }

    out.push_str(
        "\n# --- trade -----------------------------------------------------------\n\
         # The merchant's base SELL price. The buy price the scroll shows is\n\
         # twice this. Sheep and wool are priced zero because they are the two\n\
         # goods a county cannot produce or trade in the base game. Slot 0 is\n\
         # not a good.\n",
    );
    for (index, id) in GOOD_IDS.iter().enumerate() {
        let _ = write!(
            out,
            "\n[kingdom.good.{id}]\nindex = {index}\nsell_price = {}\n",
            t.good[index].sell_price
        );
    }

    let _ = write!(
        out,
        "\n# --- wages -----------------------------------------------------------\n\
         # A human pays men / 4. Troop type does not enter it: a knight and a\n\
         # peasant cost the same. divisor_ai is by AI difficulty.\n\
         \n[kingdom.wages]\n\
         divisor_human = {}\n\
         divisor_ai = [{}]\n\
         bankrupt_stage_max = {}\n",
        t.wages.divisor_human,
        join_i32(&t.wages.divisor_ai),
        t.wages.bankrupt_stage_max
    );

    let _ = write!(
        out,
        "\n# --- the AI's advantages ---------------------------------------------\n\
         # Row 0 being all zeros is the load-bearing part: the human's lord\n\
         # byte is 0, so the human gets nothing. Rows 1..3 are NOT documented\n\
         # and are zeroed here rather than invented - a known gap, not a rule.\n\
         \n[kingdom.ai]\n\
         grant_population_per_difficulty = {}\n\
         grant_herd_per_difficulty = {}\n\
         grant_grain_per_difficulty = {}\n\
         grant_min_population = {}\n\
         grant_min_herd = {}\n\
         grant_min_grain = {}\n",
        t.ai.grant_population_per_difficulty,
        t.ai.grant_herd_per_difficulty,
        t.ai.grant_grain_per_difficulty,
        t.ai.grant_min_population,
        t.ai.grant_min_herd,
        t.ai.grant_min_grain
    );
    for (lord, row) in t.ai.gold_grant.iter().enumerate() {
        let _ = write!(
            out,
            "\n[[kingdom.ai.gold_grant]]\nlord = {lord}\nby_difficulty = [{}]\n",
            join_i32(row)
        );
    }

    out.push_str(
        "\n# The four tax ladders. In the original these are not tables at all -\n\
         # they are four if/else-if chains inside AI_SetTaxRates, which is why\n\
         # docs/kingdom.md sec 8.2 says the ladders exist and does not give\n\
         # them. Each row is `happiness below this -> tax this rate`, walked in\n\
         # order; the last row is the catch-all and its threshold is never\n\
         # read. A ladder may state 1 to 8 rows and the last is repeated to\n\
         # fill, so only the neutral ladder writes all eight.\n\
         #\n\
         # tax_ladder_neutral is what unowned counties pay, set once a turn in\n\
         # phase 1. Note it charges 1% at 20 happiness where every lord's own\n\
         # ladder charges nothing below 30 - nobody collects it, though:\n\
         # Tax_CollectAll banks an unowned county's take into the county.\n",
    );
    for &(below, rate) in tax_ladder_rows(&t.ai.tax_ladder_neutral) {
        let _ = write!(out, "\n[[kingdom.ai.tax_ladder_neutral]]\nbelow = {below}\nrate = {rate}\n");
    }
    out.push_str(
        "\n# The three an AI realm picks between, by its lord's personality.\n\
         # 0 is the greediest - 15% on a happy county. 2 is the gentlest, the\n\
         # only one charging nothing below 60, and the one three lords use.\n",
    );
    for (i, ladder) in t.ai.tax_ladders.iter().enumerate() {
        for &(below, rate) in tax_ladder_rows(ladder) {
            let _ =
                write!(out, "\n[[kingdom.ai.tax_ladder.{i}]]\nbelow = {below}\nrate = {rate}\n");
        }
    }

    out.push_str(
        "\n# One personality record per AI lord, in lord order. Two of the six\n\
         # ints in each record are identified and those two are here.\n\
         #\n\
         # farm_style LOADS AND DOES NOTHING. AI_ManageFields copies it into\n\
         # county +0x1FE and dispatches into one of three labour allocators\n\
         # that were never traced, so this engine has no behaviour to attach\n\
         # to it. It is here because it is half of the record, not because\n\
         # changing it will change a game. tax_ladder does take effect.\n\
         #\n\
         # There are four records and not five: docs/kingdom.md sec 2 says the\n\
         # lord byte runs 1..5, but a fifth record's bytes read a farm style of\n\
         # 17 where every real one reads 0, 1 or 9. A realm whose lord names no\n\
         # record sets no tax rates at all.\n",
    );
    for (i, row) in t.ai.personality.iter().enumerate() {
        let _ = write!(
            out,
            "\n[[kingdom.ai.personality]]\nlord = {}\nfarm_style = {}\ntax_ladder = {}\n",
            i + 1,
            row.farm_style,
            row.tax_ladder
        );
    }

    out.push_str(
        "\n# --- score -----------------------------------------------------------\n\
         # The gold brackets are the one term of the score whose meaning is\n\
         # unambiguous. The six weights apply to six realm fields NONE of which\n\
         # docs/kingdom.md could identify, so they are named by their raw\n\
         # offset rather than given invented names.\n",
    );
    for &(at_least, points) in &t.score.gold_brackets {
        let _ = write!(
            out,
            "\n[[kingdom.score.gold_bracket]]\nat_least = {at_least}\npoints = {points}\n"
        );
    }
    for i in 0..t.score.weights.len() {
        let (numerator, denominator) = t.score.weights[i];
        let _ = write!(
            out,
            "\n[[kingdom.score.weight]]\noffset = 0x{:02X}\nnumerator = {numerator}\n\
             denominator = {denominator}\n",
            t.score.input_offsets[i]
        );
    }

    out
}

fn join_i32(values: &[i32]) -> String {
    values.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", ")
}

/// A long integer array, wrapped at `per_line` values and indented, so a
/// hundred-entry table is readable rather than one enormous line.
fn wrap_i32(values: &[i32], per_line: usize) -> String {
    values
        .chunks(per_line)
        .map(|chunk| format!("  {},", join_i32(chunk)))
        .collect::<Vec<_>>()
        .join("\n")
}
