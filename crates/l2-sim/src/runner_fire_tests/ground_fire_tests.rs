#![allow(unused_imports)]
use super::*;
use crate::fire::{SURFACE_WOODLAND, SURFACE_WOOD_BURNING};

/// A player archer alone on the field, and an empty wood to the south.
pub(crate) fn an_archer_and_an_empty_wood() -> (BattleRunner, usize, usize) {
    let mut field = blank_field();
    for y in 40..=44 {
        for x in 37..=43 {
            field.cells[cell(x, y)].surface = SURFACE_WOODLAND;
        }
    }
    let mut r = BattleRunner::empty(field, DEFAULT_SEED);
    let archer = stand(&mut r, Troop::Archers, SIDE_A, 1, true, (40, 34));
    let unit = r.units.create(1, true, SIDE_A, 1).unwrap();
    let sim = r.fighters[archer].sim;
    r.sim.figures[sim].unit = unit as u16;
    r.settle();
    (r, unit, archer)
}

/// **Fire at ground.** `BattleUnit_Order` (`0x00479E90`, `00470000.c:4290-4292`)
/// writes `targetCell` *and* parks figure slot 0 on the clicked cell;
/// `Formation_SendFigure` (`0x00489B8D`, `:3906-3912`) sends the archer to
/// state 17 on `targetCell != 0` with **no** target; `BattleMan_StateCloseToAttack`
/// (`0x00484BF9`, `:1999`) then falls past the stand-down arm and aims at
/// `g_battleMen[0]` — the cell itself (`:2021-2024`, `:2032-2033`, `:2059-2060`).
///
/// Nobody is in the wood, so there is nothing else to fly at: without the arm
/// the archer walks and never looses.
///
/// Ablation: make `closing_aim` return `None` whenever the target figure is
/// gone (drop its `cell_of_byte_offset` arm) and the wood never lights.
#[test]
fn a_volley_ordered_at_an_empty_wood_still_lights_the_cell_it_named() {
    let (mut r, unit, archer) = an_archer_and_an_empty_wood();
    r.order_full(unit, 40, 42, None, true, Formation::Keep);
    assert_eq!(r.units.get(unit).target_cell, crate::fire::cell_byte_offset(40, 42));
    assert_eq!(r.sim.figures[r.fighters[archer].sim].target, None, "nobody to shoot at");

    let mut caught = false;
    for _ in 0..600 {
        r.step();
        if r.wood_fire {
            caught = true;
            break;
        }
    }
    assert!(caught, "the ordered volley lit an empty wood");
    let lit: Vec<usize> =
        (0..DIM * DIM).filter(|&c| r.field.cells[c].surface == SURFACE_WOOD_BURNING).collect();
    assert_eq!(lit, vec![cell(40, 42)], "only the cell the order named");
    assert_eq!(r.units.get(unit).target_cell, 0, "`00490000.c:1231` clears it");
}

/// `:1999` itself: `other.owner == 0 && targetCell == 0` is the stand-down —
/// no order, nothing in range, so the archer stays idle and looses nothing.
#[test]
fn an_archer_with_neither_target_nor_ordered_cell_stands_down() {
    let (mut r, _, archer) = an_archer_and_an_empty_wood();
    r.sim.figures[r.fighters[archer].sim].state = State::Shooting;
    for _ in 0..200 {
        r.step();
    }
    assert!(!r.wood_fire, "no targetCell, no shot");
    assert_eq!(r.sim.cues.closing_loosed(WeaponClass::Bow), 0);
}
