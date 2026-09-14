#![allow(unused_imports)]
use super::*;
use super::traversal::*;
use super::tests_part::*;
use crate::county::{County, MAX_COUNTIES};
use crate::map::{flags, CampaignMap};
use crate::movement;
use crate::unit::{UnitKind, Units};

/// `County_FindFreeRoadTile` (`0x00428007`) — a free **road** tile within
/// three of the county's anchor.
///
/// The box around the anchor is grown `r = 1, 2, 3` and only road tiles
/// qualify,: `plane4.md` §2.2's
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

