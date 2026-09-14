#![allow(unused_imports)]
use super::*;
use l2_formats::maps::{MapSlot, Plane, LATTICE_H, LATTICE_W, PLANE_DIM};
use crate::canvas::{Canvas, Clip, Tags};
use crate::sheet::Sheet;

/// **`Map_DrawArmies` (`0x00408438`) — one unit's figure standing on a tile.**
///
/// The original's placement, statement for statement:
///
/// ```c
/// x = walkTableX[dir * 16 + stepAccum] + g_mapTileHalfStep;   /* 30 / 6 */
/// y = walkTableY[dir * 16 + stepAccum] + g_mapHalfPitch;      /* 30 / 6 */
/// switch (kind) { case 1: case 2: y -= 4; break;
///                 case 3: case 4: y -= 2; x -= 4; break; }
/// drawX += x;  drawY += y;
/// drawX -= spriteWidth / 2;  drawY -= spriteHeight;
/// ```
///
/// `g_mapTileHalfStep` and `g_mapHalfPitch` are **both** 30 at the near zoom and
/// both 6 at the far one — `Map_SetZoom` writes them from the same literal — and
/// the near tile is 58 × 30 and the far one 10 × 6, so the anchor is the
/// diamond's **bottom vertex, one pixel right of centre**, and the figure hangs
/// upwards from it. That is why a unit reads as standing *on* the tile rather
/// than floating in it.
///
/// **The walk table is applied here**, as [`UnitSprite::walk`] — see
/// [`walk_offset`]. The unit's own tile is the one it is walking *into*, and the
/// offset drags the figure back toward the tile it left while `+0x149` counts
/// across.
///
/// This paragraph used to say the opposite: that the tables had nothing to
/// index with, and that *"a unit mid-step would sit at its destination tile in
/// the original for the same reason it does here"*. The first half stopped being
/// true when `docs/decisions.md` C134 gave the unit its sub-tile counter, and
/// the second half — the original's tile is the destination and
/// its figure is not on it. A player: *"The army marching animation is jumping
/// from square to square, I remember there being an animation and some
/// interpolation between walking squares."*
///
/// Returns false when the sheet or the frame is missing, so the caller can fall
/// back to a marker of its own.
#[allow(clippy::too_many_arguments)]
pub fn draw_unit(
    canvas: &mut Canvas,
    assets: &MapAssets,
    view: Viewport,
    zoom: &Zoom,
    tile: (usize, usize),
    sprite: UnitSprite,
    clip: Clip,
) -> bool {
    // One copy of the placement arithmetic, shared with the hit test.
    let Some((x, y, decoded)) = unit_sprite_rect(assets, view, zoom, tile, sprite) else {
        return false;
    };
    canvas.blit_clipped(&decoded, x, y, clip);
    true
}

/// Where [`draw_unit`] would put a unit's figure, and the frame it would use.
///
/// The same arithmetic, factored out so that a **hit test** can ask where the
/// figure is. A
/// sprite is anchored on the tile's bottom vertex and is taller than the tile,
/// so most of it stands over the tiles behind — see `docs/decisions.md` C57 and C58.
/// `None` when the sheet or the frame is missing, which is a caller's cue to
/// fall back on whatever it draws instead of the figure.
pub fn unit_sprite_rect(
    assets: &MapAssets,
    view: Viewport,
    zoom: &Zoom,
    tile: (usize, usize),
    sprite: UnitSprite,
) -> Option<(i32, i32, l2_formats::pl8::DecodedFrame)> {
    let decoded = assets.sprite_sheet(zoom, sprite.sheet)?.frame(sprite.frame)?;
    let (ax, ay) = unit_anchor(view, zoom, tile);
    let (nx, ny) = sprite.nudge;
    let (wx, wy) = sprite.walk;
    // Then the kind's nudge, then `x -= w/2; y -= h`.
    Some((
        ax + wx + nx - decoded.width as i32 / 2,
        ay + wy + ny - decoded.height as i32,
        decoded,
    ))
}

/// **`Map_DrawArmies`' `mode` parameter (`0x00408438`)** — and the two pixels
/// the leftmost column of every offset row is drawn to the left of its tile.
///
/// The painter adds a constant to the figure's x before centring, and which
/// constant is the argument its caller passed:
///
/// ```c
/// if      (mode == 1) local_1c = local_1c + -2;
/// else if (mode == 2) local_1c = local_1c + g_mapTileHalfStep + -2;   /* dead */
/// else                local_1c = local_1c + g_mapTileHalfStep;
/// local_20 = local_20 + g_mapHalfPitch;
/// ```
///
/// **`-2` is read out of that branch, not chosen**, and so is which cell gets
/// it. The army pass is `FUN_00405487` (`0x00405487`), which walks the lattice
/// through two row functions: `FUN_00405862` (`0x00405862`), the aligned row,
/// passes **mode 0** for every column, while `FUN_004059AF` (`0x004059AF`),
/// the offset row, opens with
///
/// ```c
/// g_drawX = g_mapViewX;
/// g_tileCursor = lattice[g_mapStartCol][g_latticeRow];
/// if (...) Map_DrawArmies(1);             /* the leftmost column, and only it */
/// g_drawX = g_drawX + g_mapTileHalfStep;  /* …every other column is mode 0 */
/// ```
///
/// so that one figure lands on `g_mapViewX - 2` where the row's own spacing
/// puts it on `g_mapViewX`. **`mode == 2` has no caller in the corpus** — the
/// only literals reaching `Map_DrawArmies` anywhere are 0 and that single 1 —
/// so the middle branch is written down here.
///
/// It is a *viewport*-relative quirk, not a property of the tile: scroll one
/// column and a different army is the one that shifts. `docs/bugs.md` has no
/// entry against it, and reproducing the painter is reproducing this.
///
/// Returns the anchor the sprite is centred on; the walk offset, the kind's
/// nudge and the `w/2` / `h` subtraction are [`unit_sprite_rect`]'s.
pub fn unit_anchor(view: Viewport, zoom: &Zoom, tile: (usize, usize)) -> (i32, i32) {
    let (row, col) = tile_to_cell(tile.0, tile.1);
    let (sx, sy) = cell_to_screen(view, zoom, row, col);
    // `Map_RenderIso` starts on an even lattice row and `Viewport::clamped`
    // keeps `view.row` even, so an odd row is an offset row.
    let head_of_offset_row = row & 1 == 1 && col == view.col;
    let x = if head_of_offset_row { sx + zoom.half_pitch - 2 } else { sx + zoom.half_pitch };
    (x, sy + zoom.half_pitch)
}

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

/// **`Map_DrawArmies`' walk tables** (`0x00408438`) — where a unit part-way
/// across a tile is drawn, as an offset from where it would stand at rest on the
/// tile it is walking *into*.
///
/// ```c
/// dir = unit.facing - g_mapRotation;  if (dir < 0) dir += 8;
/// if      (g_mapZoom == 0) { x = DAT_004d8108[dir * 16 + unit.field_0x149];
///                            y = DAT_004d8188[dir * 16 + unit.field_0x149]; }
/// else if (g_mapZoom == 1) { /* 0x004d8208 / 0x004d8288 — the dead zoom */ }
/// else if (g_mapZoom == 2) { x = DAT_004d8308[...];  y = DAT_004d8388[...]; }
/// ```
///
/// Six 8 × 16 `i8` tables, carried byte for byte out of `Lords2.exe` — the four
/// below, generated from the file, and asserted against the
/// user's own copy in `tests/install.rs`. The middle pair belongs to the zoom
/// `Map_SetZoom` can never reach (`docs/screens.md` §2.2) and is not carried.
///
/// Four things about them, each of which interpolating would get wrong:
///
/// * **The unit's `x`/`y` are already the destination.** `Unit_StepOnce`
///   (`0x0046634D`) commits the tile — `+0x149 = 1; Unit_MoveInFacing();` — so
///   every entry drags the figure *back* toward the tile it left. Index 1 is the
///   whole step; indices 15 and 0 are zero in all of them.
/// * **A walking unit is only ever drawn at the odd indices.** The commit writes
///   1 and each admitted tick adds 2, and on reaching 16 the same tick commits
/// the next tile. So a crossing is drawn at 1, 3 … 15 — eight positions, one
///   per admission, in a straight line — and only a unit that has stopped is
///   ever at 0.
/// * **The whole step is short of the tile it left.** Index 1 is `(−28, +14)`
///   for a step that is `(30, 15)` on screen and `−56` for one that is 60; at
///   the far zoom `−5` and `−11` against 6 and 12. The figure hops two pixels
///   (four on a straight step, one far out) at every commit. That is the
///   original's, and it is kept.
/// * **The direction is the unit's facing less the map rotation.** Only rotation
///   0 is built (`docs/screens.md` §7), so it is the facing — `Dir_FromDelta`'s
///   numbering, 0 north clockwise to 7 north-west.
///
/// An index above 15 is not a state `Unit_StepOnce` can leave, and is drawn at
/// rest. **[V]** on the values — two
/// sources, the bytes and the projection, in `walk_tests` below; **[D]** on
/// which indices are drawn.
pub fn walk_offset(zoom: &Zoom, facing: u8, sub_tile: u8) -> (i32, i32) {
    let (xs, ys) = if zoom.id == FAR.id { (&WALK_FAR_X, &WALK_FAR_Y) } else { (&WALK_NEAR_X, &WALK_NEAR_Y) };
    let dir = usize::from(facing & 7);
    let at = usize::from(sub_tile);
    match (xs[dir].get(at), ys[dir].get(at)) {
        (Some(&x), Some(&y)) => (i32::from(x), i32::from(y)),
        _ => (0, 0),
    }
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
/// `crates/l2-view/tests/install.rs` against the user's own file. **[V]**
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

/// **The waving flag's frame** — `shield * 8 - 8 + phase`, i.e.
/// `(shield − 1) * 8 + phase`.
///
/// `Flags1a.pl8`'s first 40 frames are 32 × 24 and lie on the artist's sheet as
/// five rows of eight: **five shields × eight wave phases**, and `shield = 5,
/// phase = 7` lands on frame 39, the last of them. The colour is in the frame
/// index; `shield` is the realm's `shieldIndex`,
/// clamped 1 … 5 by the original, so a zero shield has no flag.
pub const FLAG_PHASES: u8 = 8;

pub fn flag_frame(shield: u8, phase: u8) -> Option<usize> {
    (1..=5).contains(&shield).then(|| {
        (shield as usize - 1) * FLAG_PHASES as usize + (phase % FLAG_PHASES) as usize
    })
}

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

