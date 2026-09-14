#![allow(unused_imports)]
use super::*;
use super::builder::*;
use crate::siege::{
    code, FLAG_DRAWBRIDGE, FLAG_KEEP, FLAG_WALL, STRUCTURE_STONE, STRUCTURE_WOOD, SURFACE_BAILEY,
    SURFACE_DRAWBRIDGE, SURFACE_FIELD, SURFACE_GROUND, SURFACE_KEEP, SURFACE_RAMPART_WALK,
    SURFACE_WALL, SURFACE_WATER,
};
use crate::terrain::{flag, id, tileset, Battlefield, Cell, Lfsr, CELLS, DIM};
use crate::AiField;

/// The marker bytes the walker anchors on: `0x04` for side 0's tables and
/// `0x0F` for side 4's, which are the same two ids a `.skr` field battle uses
/// for its deployment markers ([`id::MARKER_SIDE0`], [`id::MARKER_SIDE4`] are
/// the terrain-layer spellings of the same idea).
pub const MARKER_SIDE0: u8 = 0x04;
pub const MARKER_SIDE4: u8 = 0x0F;

/// **Walk the structure layer** — `Battlefield_ReadStructureLayer`.
///
/// A marker is a 2 x 2 block. Byte `+0` says which side's tables it feeds,
/// byte `+1` is the **kind**, and the index is written underneath it:
///
/// * kind equal to the marker — a deployment slot, index `buf[+2] - 0x40`
///   clamped to `0..=11` (`Marker_DeploySlot`);
/// * side 0's other kinds — a four-bit index, `buf[+0x50 ..= +0x53]` each
///   contributing `8, 4, 2, 1` when it holds **7** (`Marker_Index4Bit`);
/// * side 4's other kinds — a two-bit index from `buf[+0x50]`, `buf[+0x51]`
///   (`Marker_Index2Bit`).
///
/// The walker zeroes each byte as it consumes it,
/// copy here: a `7` belonging to one marker must not be read again by the next.
/// Points are stored `(x + 2, y + 2)`, except a deployment slot's `(x + 2, y)`
/// and the reference cell's `(x, y - 2)` — the block's own corner offsets.
/// **[V]**
///
/// The four filled deployment slots are the check that this decode is right:
/// `Deploy_SlotForUnitSiege` (`0x004816F9`) reaches only slots 0, 1, 4 and 8,
/// and slots 0, 1, 4 and 8 are exactly the ones every shipped layout fills.
pub fn tables(sheet: &CastleSheet) -> CastleTables {
    assert_eq!(sheet.structures.len(), LAYER_BYTES, "a castle layer is exactly 80 x 80 bytes");
    let mut b = sheet.structures.clone();
    let mut t = CastleTables {
        deploy_side0: [(0, 0); 12],
        deploy_side4: [(0, 0); 12],
        wall_slot: [[(0, 0); 16]; 4],
        approach: [[(0, 0); 4]; 6],
    };
    // `Marker_Index4Bit` and `Marker_Index2Bit`: the row below the marker, read as bits
    // and then cleared.
    let index_of = |b: &mut [u8], i: usize, bits: usize| -> usize {
        let mut v = 0usize;
        for k in 0..bits {
            if b.get(i + 0x50 + k) == Some(&7) {
                v += 1 << (bits - 1 - k);
            }
        }
        for k in 0..4 {
            if let Some(x) = b.get_mut(i + 0x50 + k) {
                *x = 0;
            }
        }
        v
    };
    for y in 0..DIM {
        for x in 0..DIM {
            let i = y * DIM + x;
            let marker = b[i];
            if marker != MARKER_SIDE0 && marker != MARKER_SIDE4 {
                continue;
            }
            // `Marker_TakeKind`: take the kind and clear it, and `+3` with it.
            let kind = b[i + 1];
            b[i + 1] = 0;
            if let Some(v) = b.get_mut(i + 3) {
                *v = 0;
            }
            let (px, py) = (x as i16 + 2, y as i16 + 2);
            if marker == MARKER_SIDE0 {
                if kind == MARKER_SIDE0 {
                    // `Marker_DeploySlot`, and the y is the marker's own row.
                    let slot = (b[i + 2] as i16 - 0x40).clamp(0, 11) as usize;
                    b[i + 0x50] = 0;
                    b[i + 0x51] = 0;
                    t.deploy_side0[slot] = (px, y as i16);
                } else if let Some(g) = group_of(kind) {
                    let k = index_of(&mut b, i, 4);
                    t.wall_slot[g][k] = (px, py);
                }
            } else if kind == MARKER_SIDE4 {
                let slot = (b[i + 2] as i16 - 0x40).clamp(0, 11) as usize;
                b[i + 0x50] = 0;
                b[i + 0x51] = 0;
                t.deploy_side4[slot] = (px, y as i16);
            } else if kind == 0x43 {
                // `DAT_0055CDD0` … `DAT_0055CDEC`: one point, written into all
                // four lanes of row 2.
                t.approach[2] = [(x as i16, y as i16 - 2); 4];
            } else if let Some(row) = approach_row(kind) {
                let k = index_of(&mut b, i, 2);
                t.approach[row][k] = (px, py);
            }
        }
    }
    t
}

/// Side 0's four wall-slot groups, by their base address: `0x00554180`,
/// `0x00554200`, `0x00554280`, `0x00554380` — `0x80` apart, so kind `0x47`
/// lands in group **4** with group 3 never written.
fn group_of(kind: u8) -> Option<usize> {
    match kind {
        0x40 => Some(0),
        0x41 => Some(1),
        0x44 => Some(2),
        0x47 => Some(3),
        _ => None,
    }
}

/// Side 4's approach rows, `0x20` apart from `0x0055CD90`.
fn approach_row(kind: u8) -> Option<usize> {
    match kind {
        0x40 => Some(0),
        0x41 => Some(1),
        0x47 => Some(3),
        0x44 => Some(5),
        _ => None,
    }
}

/// **The AI's view of a real castle**, out of [`tables`]
/// ring we drew.
///
/// [`crate::siege::our_castle_ai_field`] is the same shape filled from our
/// stand-in's geometry; every field it marks `[I]` is a `[V]` here, because
/// this is the table `Battlefield_ReadStructureLayer` writes.
///
/// Two of our fields alias one of the original's: `castle_approach[3]` and
/// [`crate::AiField::staging`] are both `0x0055CDF0`, and `castle_approach[2]`
/// and [`crate::AiField::castle_ref`] are both `0x0055CDD0`. They are filled
/// consistently
pub fn ai_field(field: &Battlefield, level: u8, t: &CastleTables) -> AiField {
    let mut f = crate::runner::ai_field_for(field);
    f.castle_approach = t.approach;
    f.castle_ref = t.approach[2][0];
    f.staging = t.approach[3];
    f.wall_slot = [t.wall_slot[0], t.wall_slot[1], t.wall_slot[2]];
    f.castle_index = 13;
    f.layout = 1;
    // `DAT_00542CD4`, the flag two handlers jump their orders to 100 on:
    // `Battlefield_BuildCastle` writes `g_castleLevel == 1` in the campaign.
    f.castle_layout_flag = level == 1;
    // `DAT_00553274` — one row north of the keep door — and `DAT_00553EE4`,
    // one row north and one east of the first curtain block.
    let first = |pred: &dyn Fn(&Cell) -> bool, back: usize| {
        field.cells.iter().position(pred).map(|i| i.saturating_sub(back)).unwrap_or(0)
    };
    f.castle_objective = [
        first(&|c| c.flags & FLAG_KEEP != 0, DIM),
        first(&|c| c.flags & FLAG_WALL != 0, DIM - 1),
    ];
    // The posts are the holes, and they arrive when the holes do — see
    // `siege::our_castle_ai_field`.
    f.defence_posts = [0; 20];
    f
}

