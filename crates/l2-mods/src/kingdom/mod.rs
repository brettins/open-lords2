//! `docs/decisions.md` C11 in one sentence: *every* economic constant — tax,
//! happiness, rations, births, deaths, yields, wages — lives in `Lords2.exe`,
//! clustered around `0x004D6300` and `0x004D8900`, and nowhere else. There is
//! no data file to edit. Modding the 1996 game's economy means patching a
//! binary, which is the strongest single argument for building an open engine
//! at all.

mod tables;
pub use tables::*;
mod render;
pub use render::*;

use crate::ruleset::{RuleError, Ruleset};
use l2_kingdom::tables::{
    AiPersonalityRow, AiTable, AleTable, CastleTable, CommodityRow, EfficiencyTable, EventTable,
    FieldTable, FoodTable, GoodRow, GrainTable, HealthBandRow, HerdCrowdingRow, HerdTable,
    JobTable, PopulationTable, RationRow, ScoreTable, SeasonRow, Tables, TaxLadder, WageTable,
    WeaponRow, WeatherRow, AI_PERSONALITY_COUNT, AI_TAX_LADDER_COUNT, ARMY_HAPPINESS_COST_LEN,
    CASTLE_TYPE_COUNT, HERD_CROWDING_COUNT, TAX_LADDER_RUNGS,
};

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

pub const COMMODITY_IDS: [&str; 4] = ["wood", "iron", "weapons", "stone"];

pub const WEAPON_IDS: [&str; 6] = ["crossbow", "mace", "sword", "pike", "bow", "armour"];

/// Tradeable good ids, `L2.eng` group 6.
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

fn int(rs: &Ruleset, path: &str, lo: i64, hi: i64) -> Result<i32, RuleError> {
    Ok(rs.integer_in(path, lo, hi)? as i32)
}

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

fn tax_ladder_rows(ladder: &TaxLadder) -> &[(i32, i32)] {
    let mut len = ladder.len();
    while len > 1 && ladder[len - 1] == ladder[len - 2] {
        len -= 1;
    }
    &ladder[..len]
}

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

