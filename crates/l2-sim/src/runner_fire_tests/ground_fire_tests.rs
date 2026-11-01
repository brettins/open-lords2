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

/// `Battle_UpdateAllMen` (`0x004822ED`, `00480000.c:873`, `:888-890`) counts a human on
/// woodland whatever its state: a corpse in the wood counts until `BattleMan_Destroy`
/// frees it (`owner == 0`), and not after.
///
/// Ablation: put `&& self.sim.figures[sim].is_alive()` back in `update_man` and the
/// count reads 3.
#[test]
fn a_human_corpse_in_the_wood_counts_until_it_is_freed() {
    let mut field = blank_field();
    for y in 40..=44 {
        for x in 37..=43 {
            field.cells[cell(x, y)].surface = SURFACE_WOODLAND;
        }
    }
    let mut r = BattleRunner::empty(field, DEFAULT_SEED);
    let men: Vec<usize> =
        (0..5).map(|k| stand(&mut r, Troop::Peasants, SIDE_B, 1, true, (38 + k, 42))).collect();
    let sim = |r: &BattleRunner, k: usize| r.fighters[men[k]].sim;
    let corpse = sim(&r, 3);
    r.sim.figures[corpse].state = State::Dead;
    r.sim.figures[corpse].men = 0;
    let freed = sim(&r, 4);
    r.sim.figures[freed].state = State::Dead;
    r.sim.figures[freed].men = 0;
    r.sim.figures[freed].owner = 0;
    r.humans_in_woods = 0;
    for i in 0..r.fighters.len() {
        r.update_man(i);
    }
    assert_eq!(r.humans_in_woods, 4, "three standing and one corpse; the freed record is skipped");
}
