//! `BattleMan_StateMelee` (`0x004831D8`) and `BattleMan_StateChase` (`0x00483E55`):
//! the duel's survivor pursues, and the pursuit ends with its target.

use crate::battle::Battle;
use crate::figure::{State, SIDE_A, SIDE_B};
use crate::runner::*;
use crate::Troop;

/// `00480000.c:1387-1401`: opponent dead → `state = 8`.
/// Ablation: `melee.rs`'s `State::Chasing` back to `State::Idle` fails this.
#[test]
fn the_survivor_of_a_duel_goes_on_to_pursue() {
    let mut bt = Battle::new();
    bt.add(Troop::Knights, SIDE_A, 1).unwrap();
    bt.add(Troop::Peasants, SIDE_B, 1).unwrap();
    bt.engage(0, 1);
    bt.run(2000);
    let (dead, alive) = if bt.figures[0].is_alive() { (1, 0) } else { (0, 1) };
    assert_eq!(bt.figures[dead].state, State::Dead);
    assert_eq!(bt.figures[alive].state, State::Chasing, "state 8, not 5");
}

/// `00480000.c:1668-1673`: `if (target.state == 2) target = 0; if (target == 0)
/// {Anim_Stand(); state = 5;}` — a dead target is not re-chosen.
/// Ablation: re-choosing on a dead target (the old `!alive` arm) fails this.
#[test]
fn a_chase_ends_when_its_target_dies() {
    let mut r = BattleRunner::deploy(blank_field(), &[(Troop::Swordsmen, 1)], &[(Troop::Pikemen, 2)]);
    let me = (0..r.fighters.len()).find(|&i| r.fighters[i].troop == Troop::Swordsmen).unwrap();
    let foes: Vec<usize> =
        (0..r.fighters.len()).filter(|&i| r.fighters[i].troop == Troop::Pikemen).collect();
    let (sim, dead) = (r.fighters[me].sim, r.fighters[foes[0]].sim);
    r.sim.figures[dead].state = State::Dead;
    r.sim.figures[sim].state = State::Chasing;
    r.sim.figures[sim].target = Some(dead);
    r.retarget(me);
    assert_eq!(r.sim.figures[sim].state, State::Idle);
    assert_eq!(r.sim.figures[sim].target, None);
}

/// `BattleUnit_JoinMelee` (`0x0048F079`, `00480000.c:6340`): `withdrawing = 0`.
/// Ablation: delete that line in `combat/melee.rs`.
#[test]
fn contact_calls_off_a_withdrawal() {
    let mut r = BattleRunner::deploy(blank_field(), &[(Troop::Swordsmen, 1)], &[(Troop::Pikemen, 1)]);
    let i = (0..r.fighters.len()).find(|&i| r.fighters[i].troop == Troop::Pikemen).unwrap();
    let unit = r.unit_of(i);
    r.units.get_mut(unit).human = false;
    r.units.get_mut(unit).withdrawing = true;
    r.join_melee(unit);
    assert!(!r.units.get(unit).withdrawing);
}
