#![allow(unused_imports)]
use super::*;
use super::ui::*;
use super::tests::*;
use crate::county::{County, MAX_FIELDS};
use crate::map::CampaignMap;

pub const SWEEP_TRIES: usize = 20;

/// The round-robin step `FUN_0046958F`, `FUN_0046965A` and `FUN_00469A9C`
/// share: advance the cursor, wrap it at the county's used-slot count
/// (`+0x205`, [`County::field_slots_used`]), and stop on the first tile `want`
/// accepts.
///
/// The cursor is advanced **before** the wrap test and before the tile is read,
/// and it is left where the search stopped — including when the search failed,
/// which is the original's (it writes the byte on every try). `[V]`
fn sweep(
    county: &County,
    map: &CampaignMap,
    cursor: &mut u8,
    want: impl Fn(u8) -> bool,
) -> Option<usize> {
    let bound = county.field_slots_used() as u8;
    for _ in 0..SWEEP_TRIES {
        *cursor = cursor.wrapping_add(1);
        if bound <= *cursor {
            *cursor = 0;
        }
        if let Some(tile) = county.field_tile(*cursor as usize) {
            if want(map.terrain[tile]) {
                return Some(tile);
            }
        }
    }
    None
}

/// `FUN_0046958F` — turn one **fallow** field into pasture, cursor `+0x15A`.
pub fn fallow_to_pasture(county: &mut County, map: &mut CampaignMap) -> Option<usize> {
    let mut cursor = county.pasture_cursor;
    let tile = sweep(county, map, &mut cursor, |t| t == terrain::FALLOW);
    county.pasture_cursor = cursor;
    if let Some(tile) = tile {
        paint_tile(map, tile, terrain::PASTURE);
    }
    tile
}

/// `FUN_0046965A` — turn one **grain** field into pasture and dock the standing
/// crop for it. Same cursor, `+0x15A`.
pub fn grain_to_pasture(county: &mut County, map: &mut CampaignMap) -> Option<usize> {
    let mut cursor = county.pasture_cursor;
    let tile = sweep(county, map, &mut cursor, |t| {
        t > terrain::FALLOW && t < terrain::PASTURE_FIRST
    });
    county.pasture_cursor = cursor;
    let tile = tile?;
    paint_tile(map, tile, terrain::PASTURE);
    let loss = crate::math::pct(county.crop[1], crate::math::pct_of(1, county.fields_grain));
    county.crop[1] -= loss;
    // The original also does `+0x234 += loss`. **Nothing reads `+0x234`**: the
    // whole decompilation holds three writers (this, `County_DestroyField` and
    // a clear in `Season_Advance`) and no reader
    // add it to. `[V]`
    if county.fields_grain <= county.fields_grain_standing {
        county.fields_grain_standing -= 1;
    }
    county.fields_grain -= 1;
    Some(tile)
}

/// `County_EnsurePasture` (`0x0046921D`) — buying cattle into a county with no
/// pasture converts a field into one.
///
/// `Merchant_Trade` (`0x004284CE`) calls it on `good == 2 && qty >= 0`, and
/// `Transport_Deliver` on a cattle delivery.
pub fn ensure_pasture(county: &mut County, map: &mut CampaignMap) {
    recount(county, map);
    if county.fields_cattle != 0 {
        return;
    }
    if county.fields_fallow != 0 {
        fallow_to_pasture(county, map);
    } else if county.fields_grain != 0 {
        grain_to_pasture(county, map);
    }
    recount(county, map);
}

/// `FUN_00469A9C` — paint `blight` over the next field in the round-robin at
/// `+0x15B`, **whatever that field is**: this sweep tests only that the slot
/// holds a tile.
pub fn blight_one_field(
    county: &mut County,
    map: &mut CampaignMap,
    blight: u8,
) -> Option<usize> {
    let mut cursor = county.blight_cursor;
    let tile = sweep(county, map, &mut cursor, |_| true);
    county.blight_cursor = cursor;
    if let Some(tile) = tile {
        paint_tile(map, tile, blight);
    }
    tile
}

/// `FUN_0046942C` — last season's ruined fields go back to waste.
pub fn clear_blight(county: &County, map: &mut CampaignMap) {
    for slot in 0..MAX_FIELDS {
        let Some(tile) = county.field_tile(slot) else { continue };
        if map.terrain[tile] == terrain::FLOODED || map.terrain[tile] == terrain::PARCHED {
            paint_tile(map, tile, terrain::WASTE);
        }
    }
}

/// `Field_PaintTile` (`FUN_0046D7F4`) — the terrain byte, and nothing else.
pub fn paint_tile(map: &mut CampaignMap, tile: usize, terrain: u8) {
    map.terrain[tile] = terrain;
}

/// `FUN_00469D21(county, terrain, 0, first, last)` — **repaint every one of a
/// county's field tiles whose terrain is in `first ..= last`.**
///
/// **The `param_4 == 2` clause is not reproduced and this is why.** The
/// original carries an extra arm — *if the range starts at 2 and the county's
/// `+0x1A7` is set and this is not the first tile matched
/// the requested terrain* — which is `Grain_SeasonTick`'s business, not the
/// herd's, and `Herd_UpdateCrowding` passes `first = 0x13`. Naming it here
///
/// half needs to bring it. **[V]**
pub fn repaint_range(county: &County, map: &mut CampaignMap, terrain: u8, range: (u8, u8)) {
    for slot in 0..MAX_FIELDS {
        let Some(tile) = county.field_tile(slot) else { continue };
        if (range.0..=range.1).contains(&map.terrain[tile]) {
            map.terrain[tile] = terrain;
        }
    }
}

/// This is deliberately *not* [`classify`]
/// worth stating. `FUN_004697CD` and `FUN_0046988D` both test grain as
/// `1 < t && t < 0x0F` — the whole crop range, which agrees with the recount —
/// but pasture as `0x12 < t && t < 0x17`, which is only `0x13 … 0x16`. **A
/// pasture at terrain `0x0F … 0x12` is counted by the recount and invisible to
/// the AI's brush.** `[D]`
/// both functions,
fn ai_brush_matches(kind: FieldType, terrain: u8) -> bool {
    match kind {
        FieldType::Grain => (terrain::GRAIN..=terrain::GRAIN_LAST).contains(&terrain),
        FieldType::Pasture => (terrain::PASTURE..=terrain::PASTURE_LAST).contains(&terrain),
        _ => false,
    }
}

/// `FUN_0044C6C4` — order `want` of the county's fields put under
/// reclamation. Returns how many wasteland tiles were started.
///
/// So **a field already under reclamation consumes a place in the quota**
/// without anything happening. A county with one field already being reclaimed
/// that is told to add one adds nothing at all, and only the county told to add
/// two gets a second going. `[V]` — the `goto` in the decompilation skips the
/// decrement for an in-use field and falls through to it for a reclaiming one,
/// which is the whole of the difference.
pub fn order_reclamation(county: &County, map: &mut CampaignMap, mut want: i32) -> i32 {
    let mut started = 0;
    for slot in 0..MAX_FIELDS {
        let Some(tile) = county.field_tile(slot) else { continue };
        let here = map.terrain[tile];
        if here < terrain::RECLAIM_FIRST {
            if here != terrain::WASTE {
                continue;
            }
            map.terrain[tile] = terrain::RECLAIM_FIRST;
            started += 1;
        }
        want -= 1;
        if want < 1 {
            break;
        }
    }
    started
}

/// `FUN_004697CD` — turn every field of one type back to fallow.
pub fn clear_type(county: &County, map: &mut CampaignMap, kind: FieldType) {
    for slot in 0..MAX_FIELDS {
        let Some(tile) = county.field_tile(slot) else { continue };
        if ai_brush_matches(kind, map.terrain[tile]) {
            map.terrain[tile] = terrain::FALLOW;
        }
    }
}

/// `FUN_0046988D` — make exactly `count` of the county's fields this type, and
/// return everything else of that type to fallow.
///
/// This is how an AI lord farms (`FUN_004A3C67` and its siblings call it with
/// grain and half the county's fields, every Winter). The walk is one pass in
/// slot order: a **fallow** field is converted while the quota lasts, and a
/// field already of this type either consumes a quota place or is turned back
/// to fallow. Returns how many places went unused.
pub fn set_count(
    county: &County,
    map: &mut CampaignMap,
    kind: FieldType,
    mut count: i32,
) -> i32 {
    for slot in 0..MAX_FIELDS {
        let Some(tile) = county.field_tile(slot) else { continue };
        let here = map.terrain[tile];
        if here == terrain::FALLOW && count > 0 {
            map.terrain[tile] = kind.brush();
            count -= 1;
        } else if ai_brush_matches(kind, here) {
            if count < 1 {
                map.terrain[tile] = terrain::FALLOW;
            } else {
                count -= 1;
            }
        }
    }
    count.max(0)
}

