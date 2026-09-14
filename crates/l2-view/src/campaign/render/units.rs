#![allow(unused_imports)]
use super::*;
use super::markers::*;
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

