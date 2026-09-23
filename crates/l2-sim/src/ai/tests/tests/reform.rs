#![allow(unused_imports)]
use super::*;
use super::thinking::*;
use super::marching::*;
use super::siege::*;
use super::super::*;
use crate::figure::{SIDE_A, SIDE_B};
use crate::unit::CATEGORY_OF_TROOP;

#[test]
fn a_siege_attacker_on_foot_charges_outright_above_a_hundred_and_fifty_one() {
    let outcome = |advantage: i32| {
        let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 2);
        fx.ai.is_siege = true;
        fx.ai.multiplayer = true;
        fx.ai.approach_score = 10;
        fx.ai.breach_score = 5;
        let u = fx.units.create(3, false, SIDE_B, 2).unwrap();
        let mut f = Figure::new(Troop::Pikemen, SIDE_B, 4);
        f.unit = u as u16;
        f.owner = 3;
        fx.figures.push(f);
        fx.positions.push((30, 55));
        for _ in 0..3 {
            fx.think_at(advantage);
        }
        fx.ai.last_action[u]
    };
    assert_ne!(outcome(SIEGE_CHARGE_ADVANTAGE - 1), Action::Charge);
    assert_eq!(outcome(SIEGE_CHARGE_ADVANTAGE), Action::Charge);
}

#[test]
fn the_castle_layout_flag_jumps_the_approach_script_to_a_hundred() {
    let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 2);
    fx.ai.is_siege = true;
    fx.ai.multiplayer = true;
    fx.field.castle_layout_flag = true;
    let u = fx.units.create(3, false, SIDE_B, 2).unwrap();
    let mut f = Figure::new(Troop::Pikemen, SIDE_B, 4);
    f.unit = u as u16;
    f.owner = 3;
    fx.figures.push(f);
    fx.positions.push((30, 55));
    for _ in 0..19 {
        fx.frames(200);
    }
    assert_eq!(fx.units.get(u).orders, 101, "jumped to 100, then incremented");
}


#[test]
fn the_five_hundred_frame_countdown_reforms_and_runs_for_human_units_too() {
    let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 4);
    fx.ai.multiplayer = true;
    let mut who = Vec::new();
    for _ in 0..500 {
        who = fx.frame();
    }
    assert_eq!(who, vec![fx.ai_unit, fx.human_unit], "both sides reform");
    assert_eq!(fx.units.get(fx.human_unit).reform, REFORM_INTERVAL);
}

#[test]
fn a_small_human_unit_is_exempt_and_so_is_one_that_has_charged() {
    let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 3);
    fx.ai.multiplayer = true;
    let mut who = Vec::new();
    for _ in 0..500 {
        who = fx.frame();
    }
    assert!(
        !who.contains(&fx.human_unit),
        "three figures and human-controlled: exempt"
    );
    assert!(who.contains(&fx.ai_unit), "the AI's unit of three is not");

    let mut charged = *fx.units.get(fx.ai_unit);
    assert!(needs_reform(&charged));
    charged.halted = true;
    assert!(!needs_reform(&charged));
    charged.halted = false;
    charged.figures = 0;
    assert!(!needs_reform(&charged));
}


#[test]
fn two_ai_battles_from_the_same_seed_stay_identical() {
    let build = || Fixture::new(Troop::Swordsmen, Troop::Archers, 6);
    let (mut a, mut b) = (build(), build());
    for _ in 0..3_000 {
        a.frame();
        b.frame();
        assert_eq!(a.units, b.units, "units diverged");
        assert_eq!(a.ai, b.ai, "ai state diverged");
        assert_eq!(a.figures, b.figures, "figures diverged");
    }
    assert_ne!(a.ai.rng, Ai::new(0x5EED).rng, "the generator never advanced");
}

