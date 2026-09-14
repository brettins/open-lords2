//! **The siege model against the bytes of a real siege.**
//!
//! ```text
//! LORDS2_FIXTURES="E:\dev\lords2-fixtures" LORDS2_DIR="F:\games\Lords of the Realm II" \
//!   cargo test -p l2-kingdom --test siege
//! ```
//!
//! # What these five files are
//!
//! One siege, caught at five moments. `docs/armies.md` §4 was written entirely
//! from the decompiler because **there was no save with a castle under siege**;
//! there is now, and every number in this file comes out of it
//! of the document.
//!
//! | file | what it holds |
//! |---|---|
//! | `siege-safeturn.sav` | the catapult 43 % built, 3 seasons to go |
//! | `siege-old_turn.sav` | 64 %, 2 seasons |
//! | `siege-lastturn.sav` | 86 %, 1 season |
//! | `siege-sieging.sav` | 100 %, 0 seasons, **and the battle already staged** |
//! | `siege-aftersie.sav` | the assault resolved: the besieger gone, the garrison at 133 |
//!
//! The position: county 4 holds a **palisade** (`castleType` 1) garrisoned by
//! an AI army of **149**, and a human army of **43** is camped beside it
//! building one catapult.
//!
//! # The four things this settles that code alone could not
//!
//! 1. **The build model.** `work_done` climbs by exactly the besieger's 43 men
//!    a season — 86, 129, 172, 200 — its percentage is `work * 100 / 200`
//!    truncated, and `+0x19C` is `ceil(remaining / 43)` at every step. So
//! `g_siegeEngineWork[0]` really is **200** and the catapult really is
//!    record 0.
//! 2. **`+0x198` and `+0x199` are different fields.** The besieger carries
//!    `garrison_county = 0` and `besieging_county = 4`; the garrison carries
//!    `garrison_county = 4` and `besieged_by = 5`. Nothing here was ever
//!    checked against a position where the two could be told apart.
//! 3. **`Army_PrepareForBattle`.** `siege-sieging.sav` was taken with the
//!    battle staged: the besieger's troop slot **7** holds the one catapult it
//! built, and the garrison's slot **10** holds **one** oil — which is
//!    `OIL_BY_CASTLE_LEVEL[0]` for a palisade, the first arm of a five-way
//!    switch nobody had ever seen fire.
//! 4. **The castle bonus, uniquely.** The autocalc reproduces the aftermath to
//!    the man *only* at level 0. See
//!    [`the_castle_bonus_for_a_palisade_is_the_only_one_that_reproduces_the_aftermath`].

mod siege_tests;
pub use siege_tests::*;

use l2_formats::save::Save;
use l2_kingdom::battle::{self, CASTLE_STRENGTH_PERCENT};
use l2_kingdom::siege::{self, Engine, ENGINE_WORK};
use l2_kingdom::unit::{Unit, UnitKind, Units, TROOP_TYPES};

/// `g_units`, `0x0052F0B0`, stride `0x1A4` — `docs/armies.md` §1.
const UNIT_BASE: u32 = 0x0052_F0B0;
const UNIT_STRIDE: u32 = 0x1A4;

/// The besieging army and the garrison, by slot, in every one of the five
/// files. Named
/// fingerprint (`docs/environment.md`), and these two slots are part of the
/// fingerprint.
const BESIEGER: u32 = 5;
const GARRISON: u32 = 4;
const BESIEGED_COUNTY: u32 = 4;

fn w16(save: &Save, at: u32) -> i32 {
    let lo = save.u8_at(at).unwrap_or(0) as i32;
    let hi = save.u8_at(at + 1).unwrap_or(0) as i32;
    lo | (hi << 8)
}

/// One unit slot, as the fields a siege reads.
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

