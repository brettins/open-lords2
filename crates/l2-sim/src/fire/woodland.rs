#![allow(unused_imports)]
use super::*;
use super::ignite_part::*;
use super::burn_part::*;
use super::tests_part::*;
use crate::figure::{Figure, State};
use crate::missile::{Missile, Missiles, CLASS_FIRE, CLASS_OIL, SUB_CELL};
use crate::terrain::{Battlefield, DIM};

/// `Cell_NeighbourHasSurface` (`0x00496F72`): north, east, south, west, each
/// clipped at the field's edge.
pub fn neighbour_has_surface(field: &Battlefield, x: i32, y: i32, surface: u8) -> bool {
    let at = |x: i32, y: i32| field.cells[y as usize * DIM + x as usize].surface;
    (y >= 1 && at(x, y - 1) == surface)
        || (x < DIM as i32 - 1 && at(x + 1, y) == surface)
        || (y < DIM as i32 - 1 && at(x, y + 1) == surface)
        || (x >= 1 && at(x - 1, y) == surface)
}

/// **A stream of boiling oil, as `FUN_0047A814` (`0x0047A814`) leaves the
/// record** before its four launch steps.
///
/// ```c
/// Missile_Spawn(pot.owner, pot.mapX, pot.mapY, x, y);
/// +0x06 = pot;  class = 7;  sprite = 0;  +0x32 = +0x31 = 0x10;
/// power = 0;  range = 0x10;  +0x2F = 4;  blockedTicks = 0;  blocked = 0;
/// ```
///
/// **Zero power, and no class test in `Missile_Step` that could hurt a man with
/// it**: oil kills by the fire it leaves, not by landing. Its `+0x3C` is left at
/// the zeroed slot's 0, which is what lets it set a bridge alight as it crosses.
pub fn oil_record(owner: u8, pot: usize, from: (u8, u8), to: (u8, u8)) -> Missile {
    let mut m = Missile {
        owner,
        class: CLASS_OIL,
        shooter: pot as u16,
        x: from.0 as i16 * SUB_CELL,
        y: from.1 as i16 * SUB_CELL,
        target_x: to.0 as i16 * SUB_CELL,
        target_y: to.1 as i16 * SUB_CELL,
        cell_x: from.0 as i16,
        cell_y: from.1 as i16,
        dir: crate::facing::facing_from_delta(
            to.0 as i32 - from.0 as i32,
            to.1 as i32 - from.1 as i32,
        )
        .unwrap_or(8),
        launch_elevation: OIL_LAUNCH_ELEVATION,
        blocked_ticks: 0,
        sub_steps: OIL_SUB_STEPS,
        range_ticks: OIL_RANGE_TICKS,
        ..Missile::default()
    };
    m.setup_line();
    m
}

/// The facing `FUN_0047A814` turns a pot to as it pours: the axis the pour
/// travels further along, with **x winning a tie** —
/// `if (absDx < absDy) { y < ty ? 4 : 0 } else { x < tx ? 2 : 6 }`.
pub fn pour_facing(from: (u8, u8), to: (u8, u8)) -> u8 {
    let (dx, dy) = (to.0 as i32 - from.0 as i32, to.1 as i32 - from.1 as i32);
    if dx.abs() < dy.abs() {
        if from.1 < to.1 {
            4
        } else {
            0
        }
    } else if from.0 < to.0 {
        2
    } else {
        6
    }
}

/// Where a pot of oil **pours when its unit is ordered**, from `BattleUnit_Order`
/// (`0x00479E90`)'s oil loop: the pot's own surface against the destination's.
///
/// ```c
/// if      (pot == 6 && dest < 6) FUN_0047a814(pot, x, y);   /* from the keep     */
/// else if (pot == 4 && dest < 4) FUN_0047a814(pot, x, y);   /* from the rampart  */
/// else if (pot == 5 && dest < 4) FUN_0047a814(pot, x, y);   /* from the bailey   */
/// ```
///
/// So an order *down* pours and an order *along* moves: a pot on the rampart
/// walk told to go to another stretch of walk walks there, and told to go to
/// the field below tips its oil where it stands. **An order is a pour, not a
/// walk to a place to pour from.**
pub fn order_pours(pot_surface: u8, dest_surface: u8) -> bool {
    matches!((pot_surface, dest_surface), (6, d) if d < 6)
        || matches!((pot_surface, dest_surface), (4, d) if d < 4)
        || matches!((pot_surface, dest_surface), (5, d) if d < 4)
}

