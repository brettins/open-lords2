#![allow(unused_imports)]
use super::*;
use super::units::*;
use super::*;
use l2_formats::save::{CountyStored, Save, SaveError, TILE_STRIDE};
use l2_kingdom::county::{County, MAX_COUNTY_ID, MAX_FIELDS};
use l2_kingdom::explore::Explored;
use l2_kingdom::map::MAP_TILES;
use l2_kingdom::merchant::{MerchantRoutes, ROUTES, ROUTE_SLOTS};
use l2_kingdom::mercenary::{Band, MercenaryBands, MERCENARY_BANDS, ROSTER};
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::{health_band, Tables, Weather, JOB_COUNT};
use l2_kingdom::unit::{Mercenaries, TroopType, Unit, UnitKind, Units, MAX_UNITS, TROOP_TYPES};
use l2_kingdom::{field, land, CampaignMap, Kingdom, Options};

/// `g_countyFieldTiles` — `[V]` `County_RecountFields`' terrain ladder over these
/// tiles reproduces all three field counts of every England turn-one county
/// (`crates/l2-kingdom/tests/fields/main.rs`).
pub(super) fn read_field_tiles(s: &CountyStored) -> [u16; MAX_FIELDS] {
    let mut tiles = [0u16; MAX_FIELDS];
    for (out, &offset) in tiles.iter_mut().zip(s.field_tiles.iter()) {
        if offset <= 0 || offset % TILE_STRIDE as i32 != 0 {
            continue;
        }
        let index = offset / TILE_STRIDE as i32;
        if (index as usize) < MAP_TILES {
            *out = index as u16;
        }
    }
    tiles
}

pub(super) fn read_map(save: &Save) -> Result<CampaignMap, SaveError> {
    let mut terrain = vec![0u8; MAP_TILES];
    let mut flags = vec![0u8; MAP_TILES];
    let mut bank = vec![0u8; MAP_TILES];
    let mut county = vec![0u8; MAP_TILES];
    let mut frame = vec![0u8; MAP_TILES];
    for tile in 0..MAP_TILES {
        let t = save.tile(tile)?;
        terrain[tile] = t[0];
        flags[tile] = t[1];
        bank[tile] = t[TILE_BANK];
        county[tile] = t[7];
        frame[tile] = t[3];
    }
    let mut map = CampaignMap::from_planes(&terrain, &flags, &bank, &county)
        .expect("four planes of MAP_TILES bytes each");
    // Map_DrawTile 0x004063C1: DAT_005c9288 = g_tiles[t].frame, save block 0 [V].
    map.frame = frame;
    Ok(map)
}

pub(super) fn read_explored(save: &Save, local_player: u8) -> Result<Explored, SaveError> {
    let mut explored = Explored::new();
    for tile in 0..MAP_TILES {
        if save.tile(tile)?[TILE_BANK] & BANK_SEEN != 0 {
            explored.set_seen(local_player, tile);
        }
    }
    Ok(explored)
}

pub(super) fn read_industry(s: &CountyStored) -> [IndustryState; 4] {
    s.industry.map(|r| IndustryState {
        has_resource: r.has_resource != 0,
        disabled_seasons: r.disabled_seasons as i32,
        enabled: r.enabled != 0,
        next_season: r.next_season,
        efficiency: r.efficiency as i32,
        capacity: r.capacity as i32,
        total: r.total,
        total_snapshot: r.total_snapshot,
    })
}
