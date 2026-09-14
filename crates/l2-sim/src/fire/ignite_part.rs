#![allow(unused_imports)]
use super::*;
use super::woodland::*;
use super::burn_part::*;
use super::tests_part::*;
use crate::figure::{Figure, State};
use crate::missile::{Missile, Missiles, CLASS_FIRE, CLASS_OIL, SUB_CELL};
use crate::terrain::{Battlefield, DIM};

/// **Set one cell burning** — `FUN_00485675` (`0x00485675`).
///
/// ```c
/// Missile_Spawn(1, x, y, x, y);  class = 5;  x -= 0x10;  y -= 0x18;
/// +0x3C = 0x280 - param_3;  +0x38 = 20000;  +0x2F = 6;
/// +0x3E = cell.surface;  cell.surface = 10;
/// if (+0x3E == 7) { cell.flags = 0; cell.elevation = 0; flags2 = …; cell.frame = 0; }
/// ```
///
/// A bridge that catches is **flattened and cleared on the spot**: elevation 0,
/// every flag gone. So a burning bridge is passable ground at the height of the
/// water, for as long as it burns and after.
///
/// **The slot is not checked**, and that is the original's defect
/// ours: `Missile_Spawn` returns 0 with a hundred records in flight, and the
/// writes land on a hundred-and-first record past the array. The cell's own
/// writes still happen — so **a cell set alight with the array full burns for
/// the rest of the battle**, with no record to put it out. Reproduced for the
/// cell; the overrun itself is not behaviour (`docs/bugs.md` N4's reasoning)
/// and is not reproduced. `docs/bugs.md` `B102`.
///
/// Returns the cell, for the caller to re-derive what it keeps from the field.
pub fn ignite(field: &mut Battlefield, missiles: &mut Missiles, x: i32, y: i32, param: i16) -> usize {
    let cell = y as usize * DIM + x as usize;
    let saved = field.cells[cell].surface;
    if let Some(slot) = missiles.alloc() {
        *missiles.get_mut(slot) = fire_record(x, y, FIRE_LIFE.wrapping_sub(param), saved);
    }
    let c = &mut field.cells[cell];
    c.surface = SURFACE_BURNING;
    if saved == SURFACE_BRIDGE {
        c.flags = 0;
        c.elevation = 0;
        c.gfx = 0;
    }
    cell
}

/// **Set one woodland cell catching** — `FUN_00485861` (`0x00485861`).
///
/// Unlike [`ignite`] this one **does** check the slot, and does nothing at all
/// without one. The life is `0x280 − 10 × ((x + y) & 0x1F`)
/// out in a diagonal stripe pattern over 330 to 640 frames
/// once, and the remembered surface is always woodland.
///
/// Returns the cell when it caught.
pub fn ignite_woodland(field: &mut Battlefield, missiles: &mut Missiles, x: i32, y: i32) -> Option<usize> {
    let slot = missiles.alloc()?;
    let cell = y as usize * DIM + x as usize;
    let life = FIRE_LIFE - 10 * (((x + y) & 0x1F) as i16);
    *missiles.get_mut(slot) = fire_record(x, y, life, SURFACE_WOODLAND);
    field.cells[cell].surface = SURFACE_WOOD_CATCHING;
    Some(cell)
}

/// **A fire goes out** — `Missile_UpdateAll`'s class-5 arm, on the frame its
/// countdown reads [`FIRE_RESTORE_AT`].
///
/// ```c
/// if (+0x3E == 7)    { surface = 5; flags = 0; elevation = 0; frame = 0; }
/// else if (+0x3E == 0x0F) { surface = 0; flags = 0; frame = 0; }
/// else               { surface = +0x3E; }
/// ```
///
/// So what burns away is exactly **a bridge** — left as surface 5 at the
/// height of the water — and **a wood**, left as surface 0, which no zone of
/// the battlefield classifier ever writes. Everything else comes back.
pub fn put_out(field: &mut Battlefield, fire: &Missile) -> usize {
    let cell = fire.cell_y as usize * DIM + fire.cell_x as usize;
    let c = &mut field.cells[cell];
    match fire.saved_surface {
        SURFACE_BRIDGE => {
            c.surface = SURFACE_BURNT_BRIDGE;
            c.flags = 0;
            c.elevation = 0;
            c.gfx = 0;
        }
        SURFACE_WOODLAND => {
            c.surface = 0;
            c.flags = 0;
            c.gfx = 0;
        }
        s => c.surface = s,
    }
    cell
}

/// **A bridge goes up** — `FUN_0048551D` (`0x0048551D`), less the
/// `Sound_PlayFile("dest_ind.wav", 0, 0)` it opens with, which is the caller's
/// cue.
///
/// ```c
/// FUN_00485675(x, y, 0x78);
/// c = 0;
/// for (r = 1; r < 6; r++)
///   for each cell of the (2r+1) square round (x, y), rows top to bottom:
///     if (Cell_NeighbourHasSurface(cell, 10)) {
///       if (cell.surface == 7) { FUN_00485675(cell, 0x78 - c); c += 7; if (c > 0x78) c = 0x78; }
///       else if (cell.surface == 5) FUN_00485B47(cell);     /* scorch the ground */
///     }
/// ```
///
/// **The fire walks along the bridge, and only along the bridge**, one ring at
/// a time for five rings: a bridge cell beside something burning catches, and
/// so does the one beside *it* on the next ring — so a straight bridge burns
/// five cells either way of where it caught and no further. **Anything burning
/// counts as a neighbour**, oil included. A cell of surface 5 beside the fire
/// is only scorched: `FUN_00485B47` masks its frame to the low nibble and
/// touches no rule.
///
/// The original walks the square with no bound at the field's edge
/// within five of it reads cells of the neighbouring row. That is not
/// behaviour and is not reproduced. Returns every cell it wrote.
pub fn bridge_fire(field: &mut Battlefield, missiles: &mut Missiles, x: i32, y: i32) -> Vec<usize> {
    let mut touched = vec![ignite(field, missiles, x, y, BRIDGE_FIRE_PARAM)];
    let mut c: i16 = 0;
    for r in 1..=BRIDGE_SPREAD_RADIUS {
        for cy in (y - r)..=(y + r) {
            for cx in (x - r)..=(x + r) {
                if !in_field(cx, cy) || !neighbour_has_surface(field, cx, cy, SURFACE_BURNING) {
                    continue;
                }
                let cell = cy as usize * DIM + cx as usize;
                match field.cells[cell].surface {
                    SURFACE_BRIDGE => {
                        touched.push(ignite(field, missiles, cx, cy, BRIDGE_FIRE_PARAM - c));
                        c = (c + BRIDGE_SPREAD_STEP).min(BRIDGE_FIRE_PARAM);
                    }
                    // Surface 5 — the bailey, and a bridge that has already
                    // burnt out, which is written as the same value.
                    crate::siege::SURFACE_BAILEY => {
                        field.cells[cell].gfx &= 0x0F;
                        touched.push(cell);
                    }
                    _ => {}
                }
            }
        }
    }
    touched
}

/// **The wood fire, one frame of it** — `FUN_004859E5` (`0x004859E5`), which
/// `Battle_Frame` runs after the unit sweep.
///
/// ```c
/// if (0 < DAT_0053E9D0) {
///     DAT_0053E9D0 = 0;
///     for every cell: if (surface == 0x10) { surface = 0x11; DAT_0053E9D0 = 1; }
///     for every cell: if (surface == 0x0F && Cell_NeighbourHasSurface(cell, 0x11)) FUN_00485861(cell);
/// }
/// ```
///
/// **One ring a frame, through the whole of a connected wood**, and it stops
/// only when a frame turns nothing from catching to burning. The second pass
/// lights cells beside `0x11` and writes `0x10`
/// cannot light its own neighbour until the next frame — the ring is exact.
/// `spreading` is `DAT_0053E9D0`. Returns every cell it wrote.
pub fn spread_woodland(field: &mut Battlefield, missiles: &mut Missiles, spreading: &mut bool) -> Vec<usize> {
    let mut touched = Vec::new();
    if !*spreading {
        return touched;
    }
    *spreading = false;
    for (cell, c) in field.cells.iter_mut().enumerate() {
        if c.surface == SURFACE_WOOD_CATCHING {
            c.surface = SURFACE_WOOD_BURNING;
            *spreading = true;
            touched.push(cell);
        }
    }
    for y in 0..DIM as i32 {
        for x in 0..DIM as i32 {
            let cell = y as usize * DIM + x as usize;
            if field.cells[cell].surface == SURFACE_WOODLAND
                && neighbour_has_surface(field, x, y, SURFACE_WOOD_BURNING)
            {
                if let Some(c) = ignite_woodland(field, missiles, x, y) {
                    touched.push(c);
                }
            }
        }
    }
    touched
}

