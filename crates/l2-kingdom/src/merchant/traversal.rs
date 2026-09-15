#![allow(unused_imports)]
use super::*;
use super::placement::*;
use super::tests_part::*;
use crate::county::{County, MAX_COUNTIES};
use crate::map::{flags, CampaignMap};
use crate::movement;
use crate::unit::{UnitKind, Units};

/// The next county on a merchant's route, advancing its cursor — the inner
/// loop of `Merchant_AdvanceAll` (`0x004280E9`), `plane4.md` §2.3:
///
/// Returns `None` when sixteen tries found nothing — an empty row, which is
/// what a map with fewer than six routes has. `[V]` on the loop.
pub fn next_destination(routes: &MerchantRoutes, route_row: usize, cursor: &mut i32, county_count: usize) -> Option<u8> {
    let row = routes.row(route_row);
    for _ in 0..ROUTE_SLOTS {
        let at = (*cursor).clamp(0, ROUTE_SLOTS as i32 - 1) as usize;
        let mut dest = row[at];
        if dest as usize > county_count {
            dest = 0;
        }
        *cursor += 1;
        if *cursor > ROUTE_SLOTS as i32 - 1 {
            *cursor = 0;
        }
        if dest != 0 {
            return Some(dest);
        }
    }
    None
}

/// `Merchant_AdvanceAll` (`0x004280E9`) — **the first step of turn phase 6**.
pub fn advance_all(
    routes: &MerchantRoutes,
    map: &CampaignMap,
    counties: &[County; MAX_COUNTIES],
    units: &mut Units,
    county_count: usize,
) -> usize {
    let mut started = 0;
    let ids: Vec<usize> = units
        .iter()
        .filter(|(_, u)| u.kind == UnitKind::Merchant)
        .map(|(id, _)| id)
        .collect();
    for id in ids {
        if units.get(id).is_some_and(|u| u.needs_destination) {
            let Some(row) = route_row_for(id) else { continue };
            let mut cursor = units.get(id).map_or(0, |u| u.year_formed);
            let dest_county = next_destination(routes, row, &mut cursor, county_count);
            if let Some(u) = units.get_mut(id) {
                u.year_formed = cursor;
            }
            let Some(dest_county) = dest_county else { continue };
            let Some(county) = counties.get(dest_county as usize) else { continue };
            let anchor = (county.anchor_x, county.anchor_y);
            let Some(tile) = find_free_road_tile(map, units, anchor) else { continue };
            let Some(u) = units.get_mut(id) else { continue };
            u.dest = Some(tile);
            u.dest_county = dest_county;
            u.needs_destination = false;
        }
        if units.get(id).is_some_and(|u| !u.needs_destination) {
            let Some((dest, dest_county)) =
                units.get(id).and_then(|u| u.dest.map(|d| (d, u.dest_county)))
            else {
                continue;
            };
            if movement::order_move_by_road(map, units, id, dest).is_some() {
                started += 1;
            }
            if let Some(u) = units.get_mut(id) {
                u.dest_county = dest_county;
            }
        }
    }
    started
}

/// `County_RecountMerchants` (`0x00451061`) — **season pass 22**, and the pass
/// that decides which counties have a stall to trade at.
///
/// * the visit counter is **not** cleared, which is what makes it a lifetime
///   total — `docs/symbols.md` records it rising by exactly that turn's count
///   across three successive turns of one saved game;
/// * the second loop is over **every unit slot**, so it credits a merchant to
///   whatever county its `+0x10` says, including county 0 — the clear loop only
/// walks `1..= g_countyCount`, so a merchant standing outside the map's
///   county range writes into a record the pass never resets. Reproduced by
/// bounds-checking the write instead of the clear
///   behaviour for every county that exists;
/// * two merchants in one county leave the **higher** slot in `merchantUnit`,
///   because the store overwrites.
///
/// The county it counts is the unit's own `+0x10`, which `Army_Tick` keeps
/// equal to the tile's county byte — **not** `dest_county`, which is where the
/// route said to go. See [`advance_all`] for why those two differ.
///
/// `+0x15C` is the third field the clear loop zeroes and this crate does not
/// model it; it is cleared here by nothing,
/// to be discovered.
pub fn recount_all(counties: &mut [County; MAX_COUNTIES], county_count: usize, units: &Units) {
    for county in counties.iter_mut().take(county_count + 1).skip(1) {
        county.merchant_count = 0;
        county.merchant_unit = 0;
    }
    for (id, unit) in units.iter() {
        if unit.kind != UnitKind::Merchant {
            continue;
        }
        let Some(county) = counties.get_mut(unit.county as usize) else { continue };
        county.merchant_count += 1;
        county.merchant_visits += 1;
        county.merchant_unit = id as u8;
    }
}

