#![allow(unused_imports)]
use super::*;
use l2_formats::maps::{MapSlot, Plane, LATTICE_H, LATTICE_W, PLANE_DIM};
use crate::canvas::{Canvas, Clip, Tags};
use crate::sheet::Sheet;

/// `maps-layers.md` §4, rotation 0 — recovered again here out of
/// `Map_BuildLattice`'s loop, which writes `row = 1 + y + x`,
/// `col = (64 - y + x) / 2`.
///
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
/// Every state change on the campaign map goes through it: the field brush
/// (`Field_SetType`), the seasonal crop pass, `Field_ReclaimTick`,
/// `County_DestroyField`, `Unit_TrampleTile` and `Counties_PlaceSites` all
/// call it, sixteen call sites in all. So this is not inferred from what the
/// tiles look like — it is the game's own assignment.
///
/// ```c
/// frame = ((frame - oldBase) & 3) + base + variant * 4;
/// bank  = (((bank | 1) & 0xE3) | layer) & 0x7F;
/// if (0x0E < terrain && terrain < 0x17) bank |= 0x80;
/// ```
///
/// # `& 3` is the point
///
/// Every field state is **four consecutive frames** and the tile keeps
/// whichever of the four it already had, so a repaint changes the crop without
/// changing the tile's variation. Every base below is a multiple of four
/// **except** 130 and 134, and `oldBase` exists for exactly that: it is `0x82`
/// when the *previous* terrain was `0x17` or `0x18` and zero otherwise, which
/// subtracts the offset those two introduce. (The original writes `0x82` for
/// both, not `0x82` and `0x86`; `130 & 3` and `134 & 3` are both 2, so it makes
/// no difference and the shortcut is harmless.)
///
/// The consequence is the one this function relies on: **the low two bits of a
/// farm tile's frame never change.** Whatever `L2_maps.dat` stored is the
/// variant for the life of the game, so [`field_frame`] can be a
/// pure function of the terrain and the stored frame
/// tile's history.
///
/// The claim self-checks against the map file: farm tiles on disk are bank
/// `0x20`/roads **frame 80 only**, with boundary twins 81 … 83 — which is
/// `base = 80` and its four variants exactly (`maps-layers.md` §1.2).
///
/// # `variant` is not dead
///
/// The third parameter is `variant * 4` — a whole sub-block shift on top of the
/// base. This heading used to say it was dead, *"all sixteen call sites pass
/// zero"*, and C124 found the twenty-fourth that does not: `Grain_SeasonTick`'s
/// forward through `FUN_00469D21`. That paragraph stood here, above the table,
/// for the whole life of the fix that refuted it — **a claim that produced a
/// defect once, left in the one place a person reads before touching this
/// table.** [`field_variant`] is the term.
pub const FIELD_BASES: [(u8, u8, u8); 10] = [
    // (terrain, first frame of the four, plane-1 bank byte for the layer)
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

/// The plane-1 byte for the `base` bank — `Base1?.pl8`, bank index 0.
pub const BANK_BASE: u8 = 0x00;
/// The plane-1 byte for the `roads` bank — `Roads1?.pl8`, bank index 2.
pub const BANK_ROADS: u8 = 0x08;

/// The first frame of a terrain's four, and the bank layer it draws from.
///
/// The ladder is `Terrain_Set`'s, in its own order — the specific values are
/// tested before the two ranges, so `0x17` and `0x18` do not fall
/// into the `0x13 …` arm and `0x19 … 0x1C` do not fall into the tail.
///
/// **The tail catches more than `maps-layers.md` §5.5's table says.** The
/// original's last arm is a bare `else`, so every terrain at `0x13` or above
/// that is not one of `0x17 … 0x1C` lands on base 104 — including `0x1D` and
/// up, which `County_RecountFields` buckets as *being reclaimed*. The
/// document's table reads as if `0x13 … 0x16` were exhaustive. It is not
/// wrong about those four; it is silent about the ones past `0x1C`.
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

/// **The picture for one farm tile.** Returns `(plane-1 byte, frame)`, ready
/// for [`Overrides::set`].
///
/// `stored_frame` is the tile's graphic index as `L2_maps.dat` holds it — the
/// low two bits of which are the tile's variant, permanently (see
/// [`FIELD_BASES`]). `Map_PlaceStartingFields` writes
/// `frame = base + ((frame + 0xB0) & 3)` from the other side and `0xB0` is a
/// multiple of four, so the two functions agree on which two bits carry the
/// variant.
///
/// The bank byte reproduces `Terrain_Set`'s own arithmetic —
/// `(((bank | 1) & 0xE3) | layer) & 0x7F` — applied to the byte the file holds
/// for a farm tile, which is always `0x08`, the roads bank. That comes out
/// `0x09` for a roads-layer state and `0x01` for a base-layer one; bit `0x01`
/// is the road bit the original sets unconditionally and bits `0x1C` are the
/// bank, which is all [`draw`] reads.
///
/// **Bit `0x80` is set for terrain `0x0F … 0x16` and changes no pixel here.**
/// It is a run-time *draw* bit asking for the building-overlay blitter
/// (`maps-layers.md` §5.3) — on a pasture, presumably the animals — and this
/// crate has no such blitter, so it is carried:
/// [`Overrides`] stores the plane-1 byte and a caller reading it back should
/// see what the game's own tile record would hold.
pub fn field_graphic(terrain: u8, stored_frame: u8) -> (u8, u8) {
    let (_, layer) = field_base(terrain);
    let bank = ((((BANK_ROADS | 1) & 0xE3) | layer) & 0x7F)
        | if (0x0F..0x17).contains(&terrain) { 0x80 } else { 0 };
    (bank, field_frame(terrain, stored_frame))
}

/// **`Terrain_Set`'s third parameter, and it is not dead.** This is the wheat.
///
/// A player: *"The wheat fields don't show the wheat growing."* He is right, and
/// the reason is a `[V]` claim in `docs/formats/maps-layers.md` §5.5 that says
/// *"The third parameter is dead … all sixteen call sites in the shipped binary
/// pass zero — including the two that forward a parameter (`FUN_00469D21`, whose
/// only callers are `Grain_SeasonTick` and `Herd_UpdateCrowding`, and both pass
/// `'\0'`)."*
///
/// **`Herd_UpdateCrowding` passes `'\0'`. `Grain_SeasonTick` does not.** Its
/// three arms each band the crop, and its last lines paint the result:
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
/// `l2_kingdom::land::grain_stage_band` is the other half now.
/// `docs/decisions.md` C195.
///
/// **The content byte alone carries none of that.** All four values fall in
/// `2 … 0x12`, whose base is 88 — so `base + (stored & 3)` is the same four
/// frames at every stage of growth, and a renderer that drops the variant draws
/// a just-sown field all year. That is the defect, and it means the two halves
/// of this bug are not independent: fixing the season pass without this would
/// change no pixel either.
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
    // `band < 3 ? 0 : (band - 3) / 4 + 1`, over the range `Terrain_Set` gives
    // base 88. Everything else in the game passes a literal zero.
    if (3..=0x12).contains(&terrain) {
        (terrain - 3) / 4 + 1
    } else {
        0
    }
}

/// The frame alone, for a caller that already knows the bank.
///
/// `Terrain_Set`: `frame = ((frame - oldBase) & 3) + base + variant * 4`.
pub fn field_frame(terrain: u8, stored_frame: u8) -> u8 {
    field_base(terrain).0 + (stored_frame & 3) + field_variant(terrain) * 4
}

// ------------------------------------------------------- the industry sites

/// **`Sprite_TopIt` arm 5 — one industry site's four frame numbers**, in
/// commodity order (wood, iron, weapons, stone) so it indexes the way
/// `County::industry` does.
///
/// Each entry is `(idle, first working, last working, wrecked)`, and all
/// sixteen numbers are literals read out of the four arms of `Sprite_TopIt`
/// (`0x004071A0`) and `Industry_UpdateSiteTile` (`0x0044EDC2`), which supplies
/// the idle column:
///
/// | commodity | idle | working run | wrecked |
/// |---|---:|---|---:|
/// | wood | 20 | 20 … 28 | 29 |
/// | iron | 30 | 30 … 45 | 46 |
/// | weapons | 10 | 10 … 18 | 19 |
/// | stone | **0** | 1 … 4 | 9 |
///
/// **Stone is the one whose idle frame is outside its working run**, and it is
/// not a slip: `Industry_UpdateSiteTile` writes 0 and `Sprite_TopIt`'s stone arm
/// wraps `if (frame > 4) frame = 1`, so a quarry switched on shows its idle
/// picture for one pulse and then never again until it stops. Everything else
/// starts on the first frame of its own run.
///
/// Every number lands inside `Town1a.pl8`'s 61 frames, which is the check
/// `docs/draws-map.md` §3.1 makes on the same table — and it is also why
/// `L2_maps.dat` storing 0, 20 and 30 is the *idle* frame in every case and not
/// three unrelated pictures.
pub const INDUSTRY_FRAMES: [(u8, u8, u8, u8); 4] =
    [(20, 20, 28, 29), (30, 30, 45, 46), (10, 10, 18, 19), (0, 1, 4, 9)];

/// One step of an industry site's wheel — `Sprite_TopIt`'s arm 5b, whole.
///
/// ```c
/// frame++;
/// if (frame > last) frame = first;
/// tile.frame = frame;
/// ```
///
/// It is a **rewrite of the tile's own terrain frame** and draws no overlay at
/// all, so a working mine and an idle one are the same sprite sheet,
/// the same bank and the same position: the only difference on screen is that
/// one of them is moving. A reader looking for the "on" picture will not find
/// one.
pub fn industry_step(commodity: usize, frame: u8) -> u8 {
    let (_, first, last, _) = INDUSTRY_FRAMES[commodity.min(3)];
    let next = frame.saturating_add(1);
    if next > last {
        first
    } else {
        next
    }
}

/// **How fast the wheel turns, which is a mechanic and not decoration.**
///
/// `Sprite_TopIt` chooses the pulse from `total − totalSnapshot` — the
/// commodity's output *this season* — and the bands are the original's
/// literals:
///
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
///
/// **So the busier the mine, the faster its wheel turns — eight times faster at
/// the top band than at the bottom — and the picture is the only place the
/// player is told.** A site switched on but unstaffed produces nothing and
/// therefore still turns, at the slowest rate; a site switched *off* has no
/// working terrain and does not turn at all.
///
/// Returned in milliseconds so the caller can convert to whatever tick it has.
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

