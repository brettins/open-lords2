#![allow(unused_imports)]

mod units;
pub use units::*;
mod markers;
pub use markers::*;

use super::*;
use l2_formats::maps::{MapSlot, Plane, LATTICE_H, LATTICE_W, PLANE_DIM};
use crate::canvas::{Canvas, Clip, Tags};
use crate::sheet::Sheet;

/// Which sheet, which frame and which per-kind nudge one unit draws with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitSprite {
    /// 0 for `g_spriteSheetA`, 1 for `g_spriteSheetB`. `Map_DrawArmies` picks B
    /// for `kind == 4` — a transport — and A for everything else.
    pub sheet: usize,
    pub frame: usize,
    /// The per-kind `(x, y)` the original adds before centring.
    pub nudge: (i32, i32),
    /// **How far across its tile the unit is** — [`walk_offset`] for its
    /// facing and `+0x149`. `(0, 0)` is a unit at rest.
    pub walk: (i32, i32),
}

/// `DAT_004D8108` — zoom 0, `x`, `[facing][+0x149]`.
const WALK_NEAR_X: [[i8; 16]; 8] = [
    [0, -28, -26, -24, -22, -20, -18, -16, -14, -12, -10, -8, -6, -4, -2, 0],
    [0, -56, -52, -48, -44, -40, -36, -32, -28, -24, -20, -16, -12, -8, -4, 0],
    [0, -28, -26, -24, -22, -20, -18, -16, -14, -12, -10, -8, -6, -4, -2, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 28, 26, 24, 22, 20, 18, 16, 14, 12, 10, 8, 6, 4, 2, 0],
    [0, 56, 52, 48, 44, 40, 36, 32, 28, 24, 20, 16, 12, 8, 4, 0],
    [0, 28, 26, 24, 22, 20, 18, 16, 14, 12, 10, 8, 6, 4, 2, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
];
/// `DAT_004D8188` — zoom 0, `y`.
const WALK_NEAR_Y: [[i8; 16]; 8] = [
    [0, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, -14, -13, -12, -11, -10, -9, -8, -7, -6, -5, -4, -3, -2, -1, 0],
    [0, -28, -26, -24, -22, -20, -18, -16, -14, -12, -10, -8, -6, -4, -2, 0],
    [0, -14, -13, -12, -11, -10, -9, -8, -7, -6, -5, -4, -3, -2, -1, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0],
    [0, 28, 26, 24, 22, 20, 18, 16, 14, 12, 10, 8, 6, 4, 2, 0],
];
/// `DAT_004D8308` — zoom 2, `x`.
const WALK_FAR_X: [[i8; 16]; 8] = [
    [0, -5, -5, -5, -4, -4, -3, -3, -3, -2, -2, -1, -1, -1, 0, 0],
    [0, -11, -10, -9, -9, -8, -7, -6, -6, -5, -4, -3, -3, -2, -1, 0],
    [0, -5, -5, -5, -4, -4, -3, -3, -3, -2, -2, -1, -1, -1, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 5, 5, 5, 4, 4, 3, 3, 3, 2, 2, 1, 1, 1, 0, 0],
    [0, 11, 10, 9, 9, 8, 7, 6, 6, 5, 4, 3, 3, 2, 1, 0],
    [0, 5, 5, 5, 4, 4, 3, 3, 3, 2, 2, 1, 1, 1, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
];
/// `DAT_004D8388` — zoom 2, `y`.
const WALK_FAR_Y: [[i8; 16]; 8] = [
    [0, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, -2, -2, -2, -2, -2, -1, -1, -1, -1, -1, 0, 0, 0, 0, 0],
    [0, -5, -5, -5, -4, -4, -3, -3, -3, -2, -2, -1, -1, -1, 0, 0],
    [0, -2, -2, -2, -2, -2, -1, -1, -1, -1, -1, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0],
    [0, 5, 5, 5, 4, 4, 3, 3, 3, 2, 2, 1, 1, 1, 0, 0],
];

#[cfg(test)]
mod walk_tests {
    use super::*;

    /// **The bytes and the projection agree about every step**,
/// makes the tables `[V]`.
    ///
    /// Two sources that share nothing: the tables out of `Map_DrawArmies`, and
    /// our own isometric projection ([`tile_to_cell`], [`cell_to_screen`], which
    /// the tile artwork is asserted against). For each of `Dir_FromDelta`'s
    /// eight headings — typed, 0 north clockwise — the table's index 1 must
    /// point from the destination back at the tile the unit left, fall short of
    /// it by no more than a seventh, and be zero at both ends.
    ///
    /// **Ablated**: swapping two rows of `WALK_NEAR_X`, or negating one, turns
    /// this red at that heading; so does numbering the headings from east.
    #[test]
    fn each_table_drags_the_figure_back_toward_the_tile_it_left() {
        const HEADINGS: [(i32, i32); 8] =
            [(0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1)];
        for zoom in ZOOMS {
            let view = Viewport::new(40, 20);
            for (dir, &(dx, dy)) in HEADINGS.iter().enumerate() {
                let (from, to) = ((32usize, 32usize), ((32 + dx) as usize, (32 + dy) as usize));
                let (fr, fc) = tile_to_cell(from.0, from.1);
                let (tr, tc) = tile_to_cell(to.0, to.1);
                let (a, b) = (cell_to_screen(view, &zoom, fr, fc), cell_to_screen(view, &zoom, tr, tc));
                let back = (a.0 - b.0, a.1 - b.1);
                let whole = walk_offset(&zoom, dir as u8, 1);
                for (axis, (w, s)) in [(whole.0, back.0), (whole.1, back.1)].into_iter().enumerate() {
                    let what = format!("zoom {} heading {dir} axis {axis}: table {w}, projection {s}", zoom.id);
                    assert_eq!(w.signum(), s.signum(), "{what}");
                    assert!(w.abs() <= s.abs() && s.abs() - w.abs() <= s.abs() / 7 + 1, "{what}");
                }
                assert_eq!(walk_offset(&zoom, dir as u8, 0), (0, 0), "at rest");
                assert_eq!(walk_offset(&zoom, dir as u8, 15), (0, 0), "the last admission");
                assert_eq!(walk_offset(&zoom, dir as u8, 16), (0, 0), "not a state, drawn at rest");
            }
        }
    }
}

/// `Flags1a.pl8` frame `0x82` — the tent an army camped outside a castle flies,
/// `DAT_005C9288 = 0x82` in `FUN_00407F82` and its only appearance in the
/// image.
pub const BESIEGER_MARKER_FRAME: usize = 0x82;

/// Ten pixels below the mark's own top-left, `y0 + dy + 10`.
pub const BESIEGER_COUNT_DY: i32 = 10;

/// `if (0x18 < …)` — the menu bar's height, and the count's only clip.
pub const BESIEGER_COUNT_TOP: i32 = 0x18;

/// `0xF9`, the pen `FUN_00407F82` passes and the one the diplomacy screen's
/// inner selection outline uses. Not the ordinary `0x3F`.
pub const BESIEGER_COUNT_INK: u8 = 0xF9;

/// Where `Map_DrawPathMarker` (`0x004081A6`) puts a ball: `(+0x14, +6)` from the
/// tile origin, at **both** zooms — the function has no zoom test on the
/// offset, only on the vertical clip. **[D]**, two literals.
pub const PATH_MARKER_AT: (i32, i32) = (0x14, 6);

/// **The path ball's frame** — `0x38 + n`, and the frame index *is* the
/// accumulated cost; **or `0x4E` on a tile a click would act on.**
///
/// `Map_DrawPathMarker` (`0x004081A6`), verbatim:
///
/// ```c
/// local_14 = flags & 0x50;                                   /* the town, or a dwelling plot */
/// if ((flags & 0x80) != 0 && content != 0x14) local_14 = 1;  /* a site or a castle, not a bare plot */
/// n = g_moveDistLocal[tile] - 1;
/// if (unit.moveAllowance - unit.movesUsed < n) n = 0;       /* out of range */
/// frame = local_14 == 0 ? 0x38 + n : 0x4E;
/// ```
///
/// So nothing about the realm, the shield or the unit's kind selects the
/// colour: **the cost does**, and everything past the remaining budget collapses
/// to [`PATH_MARKER_FIRST`]. `docs/armies.md` §2.3. **[V]**
///
/// **The action arm was missing, and a player found it by attacking a town.**
/// *"The balls of the army movement are missing the gold ball of action, it's
/// just a grey ball like I can't get there."* A county town costs 100 to enter,
/// so its cost is always past the budget and the cost arm greys it — and the
/// original never asks the cost arm about that tile at all. The test is the
/// tile's own plane-0 bits and terrain, and **only** those: no owner (your own
/// town is gold too), no reach (a town past the budget is gold), and no unit
/// (an enemy army on open ground is coloured by its cost like any other tile).
/// See [`PATH_MARKER_ACTION`].
///
/// # What the sheet says, now that somebody has looked at it
///
/// `docs/armies.md` carried *"**[I]** that frame `0x38` is specifically the grey
/// one — nobody has looked at the sheet"*. Measured against a real
/// `Flags1a.pl8`: frames `0x38 … 0x4E` are **23 frames, every one of them
/// 15 × 15 with the same 177-pixel silhouette** — one picture, recoloured — and
/// **`0x38` is the only frame in the run whose every opaque pixel is a true
/// grey** (`r == g == b`). Frame `0x39` has 13 coloured pixels, and the count
/// climbs to 39 by `0x4D`, so the ball gains colour as the cost rises. The
/// inference is now a measurement, and it is asserted in
/// `crates/l2-view/tests/install/main.rs` against the user's own file. **[V]**
///
/// `0x4E` is the odd one out and is the action marker the ladder above names:
/// 177 coloured pixels and **not one grey**, a different picture entirely.
pub const PATH_MARKER_FIRST: usize = 0x38;

/// `Map_DrawPathMarker`'s `0x4E` — the ball on a tile a click would act on: a
/// county town (`0x40`), a dwelling plot (`0x10`), or an industry site or
/// standing castle (`0x80` with terrain other than the bare plot's `0x14`).
///
/// It is the *gold* ball of the player's report, and it is the one frame of the
/// run that is not a recolouring of `0x38`.
pub const PATH_MARKER_ACTION: usize = 0x4E;

/// The last cost-indexed ball. `0x4E` is the castle/settlement marker and is
/// not part of the ramp.
pub const PATH_MARKER_LAST: usize = 0x4D;

/// **The waving flag's frame** — `shield * 8 - 8 + phase`, i.e.
/// `(shield − 1) * 8 + phase`.
///
/// `Flags1a.pl8`'s first 40 frames are 32 × 24 and lie on the artist's sheet as
/// five rows of eight: **five shields × eight wave phases**, and `shield = 5,
/// phase = 7` lands on frame 39, the last of them. The colour is in the frame
/// index; `shield` is the realm's `shieldIndex`,
/// clamped 1 … 5 by the original, so a zero shield has no flag.
pub const FLAG_PHASES: u8 = 8;

/// `Flags1a.pl8` frame `0x81`, the mercenary-offer marker — drawn on the town
/// block's **`part == 2`** quadrant, which is the tile one row *south* of the
/// block's origin and **not** the north-east one this line used to name.
///
/// **Two unrelated painters pass this one index**, which is what makes it `[V]`
///
///
/// * `Sprite_TopIt` (`0x004071A0`) — [`draw_mercenary_marker`], on the map;
/// * `TileInfo_Draw` (`0x0041C208`) — `Pl8_DrawFrameClipped(g_flagsSheet, 0x81,
///   0x32, row * 0x10 + 0x9C)` on the tile information panel, immediately above
///   `L2.eng` 30/59 *"Mercenaries are available for hire in the county."*
///
/// The frame is 25 × 45 in the player's own `Flags1a.pl8`.
pub const MERCENARY_MARKER_FRAME: usize = 0x81;

// ------------------------------------------------------------- the cattle

/// **The herd grazing on a pasture** — `FUN_004071A0`'s farm arm, the third of
/// its four and the only one that had never been read.
///
/// The same overlay pass that draws the town's flag draws the animals, off the
/// same sheet, on the same bank bit. `Terrain_Set` sets bank bit `0x80` when
/// `0x0E < terrain && terrain < 0x17` — **exactly the pasture range**, which is
/// why a pasture is the one field state that gets a second blit at all
/// ([`field_graphic`] already carried the bit and called it *"presumably the
/// animals"*; it is).
///
/// The arm, in full, with `flags & 0x20` (farmland) and neither `0x40` nor
/// `0x10` set:
///
/// ```c
/// if (g_mapZoom == 2)        return;      /* no animals at the far zoom */
/// if (content < 0x0F)        return;
/// if (0x16 < content)        return;
/// if (content < 0x13) {                   /* the dead half — see below   */
///     dx = 0; dy = 0;
///     if (2 < (byte)(content - 0x10)) return;
///     frame = (content - 0x10) * 6 + phase + 0x67;
/// } else {
///     dx = 4; dy = -4;
///     if (2 < (byte)(content - 0x14)) return;
///     frame = (content - 0x14) * 6 + phase + 0x55;
/// }
/// ```
///
/// `phase` is `_DAT_0057D38C`, and the `* 6` is the whole shape of the sheet:
/// **three herds of six frames each**, one herd per crowding band.
///
/// # Two states in that ladder draw nothing
///
/// `content == 0x0F` and `content == 0x13` both fall through the unsigned
/// `2 < (content - base)` guard and return — `0x13 - 0x14` is `0xFF` as a byte.
/// `0x13` is the value `l2_kingdom::land::herd_graphic` writes for **an empty
/// herd**, so a county that has lost every animal keeps its pasture and shows
/// bare grass. That is the original saying *"no cattle"* by drawing no cattle,
/// and it is the reason the guard is written as an unsigned compare
/// a range. **[V]**
///
/// # The `0x67` half is vestigial
///
/// The obvious reading of a second three-group block is *sheep*. It is not, and
/// two independent things say so.
///
/// **The game's own words.** The tile-info table at `0x004D2EC8` — 16 bytes an
/// entry, indexed by `content`, read by `TileInfo_Draw` — gives `0x0F … 0x12`
/// `L2.eng` group 30 descriptions **40 … 43** and mode **20**, which are
/// *"Over time and with continued labor, this field will return to a usable
/// state…"* through *"Almost reclaimed…"*, marked *"- Being reclaimed."* Those
/// are the **same four strings** the table gives `0x19 … 0x1C`, the live
/// reclamation ladder. `0x0F … 0x12` is an earlier encoding of reclamation that
/// moved, and the block of art reserved for it moved with it.
///
/// **The sheet.** Frames `0x67 … 0x78` are eighteen **2 × 2 stubs** — the same
/// padding as `0x4F … 0x54`, which is what makes the livestock block start on a
/// row of six — while `0x55 … 0x66` are eighteen frames of **58 × 30**, exactly
/// the near-zoom diamond.
/// [`tests::the_pasture_frames_are_full_tiles_and_the_dead_block_is_stubs`]
/// measures both against a real `Flags1a.pl8`.
///
/// And nothing writes `0x0F … 0x12` onto a farm tile in any case. `Terrain_Set`
/// is the single writer of a tile's `content` (`maps-layers.md` §5.5) and its
/// sixteen call sites pass `0`, `1`, `2 … 0x0E`, `0x13 … 0x16`, `0x17`, `0x18`,
/// `0x19 … 0x1C` and the brush's `{0, 1, 2, 0x13, 0x19}`. `County_UpdateDwellings`
/// does write those four values, but onto a tile carrying plane-0 bit `0x10`,
/// which `FUN_004071A0` tests **before** `0x20` and sends down the dwelling arm.
///
/// The arm is reproduced because the ladder is what the
/// function does, and a renderer that silently narrowed it would be asserting
/// the absence. **[V]** — an exhaustive scan of every
/// `Terrain_Set` call site and every direct `content` write in the corpus, plus
/// the two readings above, which were arrived at separately and agree.
///
/// # There are no sheep
///
/// A pasture is a *"dairy meadow"* in `L2.eng` group 30 and its mode line is
/// *"- Cattle."*
/// branch anywhere that picks a species — the five styles choose grain against
/// pasture and how many fields, never what grazes. Goods 3 (*Sheep*) and 5
/// (*Wool*) carry price 0, have no `Merchant_Trade` branch and no `mercgrid.pl8`
/// cell. **[V]**, and worth writing down because the question is natural and the
/// answer is a negative that cannot be found by looking harder.
pub const HERD_PHASES: u8 = 6;

/// The first frame of the reachable herds — `content 0x14`'s six.
pub const HERD_FIRST_FRAME: usize = 0x55;
/// The first frame of the three unreachable ones — `content 0x10`'s six.
pub const HERD_DEAD_FIRST_FRAME: usize = 0x67;

/// Where the animals go relative to the tile origin, for each half of the
/// ladder. The original does not scale either by zoom — unlike
/// [`Zoom::flag_at`], which has a value per zoom — so both are literal.
pub const HERD_AT: (i32, i32) = (4, -4);
pub const HERD_DEAD_AT: (i32, i32) = (0, 0);

/// Where a lattice cell's tile is drawn, given the viewport.
///
/// Odd lattice rows are shifted left by half a pitch — `maps-layers.md` §4's
/// parity term, and the reason `Map_RenderOffsetRow` exists as a separate
/// function in the original. The returned `y` is the **diamond's** top, not the
/// frame's: a frame with apex ("overhang") rows is taller than its tile and
/// [`draw`] subtracts the difference.
pub fn cell_to_screen(view: Viewport, zoom: &Zoom, row: i32, col: i32) -> (i32, i32) {
    let offset = if row & 1 == 1 { zoom.half_pitch } else { 0 };
    (
        zoom.view_x + (col - view.col) * zoom.pitch - offset,
        zoom.view_y + (row - view.row) * zoom.row_step,
    )
}

/// The centre of a map tile's diamond on screen, which is where a marker for
/// that tile goes. `None` when the tile is not in the viewport.
pub fn tile_centre(view: Viewport, zoom: &Zoom, x: usize, y: usize) -> Option<(i32, i32)> {
    let (row, col) = tile_to_cell(x, y);
    let (sx, sy) = cell_to_screen(view, zoom, row, col);
    let (cx, cy) = (sx + zoom.tile_w / 2, sy + zoom.tile_h / 2);
    zoom.clip().contains(cx, cy).then_some((cx, cy))
}


