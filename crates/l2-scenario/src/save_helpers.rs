#![allow(unused_imports)]
use super::*;
use super::map::*;
use super::units::*;
use super::scenario_impl::*;
use l2_formats::save::{Save, SaveError, COUNTY_BASE, COUNTY_STRIDE, REALM_BASE, REALM_STRIDE};
use l2_kingdom::county::{County, MAX_COUNTY_ID, MAX_FIELDS};
use l2_kingdom::explore::Explored;
use l2_kingdom::map::MAP_TILES;
use l2_kingdom::merchant::{MerchantRoutes, ROUTES, ROUTE_SLOTS};
use l2_kingdom::mercenary::{Band, MercenaryBands, MERCENARY_BANDS, ROSTER};
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::{health_band, Tables, Weather, JOB_COUNT};
use l2_kingdom::unit::{Mercenaries, TroopType, Unit, UnitKind, Units, MAX_UNITS, TROOP_TYPES};
use l2_kingdom::{field, land, CampaignMap, Kingdom, Options};

/// One signed byte out of a county record, by offset.
fn county_i8(save: &Save, county: usize, offset: u32) -> Result<i32, SaveError> {
    Ok(save.i8_at(COUNTY_BASE + (county * COUNTY_STRIDE) as u32 + offset)? as i32)
}

/// One `i32` out of a county record, by offset.
fn county_i32(save: &Save, county: usize, offset: u32) -> Result<i32, SaveError> {
    save.i32_at(COUNTY_BASE + (county * COUNTY_STRIDE) as u32 + offset)
}

fn read_industry(save: &Save, county: usize) -> Result<[IndustryState; 4], SaveError> {
    let base = COUNTY_BASE + (county * COUNTY_STRIDE) as u32 + INDUSTRY_BASE;
    let mut out = [IndustryState::default(); 4];
    for (c, slot) in out.iter_mut().enumerate() {
        let record = base + c as u32 * INDUSTRY_STRIDE;
        *slot = IndustryState {
            has_resource: save.u8_at(record + INDUSTRY_HAS_RESOURCE)? != 0,
            disabled_seasons: save.u8_at(record + INDUSTRY_DISABLED_SEASONS)? as i32,
            enabled: save.u8_at(record + INDUSTRY_ENABLED)? != 0,
            next_season: save.i32_at(record + INDUSTRY_NEXT_SEASON)?,
            efficiency: save.u8_at(record)? as i32,
            capacity: save.i16_at(record + 0x0A)? as i32,
            total: save.i32_at(record + 0x0C)?,
            total_snapshot: save.i32_at(record + 0x10)?,
        };
    }
    Ok(out)
}

/// One word out of each of a county's nine labour records.
///
/// `word` is the byte offset inside the twelve-byte record: 0 is the workers
/// assigned, 4 the wanted floor, 8 the useful ceiling. All three are
/// read the same way because the record really is three plain `i32`s — which
/// is the whole reason the stride is twelve and not four.
fn read_labour(save: &Save, county: usize, word: u32) -> Result<[i32; JOB_COUNT], SaveError> {
    let base = COUNTY_BASE + (county * COUNTY_STRIDE) as u32 + LABOUR_BASE + word;
    let mut jobs = [0i32; JOB_COUNT];
    for (job, slot) in jobs.iter_mut().enumerate() {
        *slot = save.i32_at(base + job as u32 * LABOUR_STRIDE)?;
    }
    Ok(jobs)
}

