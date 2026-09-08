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
//! Loading works, is validated, and round-trips against
//! [`Tables::DEFAULT`](l2_kingdom::tables::Tables::DEFAULT) in a test. What has
//! *not* happened is the other half: the simulation modules in `l2-kingdom`
//! still read the module-level constants directly rather than a `Tables` they
//! were handed. So a modded `kingdom.toml` is loaded, checked and reported —
//! and then not consumed. Threading `&Tables` through ~30 free functions is a
//! mechanical change to that crate's public API, deliberately not made here.
//! `docs/modding.md` says the same thing in the same words; it is stated in
//! both places so neither can quietly claim more than is true.
//!
//! The battle side, `unit.*` in [`crate::units`], *is* wired all the way
//! through — see `l2_sim::Battle::with_troops`.

use crate::ruleset::{RuleError, Ruleset};
use l2_kingdom::tables::{
    AiTable, CastleTable, CommodityRow, EventTable, FieldTable, FoodTable, GoodRow, GrainTable,
    HealthBandRow, JobTable, PopulationTable, RationRow, ScoreTable, SeasonRow, Tables, WageTable,
    WeaponRow, WeatherRow,
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
        castle_building: slot("castle_building")?,
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
         # HONEST SCOPE: l2-mods loads and validates this file, and reports\n\
         # what a mod changed in it. The l2-kingdom simulation still reads its\n\
         # own constants rather than the table built from here. See\n\
         # docs/modding.md.\n",
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
         \n[kingdom.happiness]\n\
         ration_slope = {}\n\
         ration_offset = {}\n",
        t.ration_happiness_slope, t.ration_happiness_offset
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
         # Only four of the ten job slots are established. grain_farming and\n\
         # castle_building are NOT: docs/kingdom.md never says which slot holds\n\
         # them, and these two values are this engine's placeholders so the\n\
         # rules can be written and tested at all.\n\
         \n[kingdom.job]\n\
         count = {}\n\
         iron_mining = {}\n\
         stone_quarrying = {}\n\
         wood_cutting = {}\n\
         blacksmith = {}\n\
         grain_farming = {}\n\
         castle_building = {}\n",
        t.job.count,
        t.job.iron_mining,
        t.job.stone_quarrying,
        t.job.wood_cutting,
        t.job.blacksmith,
        t.job.grain_farming,
        t.job.castle_building
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
