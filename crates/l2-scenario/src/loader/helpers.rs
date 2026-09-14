#![allow(unused_imports)]
use super::*;
use super::units::*;
use super::*;
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

/// One county's twenty field tiles, converted from byte offsets to tile
/// indices.
///
/// An offset that is not a multiple of eight, or that lands outside the
/// 64 × 64 plane, is a misread and not a field: it becomes an **empty slot**
/// Nothing in the fixture takes that path
/// — `crates/l2-scenario/tests/import/main.rs` asserts every populated slot is a
/// real tile —
/// panicking somewhere else later.
pub(super) fn read_field_tiles(save: &Save, county: usize) -> Result<[u16; MAX_FIELDS], SaveError> {
    let base = COUNTY_FIELD_TILES + county as u32 * COUNTY_FIELD_STRIDE;
    let mut tiles = [0u16; MAX_FIELDS];
    for (slot, out) in tiles.iter_mut().enumerate() {
        let offset = save.i32_at(base + slot as u32 * 4)?;
        if offset <= 0 || offset % TILE_STRIDE as i32 != 0 {
            continue;
        }
        let index = offset / TILE_STRIDE as i32;
        if (index as usize) < MAP_TILES {
            *out = index as u16;
        }
    }
    Ok(tiles)
}

/// `g_tiles`' terrain, flags and county planes, de-interleaved out of the
/// eight-byte records.
pub(super) fn read_map(save: &Save) -> Result<CampaignMap, SaveError> {
    let mut terrain = vec![0u8; MAP_TILES];
    let mut flags = vec![0u8; MAP_TILES];
    let mut bank = vec![0u8; MAP_TILES];
    let mut county = vec![0u8; MAP_TILES];
    for tile in 0..MAP_TILES {
        let base = TILES + tile as u32 * TILE_STRIDE;
        terrain[tile] = save.u8_at(base)?;
        flags[tile] = save.u8_at(base + 1)?;
        bank[tile] = save.u8_at(base + TILE_BANK)?;
        county[tile] = save.u8_at(base + 7)?;
    }
    Ok(CampaignMap::from_planes(&terrain, &flags, &bank, &county)
        .expect("four planes of MAP_TILES bytes each"))
}

pub(super) fn read_explored(save: &Save, local_player: u8) -> Result<Explored, SaveError> {
    let mut explored = Explored::new();
    for tile in 0..MAP_TILES {
        if save.u8_at(TILES + tile as u32 * TILE_STRIDE + TILE_BANK)? & BANK_SEEN != 0 {
            explored.set_seen(local_player, tile);
        }
    }
    Ok(explored)
}

/// One signed byte out of a county record, by offset.
pub(crate) fn county_i8(save: &Save, county: usize, offset: u32) -> Result<i32, SaveError> {
    Ok(save.i8_at(COUNTY_BASE + (county * COUNTY_STRIDE) as u32 + offset)? as i32)
}

/// One `i32` out of a county record, by offset.
pub(super) fn county_i32(save: &Save, county: usize, offset: u32) -> Result<i32, SaveError> {
    save.i32_at(COUNTY_BASE + (county * COUNTY_STRIDE) as u32 + offset)
}

pub(super) fn read_industry(save: &Save, county: usize) -> Result<[IndustryState; 4], SaveError> {
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
pub(crate) fn read_labour(save: &Save, county: usize, word: u32) -> Result<[i32; JOB_COUNT], SaveError> {
    let base = COUNTY_BASE + (county * COUNTY_STRIDE) as u32 + LABOUR_BASE + word;
    let mut jobs = [0i32; JOB_COUNT];
    for (job, slot) in jobs.iter_mut().enumerate() {
        *slot = save.i32_at(base + job as u32 * LABOUR_STRIDE)?;
    }
    Ok(jobs)
}

