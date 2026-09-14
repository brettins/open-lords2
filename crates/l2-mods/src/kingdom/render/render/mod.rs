#![allow(unused_imports)]

mod toml;
pub use toml::*;

use super::*;

use super::*;
use super::tables::*;
use crate::ruleset::{RuleError, Ruleset};
use l2_kingdom::tables::{
    AiPersonalityRow, AiTable, AleTable, CastleTable, CommodityRow, EfficiencyTable, EventTable,
    FieldTable, FoodTable, GoodRow, GrainTable, HealthBandRow, HerdCrowdingRow, HerdTable,
    JobTable, PopulationTable, RationRow, ScoreTable, SeasonRow, Tables, TaxLadder, WageTable,
    WeaponRow, WeatherRow, AI_PERSONALITY_COUNT, AI_TAX_LADDER_COUNT, ARMY_HAPPINESS_COST_LEN,
    CASTLE_TYPE_COUNT, HERD_CROWDING_COUNT, TAX_LADDER_RUNGS,
};

fn join_i32(values: &[i32]) -> String {
    values.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", ")
}

/// A long integer array, wrapped at `per_line` values and indented,
/// hundred-entry table is readable.
fn wrap_i32(values: &[i32], per_line: usize) -> String {
    values
        .chunks(per_line)
        .map(|chunk| format!("  {},", join_i32(chunk)))
        .collect::<Vec<_>>()
        .join("\n")
}


