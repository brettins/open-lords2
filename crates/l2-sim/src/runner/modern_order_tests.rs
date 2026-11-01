//! C272, the fix round: what a Modern order does and does not touch, against `BattleUnit_Order`
//! (0x00479E90) and the AI's own calls. Each test names the line whose removal turns it red.

use super::modern_tests::*;
use super::*;

/// Ablate `min_figures` (3 for a melee unit): a two-man foe is an enemy unit to the Modern unit.
/// `Enemy_NearestUnit(unit, 9, 3)`, 00480000.c:4498.
#[test]
fn a_modern_melee_unit_ignores_a_foe_of_two_men_and_charges_one_of_three() {
    for (men, charges) in [(2u16, false), (3, true)] {
        let mut r = duel(&[(Troop::Swordsmen, 4)], &[(Troop::Peasants, men)]);
        let (mine, foe) = (unit_of_troop(&r, Troop::Swordsmen), unit_of_troop(&r, Troop::Peasants));
        put(&mut r, mine, 20, 40);
        put(&mut r, foe, 27, 40);
        announce(&mut r);
        r.step();
        assert_eq!(r.units.get(mine).halted, charges, "{men} men");
    }
}

/// Ablate: put `self.reform_unit(unit)` back at the end of `run_verb` and the men have their slots
/// at once instead of when `reTarg` (0x14) runs out.
#[test]
fn an_order_reforms_the_unit_when_its_retarg_runs_out_not_at_once() {
    let mut r = duel(&[(Troop::Swordsmen, 4)], &[(Troop::Peasants, 1)]);
    let mine = unit_of_troop(&r, Troop::Swordsmen);
    put(&mut r, mine, 20, 40);
    r.command(ME, &[mine as u8], Verb::Move, (40, 40), None, false);
    let stood = |r: &BattleRunner| {
        r.members(mine).into_iter().all(|m| r.fighters[m].target == (r.fighters[m].x, r.fighters[m].y))
    };
    assert!(stood(&r), "BattleUnit_Order sets reTarg, it does not reform");
    r.run(5);
    assert!(stood(&r), "five frames in, reTarg has not run out");
    r.run(20);
    assert!(!stood(&r), "BattleUnit_Reform sent the men");
}

/// Ablate `woodland` in `run_verb` (a literal false, as it was): the bows get no fire arrow.
/// `Battle_OrderSelection` 00430000.c:7318 passes `DAT_0053E874`; `BattleUnit_Order` writes `+0x30`.
#[test]
fn a_missile_unit_ordered_onto_woodland_gets_its_fire_arrow_cell() {
    for (surface, armed) in [(15u8, true), (0, false)] {
        // The fire arrow is a side-0 unit's (`BattleUnit_Order`), the second army's.
        let mut r = BattleRunner::deploy_armies(
            blank_field(),
            DEFAULT_SEED,
            Army { troops: &[(Troop::Peasants, 1)], owner: FOE, human: true },
            Army { troops: &[(Troop::Archers, 4)], owner: ME, human: true },
        );
        r.settle();
        let mine = unit_of_troop(&r, Troop::Archers);
        put(&mut r, mine, 20, 40);
        r.field.cells[40 * DIM + 30].surface = surface;
        r.command(ME, &[mine as u8], Verb::Move, (30, 40), None, false);
        assert_eq!(r.units.get(mine).target_cell != 0, armed, "surface {surface}");
    }
}

/// Ablate the `if ...engaged` guard on clearing `halted` in `run_verb`: the Charge button's charge
/// is undone by a Move.
#[test]
fn a_move_undoes_the_sweeps_charge_but_not_the_charge_buttons() {
    let mut r = duel(&[(Troop::Swordsmen, 4)], &[(Troop::Peasants, 4)]);
    let (mine, foe) = (unit_of_troop(&r, Troop::Swordsmen), unit_of_troop(&r, Troop::Peasants));
    put(&mut r, mine, 20, 40);
    put(&mut r, foe, 60, 40);
    announce(&mut r);
    r.charge_all(ME);
    assert!(r.units.get(mine).halted);
    r.command(ME, &[mine as u8], Verb::Move, (20, 41), None, false);
    assert!(r.units.get(mine).halted, "FUN_0047A76D's one shot stays");

    let mut swept = duel(&[(Troop::Swordsmen, 4)], &[(Troop::Peasants, 4)]);
    put(&mut swept, mine, 20, 40);
    put(&mut swept, foe, 27, 40);
    announce(&mut swept);
    swept.step();
    assert!(swept.modern.mode(mine).engaged && swept.units.get(mine).halted, "the sweep charged it");
    swept.command(ME, &[mine as u8], Verb::Move, (20, 41), None, false);
    assert!(!swept.units.get(mine).halted, "a new order ends the sweep's own charge");
}
