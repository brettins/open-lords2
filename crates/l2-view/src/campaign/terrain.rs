#![allow(unused_imports)]
use super::*;
use l2_formats::maps::{MapSlot, Plane, LATTICE_H, LATTICE_W, PLANE_DIM};
use crate::canvas::{Canvas, Clip, Tags};
use crate::sheet::Sheet;

/// **Rotations 2, 4 and 6 are not implemented, and the original never reaches
/// them either.** [V] `Map_BuildLattice` (`0x004298C1`) builds all four, and
/// `Map_RotateCW` (`0x00429F12`) / `Map_RotateCCW` (`0x0042A01B`) step between
/// them — but `Map_RotateCW`'s only caller is `Map_RotateCCW`, `Map_RotateCCW`
/// has none, and neither address appears in a widget table. `g_mapRotation`
/// (`0x00522F7C`) is written nowhere else except `Map_LoadPlanes`
/// (`0x00467770`), which zeroes it per map load. So the shipped game runs at
/// rotation 0, its four readers all take the rotation-0 branch, and building
/// rotation 0 only is a match, not a gap.
pub fn tile_to_cell(x: usize, y: usize) -> (i32, i32) {
    let (x, y) = (x as i32, y as i32);
    (x + y + 1, (x - y + PLANE_DIM as i32) >> 1)
}

/// **`Terrain_Set` (`0x0046D7F4`) — the single writer of a tile's `content`
/// byte, and the terrain → picture map the renderer needs.[V]**
///
/// The third parameter is `variant * 4` — a whole sub-block shift on top of the
/// base. This heading used to say it was dead, *"all sixteen call sites pass
/// zero"*, and C124 found the twenty-fourth that does not: `Grain_SeasonTick`'s
/// forward through `FUN_00469D21`. That paragraph stood here, above the table,
/// for the whole life of the fix that refuted it — **a claim that produced a
/// defect once, left in the one place a person reads before touching this
/// table.** [`field_variant`] is the term.
pub const FIELD_BASES: [(u8, u8, u8); 10] = [
    (0x00, 80, BANK_ROADS),  // wild
    (0x01, 84, BANK_ROADS),  // fallow — ploughed and bare
    (0x02, 88, BANK_ROADS),  // 0x02 … 0x12: grain, and pasture up to 18
    (0x13, 104, BANK_ROADS), // 0x13 … 0x16, and anything from 0x1D up
    (0x17, 130, BANK_BASE),  // harvested — and in the **base** bank, not roads
    (0x18, 134, BANK_BASE),
    (0x19, 108, BANK_ROADS), // the four reclamation stages
    (0x1A, 112, BANK_ROADS),
    (0x1B, 116, BANK_ROADS),
    (0x1C, 120, BANK_ROADS),
];

pub const BANK_BASE: u8 = 0x00;
pub const BANK_ROADS: u8 = 0x08;

pub fn field_base(terrain: u8) -> (u8, u8) {
    match terrain {
        0x00 => (80, BANK_ROADS),
        0x17 => (130, BANK_BASE),
        0x18 => (134, BANK_BASE),
        0x01 => (84, BANK_ROADS),
        0x19 => (108, BANK_ROADS),
        0x1A => (112, BANK_ROADS),
        0x1B => (116, BANK_ROADS),
        0x1C => (120, BANK_ROADS),
        t if t < 0x13 => (88, BANK_ROADS),
        _ => (104, BANK_ROADS),
    }
}

pub fn field_graphic(terrain: u8, stored_frame: u8) -> (u8, u8) {
    let (_, layer) = field_base(terrain);
    let bank = ((((BANK_ROADS | 1) & 0xE3) | layer) & 0x7F)
        | if (0x0F..0x17).contains(&terrain) { 0x80 } else { 0 };
    (bank, field_frame(terrain, stored_frame))
}

/// A player: *"The wheat fields don't show the wheat growing."* He is right, and
/// the reason is a `[V]` claim in `docs/formats/maps-layers.md` §5.5 that says
/// *"The third parameter is dead … all sixteen call sites in the shipped binary
/// pass zero — including the two that forward a parameter (`FUN_00469D21`, whose
/// only callers are `Grain_SeasonTick` and `Herd_UpdateCrowding`, and both pass
/// `'\0'`)."*
///
/// ```c
/// /* Spring, Summer, Autumn */ band = FUN_0044CF6F(county.crop[1], (byte)county.field_0x206);
/// /* Winter, the harvest   */ band = FUN_0044CF6F(county.crop[2], (byte)county.field_0x206);
/// variant = band < 3 ? 0 : (band - 3) / 4 + 1;               /* 0, 1, 2 or 3  */
/// FUN_00469D21(county, band, variant, 2, 0xE);   /* every grain tile of the county */
/// ```
///
/// and `FUN_0044CF6F` is four sacks-per-field bands — `< 1` field or crop → 2,
/// `< 0x29` → 3, `< 0x51` → 7, else 11. So a growing crop moves the tile's
/// `content` through **2, 3, 7, 11** and its variant through **0, 1, 2, 3**.
///
/// **This snippet used to read `FUN_0044CF6F(county.crop[2], county.fieldsGrain)`
/// for every season**, and the kingdom crate was written to it: `crop[2]` is
/// zero outside Winter, so the fix that added this function drew variant 0 for
/// three seasons in four and a player reported the wheat *still* did not grow.
///
/// `docs/decisions.md` C195.
///
/// **The variant is recoverable from the terrain byte**, so no new state is
/// needed: the only writer that ever produces a non-zero one is the line above,
/// and it writes the band it derives the variant from. Twenty-three of the
/// twenty-four `Terrain_Set` call sites pass a literal `'\0'`; the
/// twenty-fourth is `FUN_00469D21`, and this is the arithmetic of its one
/// caller that does not.
///
/// **The artwork closes it.** `Roads1a.pl8` frames 88 … 103 are sixteen 58 × 30
/// diamonds, and the count of ripe-gold pixels rises with the variant at every
/// one of the four `stored & 3` positions — 36/34/36/33, then 45/43/46/43, then
/// 54/50/55/49, then 71/70/72/69 — while the fallow block before them (84 … 87)
/// and the pasture block after (104 … 107) have two to seven each. Four blocks
/// of four starting at 88 end at 103, and 104 is exactly where the next base
/// begins. `crates/l2-view/tests/install/main.rs` asserts the ramp. **[V]**
pub fn field_variant(terrain: u8) -> u8 {
    if (3..=0x12).contains(&terrain) {
        (terrain - 3) / 4 + 1
    } else {
        0
    }
}

pub fn field_frame(terrain: u8, stored_frame: u8) -> u8 {
    field_base(terrain).0 + (stored_frame & 3) + field_variant(terrain) * 4
}


/// Each entry is `(idle, first working, last working, wrecked)`, and all
/// sixteen numbers are literals read out of the four arms of `Sprite_TopIt`
/// (`0x004071A0`) and `Industry_UpdateSiteTile` (`0x0044EDC2`), which supplies
/// the idle column:
pub const INDUSTRY_FRAMES: [(u8, u8, u8, u8); 4] =
    [(20, 20, 28, 29), (30, 30, 45, 46), (10, 10, 18, 19), (0, 1, 4, 9)];

pub fn industry_step(commodity: usize, frame: u8) -> u8 {
    let (_, first, last, _) = INDUSTRY_FRAMES[commodity.min(3)];
    let next = frame.saturating_add(1);
    if next > last {
        first
    } else {
        next
    }
}

/// ```c
/// n = county.industry[k].total - county.industry[k].totalSnapshot;
/// if      (n < 0x0A) step = DAT_0058FD08;   /* 640 ms */
/// else if (n < 0x19) step = DAT_0057D3C8;   /* 320 ms */
/// else if (n < 0x32) step = g_pulse160;     /* 160 ms */
/// else               step = g_pulse80;      /*  80 ms */
/// ```
///
/// The four globals are four rungs of `Tick_Pulses`' (`0x004BBC80`) divider
/// chain: a 20 ms `timeGetTime` gate feeds an 80 ms pulse, which feeds counters
/// firing every 2, 4 and 8 of it. `g_pulse80` and `g_pulse160` are already
/// named in `docs/symbols.json`; `DAT_0057D3C8` is the fourth counter and
/// `DAT_0058FD08` the eighth, so 320 ms and 640 ms.
pub fn industry_period_ms(output: i32) -> u32 {
    if output < 0x0A {
        640
    } else if output < 0x19 {
        320
    } else if output < 0x32 {
        160
    } else {
        80
    }
}

