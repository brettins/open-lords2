#![allow(unused_imports)]
use super::*;
use super::render::*;
use crate::ruleset::{RuleError, Ruleset};
use l2_kingdom::tables::{
    AiPersonalityRow, AiTable, AleTable, CastleTable, CommodityRow, EfficiencyTable, EventTable,
    FieldTable, FoodTable, GoodRow, GrainTable, HealthBandRow, HerdCrowdingRow, HerdTable,
    JobTable, PopulationTable, RationRow, ScoreTable, SeasonRow, Tables, TaxLadder, WageTable,
    WeaponRow, WeatherRow, AI_PERSONALITY_COUNT, AI_TAX_LADDER_COUNT, ARMY_HAPPINESS_COST_LEN,
    CASTLE_TYPE_COUNT, HERD_CROWDING_COUNT, TAX_LADDER_RUNGS,
};

/// Build a [`Tables`] out of a merged ruleset.
///
/// Every value is required. There are no defaults here on purpose: a kingdom
/// rule that silently falls back to a built-in number is a rule a mod cannot
/// tell it failed to set,
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
        // Multipliers, not divisors: `Grain_Grow` and `Grain_Harvest` cap the
        // crop at `labour * this`. Zero is a county that can never tend or
        // reap anything, which is a legitimate — if bleak — ruleset.
        grow_per_worker_advanced: int(rs, "kingdom.grain.grow_per_worker_advanced", 0, 10_000)?,
        harvest_per_worker_advanced: int(
            rs,
            "kingdom.grain.harvest_per_worker_advanced",
            0,
            10_000,
        )?,
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
// zero.
        step_pct: int(rs, "kingdom.happiness.ale_step_pct", 1, 10_000)?,
        // The rung count and the cumulative cap are one number,
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
// own layout,
    // `tools/oracle/kingdom.ps1` checks that against the executable.
    //
    // The ruleset carries only the bounds, because in the shipped table the band
    // column always equals its own row index (10,0  35,1  65,2  90,3  100,4).
    // That is a property of the data, not a rule,
// bands would need the format to grow a column
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
// is therefore structure: a document with a different
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
// The shape is kept so the arrays still match the
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
// been traced, so it is carried.
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

    let mut personality = [AiPersonalityRow {
        farm_style: 0,
        tax_ladder: 0,
        gift_increment: 0,
        help_price: 0,
        grudge_tolerance: 0,
        offer_interval: 0,
        help_population_floor: 0,
        muster_pct: 0,
        muster_patience: 0,
        muster_arms: 0,
        garrison_min_population: 0,
        raid_interval: 0,
        abandon_tax_rate: 0,
        castle_concurrent: 0,
        castle_min_population: 0,
        castle_gold: [0; CASTLE_TYPE_COUNT - 1],
        weapon_rota: [0; 6],
        siege_doctrine: 0,
        trade_gold_floor: 0,
        weapon_buy_qty: 0,
        reserve_wood: 0,
        reserve_stone: 0,
        reserve_iron: 0,
    }; AI_PERSONALITY_COUNT];
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
        // `castle_gold` is one threshold per castle type 1..=5, and a zero means
// that type is never offered to this lord — so the Baron and
        // the Countess never build a royal castle at any treasury.
        let gold = rs.integer_array(&format!("{base}.castle_gold"), CASTLE_TYPE_COUNT - 1)?;
        let mut castle_gold = [0i32; CASTLE_TYPE_COUNT - 1];
        for (t, &v) in gold.iter().enumerate() {
            castle_gold[t] = narrow(rs, &format!("{base}.castle_gold"), v, 0, 1_000_000)?;
        }

        // The lord's weapon programme: six weapon types, stepped through by AI
        // turn step 12. Out-of-range entries would index past the weapon table,
        // so the bound is the table's length.
        let rota = rs.integer_array(&format!("{base}.weapon_rota"), 6)?;
        let mut weapon_rota = [0usize; 6];
        for (i, &v) in rota.iter().enumerate() {
            weapon_rota[i] =
                narrow(rs, &format!("{base}.weapon_rota"), v, 0, l2_kingdom::tables::WEAPON_TYPE_COUNT as i64 - 1)?
                    as usize;
        }

        *slot = AiPersonalityRow {
            farm_style: int(rs, &format!("{base}.farm_style"), 0, 255)? as u8,
            weapon_rota,
            tax_ladder: int(rs, &format!("{base}.tax_ladder"), 0, AI_TAX_LADDER_COUNT as i64 - 1)?
                as usize,
            // A zero increment divides by zero in the gift ratchet.
            gift_increment: int(rs, &format!("{base}.gift_increment"), 1, 1_000_000)?,
            help_price: int(rs, &format!("{base}.help_price"), 0, 1_000_000)?,
            grudge_tolerance: int(rs, &format!("{base}.grudge_tolerance"), 0, 10_000)?,
            // A zero interval would offer every turn, which is a rebalance
            // so it is allowed.
            offer_interval: int(rs, &format!("{base}.offer_interval"), 0, 10_000)?,
            help_population_floor: int(rs, &format!("{base}.help_population_floor"), 0, 1_000_000)?,
            muster_pct: int(rs, &format!("{base}.muster_pct"), 0, 100)?,
            // The five fields AI steps 7, 9 and 10 read
            // (`l2_kingdom::ai_army`). A zero patience musters every turn and
            // a zero raid interval raids every turn; both are rebalances
            // so the low bound is open.
            muster_patience: int(rs, &format!("{base}.muster_patience"), 0, 10_000)?,
            muster_arms: int(rs, &format!("{base}.muster_arms"), 0, 1_000_000)?,
            garrison_min_population: int(
                rs,
                &format!("{base}.garrison_min_population"),
                0,
                1_000_000,
            )?,
            raid_interval: int(rs, &format!("{base}.raid_interval"), 0, 255)?,
            // The tax rate a written-off county is set to. Bounded by
            // `MAX_TAX_RATE` because it is written straight into
            // `County::tax_rate`, which indexes the tax-happiness table.
            abandon_tax_rate: int(
                rs,
                &format!("{base}.abandon_tax_rate"),
                0,
                l2_kingdom::tables::MAX_TAX_RATE as i64,
            )?,
            castle_concurrent: int(rs, &format!("{base}.castle_concurrent"), 0, 100)?,
            castle_min_population: int(rs, &format!("{base}.castle_min_population"), 0, 1_000_000)?,
            castle_gold,
            // Personality `+0xA0`: 7, 8 or 9 selects the lord's siege-engine
            // order and anything else falls to the default of two towers, so
// the range is deliberately open.
            siege_doctrine: int(rs, &format!("{base}.siege_doctrine"), 0, 255)?,
            // `Ai_TradeForCounty` (`0x0049E39B`): personality `+0x78`, `+0x7C`
            // and the three reserves `+0x84`/`+0x88`/`+0x8C`.
            // buys weapons on any positive treasury and a zero reserve sells
            // the realm bare; both are rebalances, so the low bound is open.
            trade_gold_floor: int(rs, &format!("{base}.trade_gold_floor"), 0, 1_000_000)?,
            weapon_buy_qty: int(rs, &format!("{base}.weapon_buy_qty"), 0, 1_000_000)?,
            reserve_wood: int(rs, &format!("{base}.reserve_wood"), 0, 1_000_000)?,
            reserve_stone: int(rs, &format!("{base}.reserve_stone"), 0, 1_000_000)?,
            reserve_iron: int(rs, &format!("{base}.reserve_iron"), 0, 1_000_000)?,
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

