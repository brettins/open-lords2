//! The six merchant trade routes, and how a merchant picks where to go next —
//! `docs/formats/plane4.md` §2.1 … §2.3.
//!
//! # Why this module exists at all
//!
//! `crate::unit` models a merchant's *record* and `crate::movement` walks it,
//! and between them there was still no answer to the only question a merchant
//! ever asks: **where next?** Phase 6 of the turn machine waits for every
//! type-3 unit to stop moving, and a merchant with no destination never starts,
//! so the phase settled instantly and the merchants stood still for ever. The
//! missing piece is a table — `g_merchantRoutes` (`0x00567970`), six rows of
//! sixteen county ids — and the cursor walk that reads it.
//!
//! # The table is in the save, and the England fixture proves the whole model
//!
//! `g_merchantRoutes` falls inside a saved block, so it can be read straight
//! out of `england-turn1.sav` rather than derived. It holds:
//!
//! ```text
//! route 0  14  4  7  8  2
//! route 1   5 12  6  3  8  2  1
//! route 2  14 11 13  5 10  9  7  3  1
//! route 3  11 12  6 10  4  9  3  1
//! route 4  14 11  5 13 12 10  2
//! route 5  13  6  4  9  7  8
//! ```
//!
//! and the same save's unit array holds exactly six units, slots **1 … 6**, all
//! type 3, all owner 6, standing in counties **14, 5, 13, 11, 12, 4** with
//! `needsDestination = 1` and `routeCursor = 1`. Three things that could each
//! have failed:
//!
//! * the start counties are the **first entry of each row**, which is what
//!   makes `plane4.md`'s claim that *"the route row is `unitIndex − 1`, not the
//!   stored route number"* checkable — and it holds, six for six. That
//!   document marked its England start-county list `[I]`, predicted from a
//!   simulation of `Merchant_PickStartCounties`; the file says **14, 5, 13, 11,
//!   12, 4** and the prediction was `14, 5, 13, 11, 12, 4`. **`[V]`** now.
//! * `nameIndex` (`+0x14F`) is `0, 1, 2, 3, 4, 5` — the route index, one per
//!   slot, as §3 says;
//! * **`moveAllowance` (`+0x154`) is stored as 0 in all six.** It is written by
//!   the tick handler every frame and never persisted, which is the direct
//!   evidence for `docs/armies.md` §2.1a's "tick-maintained" reading. A unit
//!   loaded from a save and never ticked has no allowance at all.
//!
//! # What is deliberately *not* here
//!
//! `Merchant_SpawnAll` and `Merchant_PickStartCounties` (`plane4.md` §2.1,
//! §2.2) run once, at new-game, off the map file's plane 4. This crate has no
//! loader and never learns what `L2_maps.dat` is, so the routes arrive as data
//! — [`MerchantRoutes`] is filled by whoever built the scenario. The odd dedup
//! walk and its two known bugs stay on the loader's side of that seam.

use crate::county::{County, MAX_COUNTIES};
use crate::map::{flags, CampaignMap};
use crate::movement;
use crate::unit::{UnitKind, Units};

/// Six routes, because plane 4 on a castle tile is a route number **1 … 6**.
pub const ROUTES: usize = 6;

/// Sixteen counties a route, the width of one `g_merchantRoutes` row.
///
/// `Merchant_RouteAppend` appends to the first free cell of a row and gives up
/// silently when the row is full; **0 of the 44 shipped maps overflow it**
/// (`plane4.md` §1), so the cap is never reached in practice.
pub const ROUTE_SLOTS: usize = 16;

/// The radius the free-tile search grows to before it gives up.
/// `County_FindFreeRoadTile` (`0x00428007`) tries the `(2r+1)²` box around the
/// county's anchor for **r = 1, 2, 3**.
pub const FREE_TILE_RADIUS: u8 = 3;

/// `g_merchantRoutes` (`0x00567970`) — six rows of sixteen county ids, `0`
/// meaning "no more".
///
/// A fixed array rather than a `Vec` of `Vec`s on purpose: `docs/netcode.md` §5
/// wants iteration whose order cannot depend on anything but an index, and this
/// is walked by index in two places.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MerchantRoutes {
    rows: [[u8; ROUTE_SLOTS]; ROUTES],
}

impl Default for MerchantRoutes {
    fn default() -> Self {
        MerchantRoutes::none()
    }
}

impl MerchantRoutes {
    /// No routes at all — what a map with no merchants on it looks like, and
    /// what a hand-built kingdom starts with.
    pub fn none() -> MerchantRoutes {
        MerchantRoutes { rows: [[0; ROUTE_SLOTS]; ROUTES] }
    }

    /// The whole row, including its trailing zeroes.
    pub fn row(&self, route: usize) -> &[u8; ROUTE_SLOTS] {
        &self.rows[route]
    }

    pub fn set_row(&mut self, route: usize, row: [u8; ROUTE_SLOTS]) {
        self.rows[route] = row;
    }

    /// Fill a row from a list of counties, in order, the way
    /// `Merchant_RouteAppend` builds it.
    pub fn set_route(&mut self, route: usize, counties: &[u8]) {
        let mut row = [0u8; ROUTE_SLOTS];
        for (slot, &c) in row.iter_mut().zip(counties.iter()) {
            *slot = c;
        }
        self.rows[route] = row;
    }

    /// How many counties a route names before its first zero.
    pub fn len(&self, route: usize) -> usize {
        self.rows[route].iter().position(|&c| c == 0).unwrap_or(ROUTE_SLOTS)
    }

    pub fn is_empty(&self, route: usize) -> bool {
        self.len(route) == 0
    }

    /// Whether any route names anything. A kingdom whose routes are all empty
    /// has no merchants to advance, and phase 6 is then a formality.
    pub fn any(&self) -> bool {
        self.rows.iter().any(|r| r[0] != 0)
    }
}

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
///   merchant's first destination is entry 1, which is why it is usually not
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

/// `County_FindFreeRoadTile` (`0x00428007`) — a free **road** tile within
/// three of the county's anchor.
///
/// The box around the anchor is grown `r = 1, 2, 3` and only road tiles
/// qualify, which is why merchants are always found on roads: `plane4.md` §2.2's
/// live dump of England found all six start tiles carrying plane 0 `0x01`
/// exactly. A tile is free only if no unit stands on it.
///
/// **This has no fallback, and that is the point.** `Merchant_SpawnAll` pairs
/// it with [`find_free_open_tile`] at new-game; `Merchant_AdvanceAll` does
/// **not** — a county with no free road tile near its anchor is skipped
/// outright and the merchant keeps `needs_destination` set to try again next
/// season. A merchant therefore cannot be routed onto open ground mid-circuit
/// even though it could have been spawned there.
///
/// **`[I]` on the scan order inside a box.** The original's is not recorded and
/// nothing observable depends on it *except* which of several equally free
/// tiles is picked; this walks rows north to south and columns west to east,
/// the order `Map_LoadPlanes` walks the map in.
pub fn find_free_road_tile(map: &CampaignMap, units: &Units, anchor: (u8, u8)) -> Option<(u8, u8)> {
    (1..=FREE_TILE_RADIUS).find_map(|r| scan_box(map, units, anchor, r, |f| f & flags::ROAD != 0))
}

/// `County_FindFreeOpenTile` (`0x00428078`) — the spawn-time fallback.
///
/// Accepts plain grass: `flags & 0xFD == 0`, i.e. every bit clear except the
/// county-boundary bit, which is the one bit that does not describe the ground.
/// Only `Merchant_SpawnAll` uses it; see [`find_free_road_tile`].
pub fn find_free_open_tile(map: &CampaignMap, units: &Units, anchor: (u8, u8)) -> Option<(u8, u8)> {
    (1..=FREE_TILE_RADIUS)
        .find_map(|r| scan_box(map, units, anchor, r, |f| f & !flags::BOUNDARY == 0))
}

fn scan_box(
    map: &CampaignMap,
    units: &Units,
    (ax, ay): (u8, u8),
    radius: u8,
    accept: impl Fn(u8) -> bool,
) -> Option<(u8, u8)> {
    let lo = |v: u8| v.saturating_sub(radius);
    let hi = |v: u8| (v as usize + radius as usize).min(crate::map::MAP_DIM - 1) as u8;
    for y in lo(ay)..=hi(ay) {
        for x in lo(ax)..=hi(ax) {
            if units.at(x, y).is_none() && accept(map.flags_at(x, y)) {
                return Some((x, y));
            }
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
///   lets a merchant interrupted mid-leg — blocked by another unit, or simply
///   out of moves — resume the same leg next season instead of stalling for
///   ever. A phase that waits for merchants to stop moving and never restarts
///   them is a phase that settles instantly, which is the bug this whole
///   module exists to fix.
/// * **No free road tile means the merchant is skipped with its flag still
///   set**, so it retries next season rather than losing its place.
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
            // Restored rather than avoided, so both orders keep sharing one
            // pathfinder.
            if let Some(u) = units.get_mut(id) {
                u.dest_county = dest_county;
            }
        }
    }
    started
}

/// The `g_merchantRoutes` row a unit reads: **its slot number minus one**.
///
/// Not the unit's stored route index. See [`advance_all`].
pub fn route_row_for(id: usize) -> Option<usize> {
    id.checked_sub(1).filter(|&r| r < ROUTES)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::county::County;
    use crate::map::MAP_TILES;
    use crate::unit::Unit;

    /// England's route 0, straight out of `england-turn1.sav`.
    const ENGLAND_ROUTE_0: [u8; 5] = [14, 4, 7, 8, 2];

    fn routes() -> MerchantRoutes {
        let mut r = MerchantRoutes::none();
        r.set_route(0, &ENGLAND_ROUTE_0);
        r
    }

    /// The cursor starts at 1, so the first destination is the *second* county
    /// on the list — and the merchant does not immediately re-visit the county
    /// it was spawned in. `plane4.md` §2.3.
    #[test]
    fn the_first_destination_is_the_second_entry() {
        let mut cursor = 1;
        assert_eq!(next_destination(&routes(), 0, &mut cursor, 14), Some(4));
        assert_eq!(cursor, 2);
    }

    /// The whole circuit, twice round, including the eleven empty cells the
    /// cursor walks through to get back to entry 0.
    #[test]
    fn the_route_cycles_and_entry_zero_is_only_reached_by_wrapping() {
        let r = routes();
        let mut cursor = 1;
        let walked: Vec<u8> = (0..10)
            .filter_map(|_| next_destination(&r, 0, &mut cursor, 14))
            .collect();
        assert_eq!(walked, vec![4, 7, 8, 2, 14, 4, 7, 8, 2, 14]);
    }

    /// A county id above the map's count is absent, not an error — and a row
    /// of nothing but such ids yields nothing rather than looping for ever.
    #[test]
    fn a_county_above_the_map_count_is_skipped() {
        let mut r = MerchantRoutes::none();
        r.set_route(0, &[9, 3]);
        let mut cursor = 1;
        assert_eq!(next_destination(&r, 0, &mut cursor, 4), Some(3));

        r.set_route(1, &[9, 11]);
        let mut cursor = 0;
        assert_eq!(next_destination(&r, 1, &mut cursor, 4), None);
    }

    #[test]
    fn an_empty_row_yields_nothing() {
        let mut cursor = 1;
        assert_eq!(next_destination(&MerchantRoutes::none(), 3, &mut cursor, 14), None);
    }

    /// The road search grows its box to radius 3, so a road three tiles away is
    /// found — and **grass next door is not a candidate at all**, because
    /// `Merchant_AdvanceAll` never calls the open-ground fallback.
    #[test]
    fn the_road_search_reaches_three_tiles_and_ignores_grass() {
        let mut map = CampaignMap::empty();
        // Everything is grass except one road at distance 3.
        map.set_flags(20, 17, flags::ROAD);
        let units = Units::new();
        assert_eq!(find_free_road_tile(&map, &units, (20, 20)), Some((20, 17)));

        // With no road at all the road search fails outright; only the spawn's
        // fallback would have taken the grass.
        let plain = CampaignMap::empty();
        assert_eq!(find_free_road_tile(&plain, &units, (20, 20)), None);
        assert!(find_free_open_tile(&plain, &units, (20, 20)).is_some());
    }

    /// A tile with a unit on it is not free, whatever it is made of.
    #[test]
    fn an_occupied_road_is_not_free() {
        let mut map = CampaignMap::empty();
        map.set_flags(20, 19, flags::ROAD);
        map.set_flags(21, 19, flags::ROAD);
        let mut units = Units::new();
        units.put(1, Unit::new(UnitKind::Merchant, 6, 20, 19));
        assert_eq!(find_free_road_tile(&map, &units, (20, 20)), Some((21, 19)));
    }

    /// Rough ground and sea are not open ground: the fallback's test is
    /// `flags & !BOUNDARY == 0`, so only the boundary bit is forgiven.
    #[test]
    fn only_the_boundary_bit_is_forgiven_by_the_open_ground_fallback() {
        let mut map = CampaignMap::empty();
        for i in 0..MAP_TILES {
            map.flags[i] = flags::ROUGH;
        }
        map.set_flags(21, 21, flags::BOUNDARY);
        let units = Units::new();
        assert_eq!(find_free_open_tile(&map, &units, (20, 20)), Some((21, 21)));
    }

    /// The row a merchant reads is its slot minus one, and slots above 6 have
    /// no row at all — a seventh merchant would walk nothing.
    #[test]
    fn the_row_is_the_slot_minus_one() {
        assert_eq!(route_row_for(1), Some(0));
        assert_eq!(route_row_for(6), Some(5));
        assert_eq!(route_row_for(7), None);
        assert_eq!(route_row_for(0), None);
    }

    /// Two counties, a road down the middle of each, and a merchant standing on
    /// the western one.
    fn two_county_map() -> (CampaignMap, [County; MAX_COUNTIES]) {
        let mut map = CampaignMap::empty();
        for i in 0..MAP_TILES {
            map.county[i] = if i % crate::map::MAP_DIM < 32 { 1 } else { 2 };
        }
        for x in 0..64u8 {
            map.set_flags(x, 10, flags::ROAD);
        }
        let mut counties: [County; MAX_COUNTIES] = core::array::from_fn(|_| County::new());
        counties[1].anchor_x = 10;
        counties[1].anchor_y = 10;
        counties[2].anchor_x = 50;
        counties[2].anchor_y = 10;
        (map, counties)
    }

    /// End to end: a merchant with no destination is given one, and it is a
    /// road tile inside the next county on its route.
    #[test]
    fn an_idle_merchant_is_sent_to_the_next_county_on_its_route() {
        let (map, counties) = two_county_map();
        let mut units = Units::new();
        let mut m = Unit::new(UnitKind::Merchant, 6, 10, 10);
        m.needs_destination = true;
        m.year_formed = 1;
        units.put(1, m);

        let mut r = MerchantRoutes::none();
        r.set_route(0, &[1, 2]);

        assert_eq!(advance_all(&r, &map, &counties, &mut units, 2), 1);
        let u = units.get(1).unwrap();
        assert_eq!(u.dest_county, 2, "the second entry on the route");
        assert!(u.moving, "and it has been set walking");
        assert!(!u.path.is_empty());
        let dest = u.dest.unwrap();
        assert_eq!(map.county_at(dest.0, dest.1), 2);
        assert!(map.has(dest.0, dest.1, flags::ROAD), "and it is a road tile");
    }

    /// **A merchant part-way through a leg is re-pathed, not left alone.** The
    /// `needs_destination` test guards only the route lookup; every merchant
    /// with somewhere to go is set walking again. Without this a merchant that
    /// stopped for any reason would never move again, and phase 6 would settle
    /// on the first tick for ever.
    #[test]
    fn a_merchant_mid_leg_keeps_its_destination_and_is_set_walking_again() {
        let (map, counties) = two_county_map();
        let mut units = Units::new();
        let mut m = Unit::new(UnitKind::Merchant, 6, 30, 10);
        m.needs_destination = false;
        m.dest = Some((50, 10));
        m.dest_county = 2;
        m.year_formed = 9;
        m.moving = false;
        units.put(1, m);

        let mut r = MerchantRoutes::none();
        r.set_route(0, &[1, 2]);

        assert_eq!(advance_all(&r, &map, &counties, &mut units, 2), 1);
        let u = units.get(1).unwrap();
        assert_eq!(u.dest_county, 2, "the same leg");
        assert_eq!(u.year_formed, 9, "and the route cursor did not move");
        assert!(u.moving);
    }

    /// A county with no free road tile near its anchor is skipped and the
    /// merchant keeps its flag, so it tries again next season rather than
    /// losing its place on the route.
    #[test]
    fn a_county_with_no_free_road_tile_is_skipped_and_retried() {
        let mut map = CampaignMap::empty();
        for i in 0..MAP_TILES {
            map.county[i] = 1;
        }
        let mut counties: [County; MAX_COUNTIES] = core::array::from_fn(|_| County::new());
        counties[1].anchor_x = 10;
        counties[1].anchor_y = 10;
        counties[2].anchor_x = 50;
        counties[2].anchor_y = 10;

        let mut units = Units::new();
        let mut m = Unit::new(UnitKind::Merchant, 6, 10, 10);
        m.needs_destination = true;
        m.year_formed = 1;
        units.put(1, m);

        let mut r = MerchantRoutes::none();
        r.set_route(0, &[1, 2]);

        assert_eq!(advance_all(&r, &map, &counties, &mut units, 2), 0);
        let u = units.get(1).unwrap();
        assert!(u.needs_destination, "still asking");
        assert!(!u.moving);
    }
}
