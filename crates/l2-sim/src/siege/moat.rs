#![allow(unused_imports)]
use super::*;
use super::castle::*;
use super::damage::*;
use super::drawbridge::*;
use super::siegetower::*;
use crate::terrain::{Battlefield, Cell, DIM};

/// **One moat cell is filled in** — `FUN_0047DD86` (`0x0047DD86`), reached from
/// `BattleMan_StateFillMoat` once a figure has tipped [`MOAT_FILL_STEPS`] loads
/// into it.
///
/// ```c
/// surface = 1; flags = 0; frame &= 0x0F;
/// for each of the four orthogonal neighbours:
///     if its surface is 3, 5 or 4  ->  g_siegeApproachScore += 1
/// DAT_0057A0D8 += 1;
/// ```
pub fn fill_moat_cell(field: &mut Battlefield, state: &mut SiegeState, cell: usize) -> i32 {
    {
        let c = &mut field.cells[cell];
        c.surface = SURFACE_FILLED;
        c.flags = 0;
        c.gfx &= 0x0F;
        c.terrain = crate::terrain::id::OPEN;
    }
    let mut score = 0;
    for n in orthogonal_neighbours(cell) {
        if matches!(
            field.cells[n].surface,
            SURFACE_GROUND | SURFACE_BAILEY | SURFACE_RAMPART_WALK
        ) {
            score += 1;
        }
    }
    state.moat_filled = state.moat_filled.saturating_add(1);
    score
}

