#![allow(unused_imports)]
use super::*;
use super::castle::*;
use super::damage::*;
use super::moat::*;
use super::siegetower::*;
use crate::terrain::{Battlefield, Cell, DIM};

/// **Lower the drawbridge** — `FUN_00496B9F` (`0x00496B9F`), the whole of it.
///
/// The garrison's fifth verb, and the one the battlefield's third button
/// exists for. `FUN_0043BBE7` guards it four ways — a siege, the local player
/// owning army B, `g_castleLevel >= 3`, and the latch — and then this happens:
///
/// ```c
/// scan row-major for the first cell with flags & 0x40
/// for 7 rows: for 4 cells:
///     flags = 0; surface = 3; frame = DAT_004D9E18[i];
///     flags2 |= 1; flags2 &= 0xE3;
/// _DAT_00569588 = 1;                  /* the gate is open */
/// DAT_0052AF9C  = 1;                  /* and it stays open */
/// g_siegeApproachScore += 4;
/// g_siegeBreachScore   += 4;
/// Path_BuildTerrainTemplate(); Path_BuildElevation();
/// ```
///
/// * **It is not siege-engine placement.** The hand-off this was built from
///   said it was. The game names it itself: the button's two refusals are
///   `L2.eng` 111 *"No drawbridge!"* and 157 *"Drawbridge is down."*, the
/// guard is level 3 and up, and the shipped `Readme.txt` says *"only the
///   Stone and Royal castles have drawbridges."* Four sources, one verb.
///
///   `C79`.
///
/// > `docs/bugs.md` `B85`. `[V]` on the control flow, `[I]`
/// > that it is harmless.
pub fn lower_drawbridge(field: &mut Battlefield, state: &mut SiegeState) -> Option<usize> {
    if state.drawbridge_down {
        return None;
    }
    let anchor = field.cells.iter().position(|c| c.flags & FLAG_DRAWBRIDGE != 0)?;
    let (ax, ay) = (anchor % DIM, anchor / DIM);
    for row in 0..DRAWBRIDGE_ROWS {
        for col in 0..DRAWBRIDGE_COLS {
            let (x, y) = (ax + col, ay + row);
            if x >= DIM || y >= DIM {
                continue;
            }
            let c = &mut field.cells[y * DIM + x];
            c.flags = 0;
            c.surface = SURFACE_GROUND;
            c.gfx = DRAWBRIDGE_FRAMES[row * DRAWBRIDGE_COLS + col];
        }
    }
    state.drawbridge_down = true;
    state.gate_breached = true;
    Some(anchor)
}

