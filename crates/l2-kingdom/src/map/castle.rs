#![allow(unused_imports)]
use super::*;
use super::site::*;
use super::campaign::*;
use super::cost::*;
use crate::tables::{MOVE_COST_BLOCKED, MOVE_COST_IMPASSABLE};


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

pub const CASTLE_BANK_BYTE: u8 = 0x11;

/// **What a castle looks like** — `Castle_StampTile` (`0x0046826C`), which is
/// the reason a castle is missing from our campaign map entirely.
///
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

pub const CASTLE_FRAME_BUILT: u8 = 0x50;
pub const CASTLE_FRAME_SCAFFOLD: u8 = 0x28;
pub const CASTLE_FRAME_HALF: u8 = 0x3C;

pub const CASTLE_HALF_PERCENT: u8 = 0x32;

pub fn stamp_castle_terrain(map: &mut CampaignMap, county: u8, castle_type: u8) -> usize {
    let Some(t) = terrain::CASTLE_PLOT.checked_add(castle_type.min(5)) else { return 0 };
    let tiles = castle_tiles(map, county);
    for i in &tiles {
        map.terrain[*i] = t;
    }
    tiles.len()
}

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

