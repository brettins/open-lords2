//! 1. **The build model.** `work_done` climbs by exactly the besieger's 43 men
//!    a season — 86, 129, 172, 200 — its percentage is `work * 100 / 200`
//!    truncated, and `+0x19C` is `ceil(remaining / 43)` at every step. So
//! `g_siegeEngineWork[0]` really is **200** and the catapult really is
//!    record 0.
//!
//! 2. **`+0x198` and `+0x199` are different fields.** The besieger carries
//!    `garrison_county = 0` and `besieging_county = 4`; the garrison carries
//!    `garrison_county = 4` and `besieged_by = 5`. Nothing here was ever
//!    checked against a position where the two could be told apart.

mod siege_tests;
pub use siege_tests::*;

use l2_formats::save::Save;
use l2_kingdom::battle::{self, CASTLE_STRENGTH_PERCENT};
use l2_kingdom::siege::{self, Engine, ENGINE_WORK};
use l2_kingdom::unit::{Unit, UnitKind, Units, TROOP_TYPES};

/// `g_units`, `0x0052F0B0`, stride `0x1A4` — `docs/armies.md` §1.
const UNIT_BASE: u32 = 0x0052_F0B0;
const UNIT_STRIDE: u32 = 0x1A4;

const BESIEGER: u32 = 5;
const GARRISON: u32 = 4;
const BESIEGED_COUNTY: u32 = 4;

fn w16(save: &Save, at: u32) -> i32 {
    let lo = save.u8_at(at).unwrap_or(0) as i32;
    let hi = save.u8_at(at + 1).unwrap_or(0) as i32;
    lo | (hi << 8)
}

struct SavedUnit {
    owner: u8,
    kind: u8,
    men: i32,
    garrison_county: u8,
    besieging_county: u8,
    besieged_by: u8,
    seasons_left: u8,
    /// `+0x182 + e*6`, as `(ordered, percent, work_done)`.
    engines: [(i32, i32, i32); 3],
    /// All **eleven** `+0x16C` counts — the four battle-only slots included,
    /// which is the point of reading them here.
    troops: [i32; 11],
}

fn unit(save: &Save, slot: u32) -> SavedUnit {
    let b = UNIT_BASE + slot * UNIT_STRIDE;
    SavedUnit {
        owner: save.u8_at(b).unwrap(),
        kind: save.u8_at(b + 0x08).unwrap(),
        men: save.i32_at(b + 0x168).unwrap(),
        garrison_county: save.u8_at(b + 0x198).unwrap(),
        besieging_county: save.u8_at(b + 0x199).unwrap(),
        besieged_by: save.u8_at(b + 0x19A).unwrap(),
        seasons_left: save.u8_at(b + 0x19C).unwrap(),
        engines: core::array::from_fn(|e| {
            let r = b + 0x182 + e as u32 * 6;
            (w16(save, r), w16(save, r + 2), w16(save, r + 4))
        }),
        troops: core::array::from_fn(|t| w16(save, b + 0x16C + t as u32 * 2)),
    }
}

fn castle_type(save: &Save) -> u8 {
    let cb = l2_formats::save::COUNTY_BASE
        + BESIEGED_COUNTY * l2_formats::save::COUNTY_STRIDE as u32;
    save.u8_at(cb + 0x1C0).unwrap()
}

fn garrison_slot(save: &Save) -> i32 {
    let cb = l2_formats::save::COUNTY_BASE
        + BESIEGED_COUNTY * l2_formats::save::COUNTY_STRIDE as u32;
    save.i32_at(cb + 0x1BC).unwrap()
}

