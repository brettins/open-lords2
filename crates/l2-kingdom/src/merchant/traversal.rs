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
/// ```c
/// dest = 0;
/// for (n = 0; dest == 0 && n < 16; n++) {
///     dest = g_merchantRoutes[(u - 1) * 16 + unit[u].routeCursor];
///     if (dest > g_countyCount) dest = 0;
///     if (++unit[u].routeCursor > 15) unit[u].routeCursor = 0;
/// }
/// ```
///
/// Three details that are the whole behaviour and are each easy to lose:
///
/// * **the cursor advances even when the cell was empty**, so the sixteen
///   tries walk the row exactly once and a route of five counties cycles
///   through eleven zeroes to get back to the start;
/// * the wrap is `> 15 -> 0`, so **entry 0 is reached only by wrapping** — a
/// merchant's first destination is entry 1,
///   the county it was spawned in;
/// * a county id above the map's count is treated as **absent**, not as an
///   error.
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
///
/// ```c
/// for (u = 1; u <= 150; u++) if (unit[u].kind == 3) {
///     if (unit[u].hasNoDestination) {
///         ...sixteen tries at the route row...
///         if (dest == 0) continue;
///         if (!County_FindFreeRoadTile(dest)) goto next;   /* skipped entirely */
///         unit[u].destX = g_foundTileX;  unit[u].destY = g_foundTileY;
///         unit[u].destCounty = dest;  unit[u].hasNoDestination = 0;
///     }
///     if (!unit[u].hasNoDestination) { ...two-pass path, moving = 1... }
/// }
/// ```
///
/// Three things the shape of that loop decides, and all three were nearly got
/// wrong here:
///
/// * **A merchant that already has a destination is re-pathed, not skipped.**
///   The `hasNoDestination` test guards only the route lookup; the pathing
///   below it runs for every merchant that has somewhere to go. That is what
/// lets a merchant interrupted mid-leg — blocked by another unit, or simply
/// out of moves — resume the same leg next season instead of stalling for
///   ever. A phase that waits for merchants to stop moving and never restarts
///   them is a phase that settles instantly, which is the bug this whole
///   module exists to fix.
/// * **No free road tile means the merchant is skipped with its flag still
/// set**, so it retries next season.
/// * The route row is the **slot minus one**, not the unit's stored route
///   number. See [`route_row_for`].
///
/// Units are walked in ascending slot order — the original's `for (u = 1; …)`,
/// and the iteration order `docs/netcode.md` §5 requires anyway.
///
/// Returns how many units were set walking.
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
                // The cursor advances whether or not a tile was found: the
                // original moves it inside the search loop, before it knows.
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
            // **`dest_county` is the county on the *route*, not the county the
            // destination tile happens to sit in**, and the two really do
            // differ. `County_FindFreeRoadTile` searches a 7 × 7 box around the
            // county's anchor and never looks at the county plane, so a merchant
            // sent to England's county 12 is sent to a road tile that county 11
            // owns — and on the shipped position that is four of the six.
            //
            // `Unit_OrderMove` derives the field from the tile and is right to,
            // because a human clicking a tile means *that tile*.
            // `Merchant_AdvanceAll` writes the route's county and then never
            // touches it again, so the derived value has to be put back.
            // Restored, so both orders keep sharing one
            // pathfinder.
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
/// ```c
/// for (c = 1; c <= g_countyCount; c++) { c.merchantCount = 0; c.f15C = 0; c.merchantUnit = 0; }
/// for (u = 1; u < 0x97; u++)
///     if (g_units[u].kind == 3) {
///         g_counties[g_units[u].county].merchantCount++;
///         g_counties[g_units[u].county].merchantVisits++;
///         g_counties[g_units[u].county].merchantUnit = (byte)u;
///     }
/// ```
///
/// **Three details are the rule **, and all three are
/// visible in the C above:
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
    // Ascending slot order — the original's `for (u = 1;...)`, and the
    // iteration order `docs/netcode.md` §5 requires anyway. `Units::iter`
    // yields slots in index order.
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

