//! `BattleMan_StateMelee` (`0x004831D8`) and `BattleMan_StateChase` (`0x00483E55`):
//! the duel's survivor pursues, and the pursuit ends with its target.

use crate::battle::Battle;
use crate::figure::{State, SIDE_A, SIDE_B};
use crate::runner::*;
use crate::melee;
use crate::Troop;

fn duel() -> Battle {
    let mut bt = Battle::new();
    bt.add(Troop::Macemen, SIDE_A, 1).unwrap();
    bt.add(Troop::Peasants, SIDE_B, 1).unwrap();
    bt.engage(0, 1);
    bt
}

/// Melee_Tick 0x00494908 (00490000.c:1817-1825) tests and kills only the man whose tick it is;
/// the survivor reaches state 8 on his own tick (0x004831D8 :1390).
/// Ablation: `other.take_hits_from(heavy, idx)` for the heavy blow in `melee.rs` fails this.
#[test]
fn a_duel_is_lost_on_the_loser_s_own_tick() {
    let mut bt = duel();
    bt.figures[1].hits = bt.figures[1].hits_per_casualty - 1;
    bt.figures[0].recovery_counter = 50;
    melee::tick(&mut bt.figures, 0);
    assert!(bt.figures[1].is_alive(), "the heavy blow waits for the peasant's own tick");
    melee::tick(&mut bt.figures, 1);
    assert_eq!(bt.figures[1].state, State::Dead);
    assert_eq!(bt.figures[0].state, State::Melee, "not written inside the victim's tick");
    melee::tick(&mut bt.figures, 0);
    assert_eq!(bt.figures[0].state, State::Chasing);
}

/// 00490000.c:1795-1799 has no else: a man's own tick does not step his recovery down.
/// Ablation: restoring `else { me.recovery_counter -= 1 }` in `melee.rs` fails this.
#[test]
fn only_the_attacker_wears_down_recovery() {
    let mut bt = duel();
    bt.figures[1].recovery_counter = 5;
    melee::tick(&mut bt.figures, 1);
    assert_eq!(bt.figures[1].recovery_counter, 5);
}

/// `00480000.c:1387-1401`: opponent dead → `state = 8`.
/// Ablation: `melee.rs`'s prologue `State::Chasing` back to `State::Idle` fails this.
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

fn chase(r: &mut BattleRunner, troop: Troop) -> (usize, usize) {
    let me = (0..r.fighters.len()).find(|&i| r.fighters[i].troop == troop).unwrap();
    let sim = r.fighters[me].sim;
    r.sim.figures[sim].state = State::Chasing;
    (me, sim)
}

/// 00480000.c:1658 `if (6 < troopType) state = 0xb;`. Ablation: drop the is_siege arm.
#[test]
fn a_siege_engine_does_not_chase() {
    let mut r = BattleRunner::deploy(blank_field(), &[(Troop::Catapults, 1)], &[(Troop::Pikemen, 1)]);
    let (me, sim) = chase(&mut r, Troop::Catapults);
    r.retarget(me);
    assert_ne!(r.sim.figures[sim].state, State::Chasing);
}

/// 00480000.c:1661 `target.owner == 0` re-chooses. Ablation: test `target.is_none()` only.
#[test]
fn an_ownerless_target_is_chosen_again() {
    let mut r = BattleRunner::deploy(blank_field(), &[(Troop::Swordsmen, 1)], &[(Troop::Pikemen, 2)]);
    let (me, sim) = chase(&mut r, Troop::Swordsmen);
    let foes: Vec<usize> =
        (0..r.fighters.len()).filter(|&i| r.fighters[i].troop == Troop::Pikemen).map(|i| r.fighters[i].sim).collect();
    r.sim.figures[foes[0]].owner = 0;
    r.sim.figures[sim].target = Some(foes[0]);
    assert!(!r.retarget(me));
    assert_eq!(r.sim.figures[sim].target, Some(foes[1]));
}

/// 00480000.c:1667-1669: state 5 writes no tgX/tgY. Ablation: rewrite the target to own cell.
#[test]
fn a_dropped_chase_keeps_its_point() {
    let mut r = BattleRunner::deploy(blank_field(), &[(Troop::Swordsmen, 1)], &[(Troop::Pikemen, 1)]);
    let (me, sim) = chase(&mut r, Troop::Swordsmen);
    let foe = (0..r.fighters.len()).find(|&i| r.fighters[i].troop == Troop::Pikemen).unwrap();
    r.sim.figures[r.fighters[foe].sim].state = State::Dead;
    r.sim.figures[sim].target = Some(r.fighters[foe].sim);
    r.fighters[me].target = (40, 40);
    assert!(r.retarget(me), "state 5 returns");
    assert_eq!(r.fighters[me].target, (40, 40));
}

/// Formation_SendFigure 0x00489B8D (00480000.c:3884-3887) has no state-4 guard.
/// Ablation: `if state == State::Melee { return; }` in send_figure fails this.
#[test]
fn an_order_reaches_a_man_in_melee() {
    let mut r = BattleRunner::deploy(blank_field(), &[(Troop::Swordsmen, 1)], &[(Troop::Pikemen, 1)]);
    let me = (0..r.fighters.len()).find(|&i| r.fighters[i].troop == Troop::Swordsmen).unwrap();
    let sim = r.fighters[me].sim;
    r.sim.figures[sim].state = State::Melee;
    let unit = r.unit_of(me);
    r.send_figure(unit, me, 10, 10, false);
    assert_eq!(r.sim.figures[sim].state, State::Idle);
    assert_eq!(r.fighters[me].target, (10, 10));
}
