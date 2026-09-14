#![allow(unused_imports)]
use super::*;
use super::units::*;
use super::*;
use l2_formats::maps::{MapSlot, Plane, LATTICE_H, LATTICE_W, PLANE_DIM};
use crate::canvas::{Canvas, Clip, Tags};
use crate::sheet::Sheet;

/// **`FUN_004071A0`'s flag** — an owner-coloured banner over a tile.
///
/// Placed at `tileOrigin + Zoom::flag_at` with **no centring at all**: the
/// function adds the offset to the draw cursor and blits, and never reads the
/// frame record's `cx`/`cy`, which are atlas coordinates. Near zoom that is
/// `(+26, −28)` — up and to the right of the diamond's top-left, so the flag
/// flies above the tile.
pub fn draw_flag(
    canvas: &mut Canvas,
    assets: &MapAssets,
    view: Viewport,
    zoom: &Zoom,
    tile: (usize, usize),
    frame: usize,
    clip: Clip,
) -> bool {
    blit_over_tile(canvas, assets, view, zoom, tile, frame, zoom.flag_at, clip)
}

/// **`Sprite_TopIt`'s second town arm — the mercenary marker.**
///
/// `(tile.flags & 0x40) != 0` and `(tile.part & 0xf) == 2`:
///
/// ```c
/// else if (bVar4 == 2) {
///   if (g_counties[uVar5].mercenaryOffer == 0) return;
///   local_c = 0;
///   if (g_mapZoom == 0)      { local_30 = 0x10; local_34 = -0x12; }
///   else if (g_mapZoom == 2) { local_30 = 6;    local_34 = -0x15; }
///   DAT_005c9288 = 0x81;
/// }
/// ```
///
/// It is [`draw_flag`] with a different offset and a constant frame, and it is a
/// separate function because sharing the entry
/// point is exactly how it came to be drawn at the banner's
/// [`Zoom::flag_at`] — ten pixels out, in a screen full of ten-pixel things.
/// See [`MERCENARY_MARKER_FRAME`] for the second, independent source of the
/// frame index.
pub fn draw_mercenary_marker(
    canvas: &mut Canvas,
    assets: &MapAssets,
    view: Viewport,
    zoom: &Zoom,
    tile: (usize, usize),
    clip: Clip,
) -> bool {
    let at = zoom.mercenary_at;
    blit_over_tile(canvas, assets, view, zoom, tile, MERCENARY_MARKER_FRAME, at, clip)
}

/// **`FUN_00407F82` (`0x00407F82`) — the besieger's camp mark over a besieged
/// castle, and the count of siege seasons left under it.**
///
/// [`draw_flag`] with a third offset and a constant frame, plus a number. The
/// number is the caller's to draw because it needs a font, so this returns
/// *where* it goes:
///
/// ```c
/// if (g_mapZoom != 0) return;                      /* the whole body */
/// g_drawX += dx;  g_drawY += dy;  DAT_005C9288 = 0x82;
/// g_spriteWidth = *(short *)(g_flagsSheet + 0x828);    /* frame 0x82's own width */
/// …clip, blit…
/// DAT_005AEA40 = 1;                                    /* flat, no emboss */
/// if (0x18 < y0 + dy + 10)
///   Ui_DrawNumberRight(seasonsLeft, ' ', " ", x0 + dx, y0 + dy + 10,
///                      g_spriteWidth, &g_fontBody, 0xF9);
/// DAT_005AEA40 = 0;
/// ```
///
/// Three things in that are decisions, not detail:
///
/// * **`Ui_DrawNumberRight` centres** (`docs/symbols.md`, `0x004030C6`), and the
///   width it centres in is **frame `0x82`'s own width** — `+0x828` is the
///   8-byte PL8 header plus `0x82 * 0x10`, landing on the width field at `+0`
///   of that frame's record (`docs/formats/pl8.md`), which is also what fixes
///   the frame index at `0x82` from a second direction. The count sits under
///   the middle of the mark whatever the artwork is; in the shipped
/// `Flags1a.pl8` that frame is **24 × 28 and the last of 131**.
/// * **The suffix is one space.** `DAT_004D2094` is `20 00 00 00`, read out of
///   `.data` at file offset `0xD0294`, and `FUN_004025D7` measures it — the
///   trailing space is inside the centring, not after it.
/// * **`0x18` is the menu bar.** The blit is clipped; the number is not, and
///   this test is all that keeps it off the bar.
///
/// Returns `(x, y, width)` for the count, or `None` when nothing was drawn —
/// the far zoom, a missing sheet, or a mark so high the number would land on
/// the menu bar.
pub fn besieger_marker(
    canvas: &mut Canvas,
    assets: &MapAssets,
    view: Viewport,
    zoom: &Zoom,
    tile: (usize, usize),
    clip: Clip,
) -> Option<(i32, i32, i32)> {
    if zoom.id != NEAR.id {
        return None;
    }
    let at = zoom.besieger_at;
    let decoded = assets.flag_sheet(zoom)?.frame(BESIEGER_MARKER_FRAME)?;
    let width = i32::from(decoded.width);
    let (row, col) = tile_to_cell(tile.0, tile.1);
    let (sx, sy) = cell_to_screen(view, zoom, row, col);
    canvas.blit_clipped(&decoded, sx + at.0, sy + at.1, clip);
    let y = sy + at.1 + BESIEGER_COUNT_DY;
    (BESIEGER_COUNT_TOP < y).then_some((sx + at.0, y, width))
}

/// The body both town arms share: `g_flagsSheet`, a frame, and an offset added
/// to the tile origin with no centring.
fn blit_over_tile(
    canvas: &mut Canvas,
    assets: &MapAssets,
    view: Viewport,
    zoom: &Zoom,
    tile: (usize, usize),
    frame: usize,
    at: (i32, i32),
    clip: Clip,
) -> bool {
    let Some(sheet) = assets.flag_sheet(zoom) else { return false };
    let Some(decoded) = sheet.frame(frame) else { return false };
    let (row, col) = tile_to_cell(tile.0, tile.1);
    let (sx, sy) = cell_to_screen(view, zoom, row, col);
    canvas.blit_clipped(&decoded, sx + at.0, sy + at.1, clip);
    true
}

/// **`Map_DrawPathMarker` (`0x004081A6`) — one ball of an ordered path.**
///
/// `Path_MarkPreviewTiles` (`0x004A91BA`) sets bank bit `0x40` on each step of
/// the local player's path, and this draws `g_flagsSheet` on every tile
/// carrying it. `docs/decisions.md` C49 is what this function is: for four
/// documents it was `Map_DrawCountyFlag` and was said to draw a castle's flag
/// at a frame derived from the castle level. Every clause of that was wrong,
/// and the frame arithmetic those documents quoted was **this** one's.
///
/// **The placement is the original's, and it used to be ours.** This centred
/// the frame on the tile and said the original's offset was not traced. It is
/// two literals in the function itself:
///
/// ```c
/// local_18 = 0x14;
/// if (mode == 1) local_18 = 0x14 - g_mapTileHalfStep;   /* the same place, from a */
///                                                       /* draw cursor half a tile on */
/// g_drawX = g_drawX + local_18;
/// g_drawY = g_drawY + 6;
/// ```
///
/// — [`PATH_MARKER_AT`] from the tile origin, **with no centring and no zoom
/// test**, blitted the way [`draw_flag`] blits: the frame record's `cx`/`cy`
/// are never read. The centred placement was two pixels right and two down of
/// it on the 15 × 15 ball.
///
/// Returns false when the sheet or the frame is missing, so the caller can fall
/// back to a mark of its own.
pub fn draw_path_marker(
    canvas: &mut Canvas,
    assets: &MapAssets,
    view: Viewport,
    zoom: &Zoom,
    tile: (usize, usize),
    frame: usize,
    clip: Clip,
) -> bool {
    blit_over_tile(canvas, assets, view, zoom, tile, frame, PATH_MARKER_AT, clip)
}

/// `Map_DrawPathMarker`'s frame for a tile the path reaches at accumulated cost
/// `n`, with `in_range` false for a step past the remaining budget.
///
/// The clamp at the top of the ramp is ours: the original indexes
/// `0x38 + n` with no bound, and a path costing more than 21 would walk off the
/// end of the bank into `0x4E`, the castle marker. Ours stops at
/// [`PATH_MARKER_LAST`]. That is a divergence and it is a deliberate one — see
/// `docs/bugs.md`.
///
/// `action` is `local_14`, and it is tested **first**: a tile a click would act
/// on draws [`PATH_MARKER_ACTION`] whatever it costs and whether or not it is
/// in range.
pub fn path_marker_frame(cost: i32, in_range: bool, action: bool) -> usize {
    if action {
        return PATH_MARKER_ACTION;
    }
    if !in_range || cost <= 0 {
        return PATH_MARKER_FIRST;
    }
    (PATH_MARKER_FIRST + cost as usize).min(PATH_MARKER_LAST)
}

pub fn flag_frame(shield: u8, phase: u8) -> Option<usize> {
    (1..=5).contains(&shield).then(|| {
        (shield as usize - 1) * FLAG_PHASES as usize + (phase % FLAG_PHASES) as usize
    })
}

/// The sprite for a pasture tile: `(frame, offset)`, or `None` for a field
/// state that draws no animals.
///
/// `phase` is the animation phase, `0 … 5`; see [`HERD_PHASES`].
pub fn herd_sprite(terrain: u8, phase: u8) -> Option<(usize, (i32, i32))> {
    let phase = (phase % HERD_PHASES) as usize;
    match terrain {
        // The reachable herds: three crowding bands.
        0x14..=0x16 => Some((
            (terrain - 0x14) as usize * HERD_PHASES as usize + phase + HERD_FIRST_FRAME,
            HERD_AT,
        )),
        // Dead in the shipped game; kept because the ladder is.
        0x10..=0x12 => Some((
            (terrain - 0x10) as usize * HERD_PHASES as usize + phase + HERD_DEAD_FIRST_FRAME,
            HERD_DEAD_AT,
        )),
        // `0x0F` and `0x13` are pasture and draw nothing at all.
        _ => None,
    }
}

/// Blit one pasture's herd. Returns false when the sheet or the frame is
/// missing, so a caller can fall back to a mark of its own.
///
/// **`Zoom::id == 2` draws nothing**, which is the arm's own first line rather
/// than a simplification: the far zoom's tiles are 10 × 6 and the animals are
/// not drawn there.
///
/// The original's `param_1` — `if (param_1 == 1) dx -= g_mapTileHalfStep` — is
/// **not** reproduced, and its absence is the point. `FUN_00405FAC` walks an
/// offset row with `g_drawX` starting at `g_mapViewX` unshifted and adds the
/// half step only after the first cell, so `param_1 == 1` marks the
/// half-clipped left-edge tile and the subtraction puts its overlay back where
/// the diamond is. [`cell_to_screen`] shifts *every* cell of an odd row by
/// `half_pitch` instead, so the two agree cell for cell — the first at
/// `viewX − half`, the second at `viewX + half`, the third at
/// `viewX + half + pitch`. [`draw_flag`] rests on the same identity. **[D]**
pub fn draw_herd(
    canvas: &mut Canvas,
    assets: &MapAssets,
    view: Viewport,
    zoom: &Zoom,
    tile: (usize, usize),
    terrain: u8,
    phase: u8,
    clip: Clip,
) -> bool {
    if zoom.id == 2 {
        return false;
    }
    let Some((frame, (dx, dy))) = herd_sprite(terrain, phase) else { return false };
    let Some(sheet) = assets.flag_sheet(zoom) else { return false };
    let Some(decoded) = sheet.frame(frame) else { return false };
    let (row, col) = tile_to_cell(tile.0, tile.1);
    let (sx, sy) = cell_to_screen(view, zoom, row, col);
    canvas.blit_clipped(&decoded, sx + dx, sy + dy, clip);
    true
}

