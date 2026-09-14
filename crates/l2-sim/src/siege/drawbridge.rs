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
/// # Three things this is not
///
/// * **It is not siege-engine placement.** The hand-off this was built from
///   said it was. The game names it itself: the button's two refusals are
///   `L2.eng` 111 *"No drawbridge!"* and 157 *"Drawbridge is down."*, the
/// guard is level 3 and up, and the shipped `Readme.txt` says *"only the
///   Stone and Royal castles have drawbridges."* Four sources, one verb.
///   `C79`.
/// * It sets the *same*
///   two globals a twenty-thousandth ram hit sets — `_DAT_00569588` and both
///   scores by 4 — so as far as every AI order handler is concerned **the
///   garrison has opened its own gate**. That is the price of a sally, and it
///   is why the Readme says a drawbridge cannot be closed again.
/// * The latch is set
/// *inside* the `if`, so a level-3 castle whose layout happens to carry no
///   `0x40` cell leaves the button live. Reproduced.
///
/// > **The original's scan has a missing `break`.** `bVar1 = true; break;`
/// > leaves only the inner loop, and the outer one then re-tests the same cell
/// > for every remaining row, breaking immediately each time. The *answer* is
/// > unaffected — the offset stops at the first `0x40` cell either way — but
/// > `g_foundTileX` / `g_foundTileY` are left at `(0, 0x50)`.
/// > cell, which is a battlefield-wide scratch pair other routines read.
/// > Nothing was found that reads them between here and their next write.
/// > `docs/bugs.md` `B85`. `[V]` on the control flow, `[I]`
/// > that it is harmless.
///
/// Returns the anchor cell when it fired, so a caller can rebuild whatever it
/// derives from the field. `flags2` — cell byte `+2` — is **not** written:
/// [`Cell`] does not carry it, because nothing in this engine reads it.
pub fn lower_drawbridge(field: &mut Battlefield, state: &mut SiegeState) -> Option<usize> {
    if state.drawbridge_down {
        return None;
    }
    let anchor = field.cells.iter().position(|c| c.flags & FLAG_DRAWBRIDGE != 0)?;
    let (ax, ay) = (anchor % DIM, anchor / DIM);
    for row in 0..DRAWBRIDGE_ROWS {
        for col in 0..DRAWBRIDGE_COLS {
            let (x, y) = (ax + col, ay + row);
            // The original walks a flat offset with no bound check at all and
            // would run off the end of the array; we stop at the edge instead.
            // `docs/bugs.md` N4's reasoning: an overrun is not behaviour.
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

