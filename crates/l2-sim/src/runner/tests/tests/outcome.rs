#![allow(unused_imports)]
use super::*;
use super::deploy::*;
use super::ai_runs::*;
use super::missiles::*;
use super::super::*;
use crate::ai::Action;

#[test]
fn a_battle_with_both_armies_standing_has_not_concluded() {
    let r = small_battle();
    assert_eq!(r.conclusion(), None);
    assert_eq!(r.men_of_side(SIDE_A), 40, "ten figures of four");
    assert_eq!(r.men_of_side(SIDE_B), 40);
}

#[test]
fn a_side_with_no_men_left_has_lost_and_the_other_holds_the_field() {
    let mut r = small_battle();
    for f in &mut r.sim.figures {
        if f.side == SIDE_A {
            f.men = 0;
            f.state = State::Dead;
        }
    }
    assert_eq!(r.men_of_side(SIDE_A), 0);
    assert_eq!(
        r.conclusion(),
        Some(Conclusion { winner: SIDE_B, cause: End::Annihilation })
    );
}

#[test]
fn a_battle_that_kills_everyone_at_once_falls_to_the_side_tested_first() {
    let mut r = small_battle();
    for f in &mut r.sim.figures {
        f.men = 0;
        f.state = State::Dead;
    }
    assert_eq!(r.conclusion().unwrap().winner, SIDE_A, "army B holds an empty field");
}

/// Withdrawal outranks annihilation: the original tests `DAT_0056D5C8`
/// before it looks at either men counter,
/// withdrawn loses even if the enemy is the one that was wiped out.
#[test]
fn a_withdrawal_decides_the_battle_before_the_men_are_counted() {
    let mut r = small_battle();
    for f in &mut r.sim.figures {
        if f.side == SIDE_A {
            f.men = 0;
            f.state = State::Dead;
        }
    }
    r.withdraw(SIDE_B);
    let c = r.conclusion().unwrap();
    assert_eq!(c.cause, End::Withdrawal);
    assert_eq!(c.winner, SIDE_A, "the side that left the field loses whatever the count says");
}

#[test]
fn siege_engines_do_not_count_towards_a_sides_men() {
    let mut r = BattleRunner::deploy_muster(
        blank_field(),
        DEFAULT_SEED,
        Muster { troops: &[(Troop::Catapults, 2)], owner: 1, human: false, mercenaries: None },
        Muster { troops: &[(Troop::Peasants, 40)], owner: 2, human: true, mercenaries: None },
    );
    assert!(r.sim.figures.iter().any(|f| f.troop == Troop::Catapults), "they were raised");
    assert_eq!(r.men_of_side(SIDE_B), 0, "and they are worth nobody");
    assert_eq!(r.conclusion().unwrap().winner, SIDE_A);
    r.withdraw(SIDE_A);
    assert_eq!(r.conclusion().unwrap().winner, SIDE_B);
}

#[test]
fn the_last_figure_of_a_unit_carries_the_remainder() {
    let a = [(Troop::Peasants, 306u32)];
    let b = [(Troop::Peasants, 200u32)];
    let r = BattleRunner::deploy_muster(
        blank_field(),
        DEFAULT_SEED,
        Muster { troops: &a, owner: 1, human: false, mercenaries: None },
        Muster { troops: &b, owner: 2, human: true, mercenaries: None },
    );
    assert_eq!(r.men_per_figure(SIDE_B), 8);
    assert_eq!(r.men(SIDE_B), 306, "not 312, which 39 full figures of eight would give");
    assert_eq!(r.survivors(SIDE_B)[Troop::Peasants.index()], 306);
    assert_eq!(r.sim.figures.iter().filter(|f| f.side == SIDE_B).count(), 39);
}

#[test]
fn a_small_side_beside_a_large_one_gets_its_own_finer_scale() {
    let a = [(Troop::Peasants, 1000u32)];
    let b = [(Troop::Peasants, 40u32)];
    let r = BattleRunner::deploy_muster(
        blank_field(),
        DEFAULT_SEED,
        Muster { troops: &a, owner: 1, human: false, mercenaries: None },
        Muster { troops: &b, owner: 2, human: true, mercenaries: None },
    );
    assert_eq!(size_class(1040), 2);
    assert_eq!(r.men_per_figure(SIDE_B), 16, "the large side keeps the pair's scale");
    assert_eq!(r.men_per_figure(SIDE_A), 8, "and the small one halves it");
    assert_eq!(r.men(SIDE_A), 40, "nobody lost either way");
    assert_eq!(r.men(SIDE_B), 1000);
}

#[test]
fn the_raise_order_is_the_binarys_and_not_the_callers() {
    let a = [(Troop::Peasants, 40u32), (Troop::Knights, 40)];
    let b = [(Troop::Peasants, 40u32)];
    let r = BattleRunner::deploy_muster(
        blank_field(),
        DEFAULT_SEED,
        Muster { troops: &a, owner: 1, human: false, mercenaries: None },
        Muster { troops: &b, owner: 2, human: true, mercenaries: None },
    );
    let first = r.sim.figures.iter().find(|f| f.side == SIDE_B).unwrap();
    assert_eq!(first.troop, Troop::Knights, "knights are raised before peasants");
}

