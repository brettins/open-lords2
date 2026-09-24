#![allow(unused_imports)]
use super::*;
use super::thinking::*;
use super::siege::*;
use super::reform::*;
use super::super::*;
use crate::figure::{SIDE_A, SIDE_B};
use crate::unit::CATEGORY_OF_TROOP;

#[test]
fn an_aggressive_foot_unit_marches_for_ten_thinks_and_then_charges() {
    let mut fx = Fixture::new(Troop::Peasants, Troop::Peasants, 4);
    let actions = fx.think_at_n(50, 12); // comfortably above 5
    assert_eq!(actions[0], Action::DoNothing, "the first two thinks are silent");
    assert_eq!(actions[1], Action::DoNothing);
    assert!(
        matches!(actions[2], Action::ToEnemyEnd(_)),
        "think 2 starts the march: {:?}",
        actions[2]
    );
    assert!(matches!(actions[9], Action::ToEnemyEnd(_)), "still marching at think 9");
    assert_eq!(actions[10], Action::Charge, "think 10 charges");
    assert!(fx.units.get(fx.ai_unit).halted, "and the unit is halted");
    assert!(fx
        .figures
        .iter()
        .filter(|f| f.unit as usize == fx.ai_unit)
        .all(|f| f.state == State::Chasing));
}

#[test]
fn a_melee_unit_marches_thirteen_thinks_rather_than_ten() {
    let mut fx = Fixture::new(Troop::Knights, Troop::Peasants, 4);
    let actions = fx.think_at_n(50, 15);
    assert_eq!(actions[3], Action::DoNothing, "silent through think 3");
    assert!(matches!(actions[4], Action::ToEnemyEnd(_)));
    assert!(matches!(actions[12], Action::ToEnemyEnd(_)), "still marching at 12");
    assert_eq!(actions[13], Action::Charge);
}

#[test]
fn five_is_the_line_between_marching_on_the_enemy_and_falling_back() {
    let run = |advantage: i32| {
        let mut fx = Fixture::new(Troop::Peasants, Troop::Peasants, 4);
        fx.think_at_n(advantage, 6)
    };
    let bold = run(AGGRESSION_THRESHOLD + 1);
    let timid = run(AGGRESSION_THRESHOLD);
    assert!(
        bold.iter().any(|a| matches!(a, Action::ToEnemyEnd(_))),
        "at 6 it marches on the enemy: {bold:?}"
    );
    assert!(
        !timid.iter().any(|a| matches!(a, Action::ToEnemyEnd(_))),
        "at exactly 5 it does not: {timid:?}"
    );
    assert!(
        timid.iter().any(|a| matches!(a, Action::ToRallyWaypoint(_))),
        "it falls back on a rally waypoint instead: {timid:?}"
    );
}

#[test]
fn foot_and_melee_units_rally_on_different_waypoints() {
    let waypoint = |troop: Troop| {
        let mut fx = Fixture::new(troop, Troop::Peasants, 4);
        for _ in 0..40 {
            if let Action::ToRallyWaypoint(k) = fx.think_at(0) {
                return Some(k);
            }
        }
        None
    };
    assert_eq!(waypoint(Troop::Peasants), Some(2));
    assert_eq!(waypoint(Troop::Knights), Some(1));
}

#[test]
fn a_worn_down_enemy_unit_becomes_invisible_to_the_melee_ai() {
    let mut fx = Fixture::new(Troop::Peasants, Troop::Peasants, 4);
    let human: Vec<usize> = fx
        .figures
        .iter()
        .enumerate()
        .filter(|(_, f)| f.owner == 2)
        .map(|(i, _)| i)
        .collect();
    for i in &human {
        fx.positions[*i] = (41, 50);
    }
    assert_eq!(fx.think_at(0), Action::Charge, "four figures at one cell is a target");

    let mut worn = Fixture::new(Troop::Peasants, Troop::Peasants, 4);
    for i in &human {
        worn.positions[*i] = (41, 50);
    }
    let doomed: Vec<usize> = worn
        .figures
        .iter()
        .enumerate()
        .filter(|(_, f)| f.owner == 2)
        .map(|(i, _)| i)
        .take(2)
        .collect();
    for &i in &doomed {
        while worn.figures[i].is_alive() {
            worn.figures[i].take_hits(1_000);
        }
    }
    // BattleUnits_RebuildFromFigures 00480000.c:3533: corpses count until the slot is freed
    assert_eq!(worn.think_at(0), Action::Charge, "corpses still count");
    for &i in &doomed {
        worn.figures[i].owner = 0;
    }
    assert_ne!(worn.think_at(0), Action::Charge, "two figures is not a target");
}

#[test]
fn a_melee_unit_under_attack_calls_the_archers_onto_its_attacker() {
    let mut fx = Fixture::new(Troop::Peasants, Troop::Peasants, 4);
    fx.ai.engagement_budget = 100; // stay out of the two escalation branches
    fx.ai.men_missile = 100;
    fx.ai.men_total = 100;
    fx.ai.multiplayer = true;

    for _ in 0..(THINK_INTERVAL - 10) {
        fx.frame();
    }
    fx.figures[0].was_hit = true;
    fx.figures[0].hit_by = Some(4);
    fx.ai.strength_advantage = 0;
    for _ in 0..10 {
        fx.frame();
    }

    assert_eq!(fx.action(), Action::StepAwayFromUnit(fx.human_unit));
    assert!(fx.ai.rally_request, "and the request went out");
    assert_eq!((fx.ai.rally_x, fx.ai.rally_y), (41, 30));
    assert!(fx.units.get(fx.ai_unit).withdrawing);
    assert_eq!(fx.units.get(fx.ai_unit).withdrawals, 1);
}

#[test]
fn the_commit_counter_makes_every_melee_unit_charge_regardless() {
    let mut fx = Fixture::new(Troop::Peasants, Troop::Peasants, 4);
    assert_ne!(fx.think_at(0), Action::Charge, "nothing in reach");

    fx.ai.commit_counter = 2;
    assert_eq!(fx.think_at(0), Action::Charge, "the counter overrides distance");
    assert_eq!(fx.ai.commit_counter, 1, "and it decays by one per unengaged think");
}

#[test]
fn the_two_escalations_raise_the_counter_by_three_and_by_twenty() {
    let escalate = |budget: i32, missile: i32| {
        let mut fx = Fixture::new(Troop::Swordsmen, Troop::Peasants, 4);
        fx.ai.multiplayer = true;
        fx.ai.engagement_budget = budget;
        fx.ai.men_total = 800;
        fx.ai.men_missile = missile;
        for _ in 0..(THINK_INTERVAL - 10) {
            fx.frame();
        }
        fx.figures[0].was_hit = true;
        fx.figures[0].hit_by = Some(4);
        fx.ai.strength_advantage = 0;
        for _ in 0..10 {
            fx.frame();
        }
        fx.ai.commit_counter
    };
    assert_eq!(escalate(0, 800), 3, "over the engagement budget");
    assert_eq!(escalate(100, 0), 20, "no archers left");
}

#[test]
fn a_winning_archer_unit_never_backs_away_however_often_it_is_hit() {
    let hit_ten_times = |advantage: i32| {
        let mut fx = Fixture::new(Troop::Archers, Troop::Peasants, 4);
        fx.ai.multiplayer = true;
        for _ in 0..(THINK_INTERVAL - 20) {
            fx.frame();
        }
        for _ in 0..12 {
            fx.figures[0].was_hit = true;
            fx.figures[0].hit_by = Some(4);
            fx.frame();
        }
        assert!(fx.units.get(fx.ai_unit).times_hit > 10);
        fx.ai.strength_advantage = advantage;
        for _ in 0..8 {
            fx.frame();
        }
        fx.action()
    };
    assert_eq!(
        hit_ten_times(0),
        Action::StepAwayFromUnit(2),
        "the cautious archer withdraws"
    );
    assert_ne!(
        hit_ten_times(50),
        Action::StepAwayFromUnit(2),
        "the confident one does not"
    );
}

