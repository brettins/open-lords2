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
//! What is *not* covered is worth naming:
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

/// One `AI_SetTaxRates` ladder: `{below, rate}` rows, walked in order, taking
/// the first row the county's happiness is strictly below.
///
/// The simulation's ladder is a fixed [`TAX_LADDER_RUNGS`]-row array because
/// the neutral ladder needs all eight, but the three personality ladders use
/// five, five and six.
/// `below = 2147483647`
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

