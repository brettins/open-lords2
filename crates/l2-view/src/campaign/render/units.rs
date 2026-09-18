#![allow(unused_imports)]
use super::*;
use super::markers::*;
use super::*;
use l2_formats::maps::{MapSlot, Plane, LATTICE_H, LATTICE_W, PLANE_DIM};
use crate::canvas::{Canvas, Clip, Tags};
use crate::sheet::Sheet;

/// **`Map_DrawArmies` (`0x00408438`) — one unit's figure standing on a tile.**
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
    let Some((x, y, decoded)) = unit_sprite_rect(assets, view, zoom, tile, sprite) else {
        return false;
    };
    canvas.blit_clipped(&decoded, x, y, clip);
    true
}

/// The same arithmetic, factored out so that a **hit test** can ask where the
/// figure is. A
/// sprite is anchored on the tile's bottom vertex and is taller than the tile,
/// so most of it stands over the tiles behind — see `docs/decisions.md` C57 and C58.
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
    Some((
        ax + wx + nx - decoded.width as i32 / 2,
        ay + wy + ny - decoded.height as i32,
        decoded,
    ))
}

/// **`Map_DrawArmies`' `mode` parameter (`0x00408438`)** — and the two pixels
/// the leftmost column of every offset row is drawn to the left of its tile.
///
/// **`-2` is read out of that branch, not chosen**, and so is which cell gets
/// it. The army pass is `FUN_00405487` (`0x00405487`), which walks the lattice
/// through two row functions: `FUN_00405862` (`0x00405862`), the aligned row,
/// passes **mode 0** for every column, while `FUN_004059AF` (`0x004059AF`),
/// the offset row, opens with
pub fn unit_anchor(view: Viewport, zoom: &Zoom, tile: (usize, usize)) -> (i32, i32) {
    let (row, col) = tile_to_cell(tile.0, tile.1);
    let (sx, sy) = cell_to_screen(view, zoom, row, col);
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
/// * **The unit's `x`/`y` are already the destination.** `Unit_StepOnce`
///   (`0x0046634D`) commits the tile — `+0x149 = 1; Unit_MoveInFacing();` — so
///   every entry drags the figure *back* toward the tile it left. Index 1 is the
///   whole step; indices 15 and 0 are zero in all of them.
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

