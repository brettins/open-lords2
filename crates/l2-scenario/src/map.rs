#![allow(unused_imports)]
use super::*;
use super::save_helpers::*;
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

/// One county's twenty field tiles, converted from byte offsets to tile
/// indices.
///
/// An offset that is not a multiple of eight, or that lands outside the
/// 64 × 64 plane, is a misread and not a field: it becomes an **empty slot**
/// Nothing in the fixture takes that path
/// — `crates/l2-scenario/tests/import.rs` asserts every populated slot is a
/// real tile —
/// panicking somewhere else later.
fn read_field_tiles(save: &Save, county: usize) -> Result<[u16; MAX_FIELDS], SaveError> {
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
fn read_map(save: &Save) -> Result<CampaignMap, SaveError> {
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

fn read_explored(save: &Save, local_player: u8) -> Result<Explored, SaveError> {
    let mut explored = Explored::new();
    for tile in 0..MAP_TILES {
        if save.u8_at(TILES + tile as u32 * TILE_STRIDE + TILE_BANK)? & BANK_SEEN != 0 {
            explored.set_seen(local_player, tile);
        }
    }
    Ok(explored)
}

