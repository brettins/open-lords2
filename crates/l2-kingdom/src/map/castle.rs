#![allow(unused_imports)]
use super::*;
use super::site::*;
use super::campaign::*;
use super::cost::*;
use crate::tables::{MOVE_COST_BLOCKED, MOVE_COST_IMPASSABLE};

// ---------------------------------------------------------------------------
// The castle plot
// ---------------------------------------------------------------------------

/// **Where a county's castle stands** — the 2×2 block on plane-0 bit `0x80`
/// whose terrain is [`terrain::CASTLE_PLOT`] or a castle above it.
///
/// `County_FindCastleTile` (`0x00468121`) scans for it once at load and stores
/// the corner in county `+0x74`/`+0x75`; we scan, the way every other
/// settlement lookup in this workspace does, because the alternative is a
/// cached tile index that a loader has to remember to fill and that nothing
/// notices is zero. Tiles come back in index order, so the first is the
/// original's `+0x74`/`+0x75` corner.
pub fn castle_tiles(map: &CampaignMap, county: u8) -> Vec<usize> {
    (0..MAP_TILES)
        .filter(|&i| {
            map.county[i] == county
                && map.flags[i] & flags::SETTLEMENT != 0
                && terrain::castle_type(map.terrain[i]).is_some()
        })
        .collect()
}

/// The corner of [`castle_tiles`] — county `+0x74`/`+0x75`, the tile an army
/// garrisoning the castle is teleported onto.
pub fn castle_tile(map: &CampaignMap, county: u8) -> Option<usize> {
    castle_tiles(map, county).first().copied()
}

/// The plane-1 byte [`castle_stamp`] writes. Bank index
/// `(0x10 & 0x1C) >> 2 == 4`, which is `Castle1a.pl8` / `Castle2a.pl8`.
///
/// Bit `0` is set by `Map_StampBlock` on every tile it touches and nothing we
/// have read consumes it; it is carried.
pub const CASTLE_BANK_BYTE: u8 = 0x11;

/// **What a castle looks like** — `Castle_StampTile` (`0x0046826C`), which is
/// the reason a castle is missing from our campaign map entirely.
///
/// ```c
/// if (castleDegraded == 0) frame = level*4 + 0x50;   /* finished */
/// else if (percent < 0x32) frame = level*4 + 0x28;   /* scaffolding */
/// else                     frame = level*4 + 0x3C;   /* half-built */
/// content = 0x15 + level;
/// Map_StampBlock(frame, 2, county.castleTile, 0x10, content);
/// ```
///
/// Three appearances per level, twenty frames apart, and **the map file does
/// not hold any of them**: unlike the mine,
/// `L2_maps.dat` stores as real artwork that `County_PlaceResourceSites`
/// flags — the castle plot is plain ground in the base bank and every castle on
/// the screen is stamped in at run time. It is re-stamped **every season** by
/// `Castle_BuildTick`, so it lives beside the build rules
/// in a load-time pass.
///
/// `frames` are in the block's own order — north-west, north-east, south-west,
/// south-east — with the quadrant offsets `[0, 2, 1, 3]` already added.
/// `Map_StampBlock`'s size-2 quadrant table was read out of `Lords2.exe` at
/// `0x004D80E0`. `[V]`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CastleStamp {
    pub bank: u8,
    pub frames: [u8; 4],
    pub terrain: u8,
}

/// The quadrant offsets `Map_StampBlock` adds for a 2×2 stamp — `0x004D80E0`,
/// read from the binary. North-west, north-east, south-west, south-east.
pub const BLOCK_QUADRANTS_2: [u8; 4] = [0, 2, 1, 3];

/// A finished castle's base frame, by level: `level * 4 + 0x50`.
pub const CASTLE_FRAME_BUILT: u8 = 0x50;
/// Under way and less than half done: `level * 4 + 0x28`.
pub const CASTLE_FRAME_SCAFFOLD: u8 = 0x28;
/// Under way and half done or more: `level * 4 + 0x3C`.
pub const CASTLE_FRAME_HALF: u8 = 0x3C;

/// The percentage at which the scaffolding becomes a half-built castle.
pub const CASTLE_HALF_PERCENT: u8 = 0x32;

/// **The simulation's half of `Castle_StampTile`** — write the castle's
/// `content` byte onto its 2×2 block.
///
/// The frame and the bank are the renderer's
/// `Unit_TryEnterTile` masks the settlement bit off at
/// [`terrain::CASTLE_PLOT`] and leaves it set above it, so this write is what
/// turns a tile an army walks over into a castle it has to garrison or besiege.
/// Ordering a castle stamps the new level at once — the scaffolding is already
/// an obstacle — which is the original's ordering in `Castle_Order`.
///
/// Returns the tiles written
/// plot at all.
pub fn stamp_castle_terrain(map: &mut CampaignMap, county: u8, castle_type: u8) -> usize {
    let Some(t) = terrain::CASTLE_PLOT.checked_add(castle_type.min(5)) else { return 0 };
    let tiles = castle_tiles(map, county);
    for i in &tiles {
        map.terrain[*i] = t;
    }
    tiles.len()
}

/// See [`CastleStamp`]. `castle_type` is 1..=5; type 0 has nothing to draw and
/// answers `None`, which leaves the bare plot the map file already holds.
pub fn castle_stamp(castle_type: u8, castle_degraded: u8, castle_percent: u8) -> Option<CastleStamp> {
    if castle_type == 0 || castle_type > 5 {
        return None;
    }
    let level = castle_type - 1;
    let base = if castle_degraded == 0 {
        CASTLE_FRAME_BUILT
    } else if castle_percent < CASTLE_HALF_PERCENT {
        CASTLE_FRAME_SCAFFOLD
    } else {
        CASTLE_FRAME_HALF
    };
    let base = base + level * 4;
    Some(CastleStamp {
        bank: CASTLE_BANK_BYTE,
        frames: core::array::from_fn(|q| base + BLOCK_QUADRANTS_2[q]),
        terrain: terrain::CASTLE_PLOT + castle_type,
    })
}

