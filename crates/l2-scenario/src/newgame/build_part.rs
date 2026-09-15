#![allow(unused_imports)]
use super::*;
use super::sites::*;
use super::fields::*;
use super::setup::*;
use super::scenario::*;
use super::tests_part::*;
use l2_formats::maps::{MapSlot, Plane, PLANE_DIM};
use l2_kingdom::county::{MAX_COUNTIES, MAX_COUNTY_ID, MAX_FIELDS, MAX_NEIGHBOURS};
use l2_kingdom::map::{CampaignMap, MAP_TILES};
use l2_kingdom::merchant::{self, MerchantRoutes, ROUTES, ROUTE_SLOTS};
use l2_kingdom::mercenary::MercenaryBands;
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::{Weather, JOB_COUNT};
use l2_kingdom::unit::{Unit, UnitKind, Units};
use l2_kingdom::Options;
use crate::{Clock, CountyState, IndustryState, RealmState, Scenario};

impl Tiles {
    /// `FUN_0046C5B8`, six times: every byte of every record zeroed.
    pub fn blank() -> Tiles {
        Tiles {
            content: vec![0; MAP_TILES],
            flags: vec![0; MAP_TILES],
            bank: vec![0; MAP_TILES],
            frame: vec![0; MAP_TILES],
            part: vec![0; MAP_TILES],
            saved_frame: vec![0; MAP_TILES],
            county: vec![0; MAP_TILES],
        }
    }

    /// `Map_LoadPlanes` (`0x00467770`)'s copy half: five of the six planes
    /// straight into the record, in the same `y` outer / `x` inner nest.
    pub fn from_slot(slot: &MapSlot<'_>) -> Tiles {
        let mut t = Tiles::blank();
        for y in 0..PLANE_DIM {
            for x in 0..PLANE_DIM {
                let i = y * PLANE_DIM + x;
                t.flags[i] = slot.at(Plane::Flags, x, y);
                t.bank[i] = slot.at(Plane::GfxBank, x, y);
                t.frame[i] = slot.at(Plane::GfxIndex, x, y);
                t.part[i] = slot.at(Plane::ObjectPart, x, y);
                t.county[i] = slot.at(Plane::County, x, y);
            }
        }
        t
    }

    pub fn campaign_map(&self) -> CampaignMap {
        CampaignMap::from_planes(&self.content, &self.flags, &self.bank, &self.county)
            .expect("Tiles holds MAP_TILES of each")
    }

    pub(crate) fn bank_layer(&self, tile: usize) -> u8 {
        self.bank[tile] & BANK_LAYER
    }

    /// `FUN_0046AC22(frameBase, 2, tile, layerBit, content)` — stamp a 2×2
/// object. The bank is rebuilt: `(bank | 1) & 0xE3 | bit`.
    pub(super) fn stamp_2x2(&mut self, tile: usize, frame_base: u8, bank_bits: u8, content: u8) {
        for (n, offset) in [0usize, 1, PLANE_DIM, PLANE_DIM + 1].into_iter().enumerate() {
            let Some(t) = tile.checked_add(offset).filter(|t| *t < MAP_TILES) else { continue };
            self.bank[t] = ((self.bank[t] | 1) & 0xE3) | bank_bits;
            self.frame[t] = frame_base.wrapping_add(ISO_2X2[n]);
            self.content[t] = content;
        }
    }
}


pub fn build(slot: &MapSlot<'_>, setup: &NewGame) -> Result<MapWorld, MapError> {
    let mut w = load_planes(slot)?;
    adjacency(&mut w);
    place_sites(&mut w);
    place_starting_fields(&mut w, setup.options.difficulty);
    collect_field_tiles(&mut w);
    pick_merchant_starts(&mut w);
    Ok(w)
}

/// `Map_LoadPlanes` (`0x00467770`) whole: the copy, the county count, and the
/// plane-4 dispatch.
///
/// **The dispatch is two tables and the flag bit picks which.** `[V]` and it
/// was written the other way round until `docs/decisions.md` C25:
fn load_planes(slot: &MapSlot<'_>) -> Result<MapWorld, MapError> {
    let tiles = Tiles::from_slot(slot);

    let mut county_count = 0usize;
    let mut player_start = [0u8; 6];
    let mut player_start_count = 0usize;
    let mut routes = MerchantRoutes::none();
    let mut rows = [[0u8; ROUTE_SLOTS]; ROUTES];

    for y in 0..PLANE_DIM {
        for x in 0..PLANE_DIM {
            let i = y * PLANE_DIM + x;
            let county = tiles.county[i];
            if county < 0x11 && county_count < county as usize {
                county_count = county as usize;
            }
            let marker = slot.at(Plane::Marker, x, y);
            if marker == 0 {
                continue;
            }
            if tiles.flags[i] & bit::TOWN != 0 {
                let row = marker as usize - 1;
                if let Some(cells) = rows.get_mut(row) {
                    if let Some(cell) = cells.iter_mut().find(|c| **c == 0) {
                        *cell = county;
                    }
                }
            } else if tiles.flags[i] & bit::SITE != 0 {
                if let Some(cell) = player_start.get_mut(marker as usize) {
                    *cell = county;
                }
                player_start_count += 1;
            }
        }
    }

    if county_count == 0 {
        return Err(MapError::NoCounties);
    }
    if county_count > MAX_COUNTY_ID as usize {
        return Err(MapError::CountyCount(county_count));
    }
    for (row, cells) in rows.iter().enumerate() {
        routes.set_row(row, *cells);
    }
    Ok(MapWorld {
        tiles,
        county_count,
        player_start,
        player_start_count,
        routes,
        merchant_start: [0; ROUTES],
        neighbours: vec![Vec::new(); MAX_COUNTIES],
        town_tile: vec![0; MAX_COUNTIES],
        anchor: vec![(0, 0); MAX_COUNTIES],
        castle_tile: vec![0; MAX_COUNTIES],
        castle: vec![(0, 0); MAX_COUNTIES],
        dwelling_plots: vec![[0; 4]; MAX_COUNTIES],
        industry_site: vec![[0; 4]; MAX_COUNTIES],
        has_resource: vec![[false; 4]; MAX_COUNTIES],
        field_tiles: vec![[0; MAX_FIELDS]; MAX_COUNTIES],
        farm_tile_count: vec![0; MAX_COUNTIES],
    })
}

/// The four 4-adjacent neighbours of a tile, in `FUN_0046C080`'s order —
/// north, east, south, west — with the edge reading as county 0.
pub(crate) fn four_neighbours(x: usize, y: usize) -> [Option<usize>; 4] {
    [
        if y > 0 { Some((y - 1) * PLANE_DIM + x) } else { None },
        if x + 1 < PLANE_DIM { Some(y * PLANE_DIM + x + 1) } else { None },
        if y + 1 < PLANE_DIM { Some((y + 1) * PLANE_DIM + x) } else { None },
        if x > 0 { Some(y * PLANE_DIM + x - 1) } else { None },
    ]
}

