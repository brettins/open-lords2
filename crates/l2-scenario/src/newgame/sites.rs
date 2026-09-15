#![allow(unused_imports)]
use super::*;
use super::build_part::*;
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

/// `FUN_00467CA6` — county `+0x5C`, the zero-terminated adjacency list.
///
/// **Ascending by discovery, not sorted.** The original appends in the scan's
/// own order (`y` outer, `x` inner, then N/E/S/W), stops at sixteen entries,
/// and counts them into `+0x5B`. Reproducing the order matters: the list is
/// walked by index in half a dozen places and two peers must walk it the same
/// way (`docs/netcode.md` D-4).
pub(super) fn adjacency(w: &mut MapWorld) {
    let n = w.county_count;
    for y in 0..PLANE_DIM {
        for x in 0..PLANE_DIM {
            let i = y * PLANE_DIM + x;
            let c = w.tiles.county[i] as usize;
            if c == 0 || c >= 0x12 {
                continue;
            }
            for slot in four_neighbours(x, y) {
                let other = slot.map(|t| w.tiles.county[t]).unwrap_or(0);
                if other == 0 || other as usize >= 0x12 || other as usize == c {
                    continue;
                }
                let list = &mut w.neighbours[c];
                if list.len() >= MAX_NEIGHBOURS || list.contains(&other) {
                    continue;
                }
                list.push(other);
            }
        }
    }
    for c in 0..MAX_COUNTIES {
        if c == 0 || c > n {
            w.neighbours[c].clear();
        } else {
            w.neighbours[c].retain(|&o| o as usize <= n);
        }
    }
}

/// `Counties_PlaceSites` (`0x00468D4F`) — five passes per county, and **the
/// order is load-bearing**.
pub(super) fn place_sites(w: &mut MapWorld) {
    for county in 1..=w.county_count {
        let Some((town, anchor)) = find_town_tile(w, county) else { continue };
        w.town_tile[county] = town;
        w.anchor[county] = anchor;
        w.dwelling_plots[county] = find_dwelling_plots(w, county);
        place_resource_sites(w, county);
        place_blacksmith(w, county);
        if let Some((tile, xy)) = find_castle_tile(w, county) {
            w.castle_tile[county] = tile;
            w.castle[county] = xy;
        }
    }
}

/// `County_FindTownTile` (`0x00467FD1`).
fn find_town_tile(w: &mut MapWorld, county: usize) -> Option<(usize, (u8, u8))> {
    let mut found = 0;
    let mut first = 0usize;
    for y in 0..PLANE_DIM {
        for x in 0..PLANE_DIM {
            let i = y * PLANE_DIM + x;
            if w.tiles.county[i] as usize != county
                || w.tiles.flags[i] & bit::TOWN == 0
                || w.tiles.content[i] != 0
            {
                continue;
            }
            if found == 0 {
                first = i;
                w.tiles.bank[i] |= BANK_OVERLAY;
                w.tiles.stamp_2x2(i, FRAME_VILLAGE_SMALL, BANK_TOWN, 0);
            }
            if found == 2 {
                w.tiles.bank[i] |= BANK_OVERLAY;
            }
            if found == 3 {
                return Some((first, (x as u8, y as u8)));
            }
            found += 1;
        }
    }
    None
}

/// `County_FindDwellingPlots` (`0x00468C41`) — the four `0x10` tiles, their
/// terrain frames saved so razing one can put the ground back.
///
/// **Four is the storage, not a rule.** The original's counter has no bound and
/// the four `i32` slots at county `+0x80` end exactly on `fieldProgress`,
/// fifth plot in one county would corrupt a field's reclamation. All 434
/// counties of the 44 shipped maps have exactly four
/// (`docs/formats/maps-layers.md` §2.3); this one drops the fifth
/// reproducing the overrun, because the overrun is a memory bug and not a rule.
fn find_dwelling_plots(w: &mut MapWorld, county: usize) -> [usize; 4] {
    let mut plots = [0usize; 4];
    let mut n = 0;
    for i in 0..MAP_TILES {
        if w.tiles.flags[i] & bit::PLOT == 0 || w.tiles.county[i] as usize != county {
            continue;
        }
        if n < plots.len() {
            plots[n] = i;
        }
        w.tiles.saved_frame[i] = w.tiles.frame[i];
        w.tiles.content[i] = 0;
        n += 1;
    }
    plots
}

/// `County_PlaceResourceSites` (`0x00468E61`) — **where a county's resources
/// come from is the map, not the county record.**
fn place_resource_sites(w: &mut MapWorld, county: usize) {
    for i in 0..MAP_TILES {
        if w.tiles.county[i] as usize != county || w.tiles.bank_layer(i) != BANK_TOWN {
            continue;
        }
        let frame = w.tiles.frame[i];
        if frame == FRAME_MINE {
            set_site(w, county, i, IND_IRON, TERRAIN_IRON);
        }
        if frame == FRAME_QUARRY && !w.has_resource[county][IND_IRON] {
            set_site(w, county, i, IND_STONE, TERRAIN_STONE);
        }
        if frame == FRAME_FOREST {
            set_site(w, county, i, IND_WOOD, TERRAIN_WOOD);
        }
    }
}

fn set_site(w: &mut MapWorld, county: usize, tile: usize, record: usize, terrain: u8) {
    w.has_resource[county][record] = true;
    w.industry_site[county][record] = tile;
    w.tiles.content[tile] = terrain;
    w.tiles.bank[tile] |= BANK_OVERLAY;
}

/// `FUN_0046C147` — the flag byte of one 4-neighbour, classified.
fn neighbour_class(w: &MapWorld, tile: Option<usize>) -> u8 {
    let Some(t) = tile else { return 0 };
    let raw = w.tiles.flags[t];
    if raw == 0 {
        return 0;
    }
    let layer = w.tiles.bank_layer(t);
    let mut r = raw & !bit::BOUNDARY;
    if r == 0x08 && layer != BANK_MTNS {
        r = 0x10;
    }
    if r == 0x10 && layer != BANK_ROADS {
        r = 0;
    }
    r
}

/// `Dist_Chebyshev` (`0x00404F4C`).
fn chebyshev(a: (u8, u8), b: (u8, u8)) -> i32 {
    let dx = (a.0 as i32 - b.0 as i32).abs();
    let dy = (a.1 as i32 - b.1 as i32).abs();
    dx.max(dy)
}

/// `County_PlaceBlacksmith` (`0x0046902A`) — **the weapons site is derived, not
/// authored, so every county has one.**
fn place_blacksmith(w: &mut MapWorld, county: usize) {
    let anchor = w.anchor[county];
    let mut best_tile = 0usize;
    let mut best = 0x40i32;
    for y in 0..PLANE_DIM {
        for x in 0..PLANE_DIM {
            let i = y * PLANE_DIM + x;
            if w.tiles.county[i] as usize != county || w.tiles.flags[i] != 0 {
                continue;
            }
            let n = four_neighbours(x, y);
            for slot in n {
                let same_county = slot.map(|t| w.tiles.county[t] as usize) == Some(county);
                if !same_county {
                    continue;
                }
                if neighbour_class(w, slot) & bit::FARM == 0 {
                    continue;
                }
                let d = chebyshev(anchor, (x as u8, y as u8));
                if d < best {
                    best = d;
                    best_tile = i;
                }
            }
        }
    }
    if best_tile == 0 || w.tiles.flags[best_tile] != 0 {
        return;
    }
    w.has_resource[county][IND_WEAPONS] = true;
    w.industry_site[county][IND_WEAPONS] = best_tile;
    w.tiles.flags[best_tile] |= bit::SITE;
    w.tiles.content[best_tile] = TERRAIN_WEAPONS;
    w.tiles.bank[best_tile] = (w.tiles.bank[best_tile] & 0xE3) | BANK_TOWN | BANK_OVERLAY;
    w.tiles.frame[best_tile] = 10;
}

/// `County_FindCastleTile` (`0x00468121`) — the bare plot, and **only** the
/// bare plot.
///
/// Finds the county's 2×2 of `0x80` tiles that no resource site took, stamps
/// terrain `0x14` on all four and saves their terrain frames. Raising a castle
/// on the plot is `FUN_0046826C`, which is keyed on the castle's level and its
/// build percentage, and is not done here.
fn find_castle_tile(w: &mut MapWorld, county: usize) -> Option<(usize, (u8, u8))> {
    let mut found = 0;
    let mut first = None;
    for y in 0..PLANE_DIM {
        for x in 0..PLANE_DIM {
            let i = y * PLANE_DIM + x;
            if w.tiles.county[i] as usize != county
                || w.tiles.flags[i] & bit::SITE == 0
                || w.tiles.content[i] != 0
            {
                continue;
            }
            if found == 0 {
                first = Some((i, (x as u8, y as u8)));
                w.tiles.bank[i] |= BANK_OVERLAY;
            }
            w.tiles.saved_frame[i] = w.tiles.frame[i];
            w.tiles.content[i] = TERRAIN_CASTLE_PLOT;
            if found == 3 {
                return first;
            }
            found += 1;
        }
    }
    None
}

