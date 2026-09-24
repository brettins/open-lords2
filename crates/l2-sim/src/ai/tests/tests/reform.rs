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
    // The AI unit's own order at its think resets its countdown to 0x14 (0x00479E90).
    let mut who = Vec::new();
    for _ in 0..500 {
        who.extend(fx.frame());
    }
    assert!(who.contains(&fx.ai_unit) && who.contains(&fx.human_unit), "both sides reform");
    assert_eq!(fx.units.get(fx.human_unit).reform, REFORM_INTERVAL);
}

#[test]
fn a_small_human_unit_is_exempt_and_so_is_one_that_has_charged() {
    let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 3);
    fx.ai.multiplayer = true;
    let mut who = Vec::new();
    for _ in 0..500 {
        who.extend(fx.frame());
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

/// `BattleUnit_Order` (`0x00479E90`) writes `reTarg = 0x14`: an AI order reaches the figures at
/// the reform twenty frames on (`Battle_UpdateAllUnits` `0x00489401`), and never once halted.
#[test]
fn an_ai_order_reforms_its_unit_twenty_frames_later_unless_it_has_charged() {
    let run = |halted: bool| {
        let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 4);
        fx.ai.multiplayer = true;
        let unit = fx.ai_unit;
        fx.units.get_mut(unit).halted = halted;
        let mut world = World {
            units: &mut fx.units,
            figures: &mut fx.figures,
            positions: &fx.positions,
            field: &fx.field,
            ai: &mut fx.ai,
        };
        world.to_rally_waypoint(unit, 0);
        assert!(fx.units.get(unit).reform_gate, "`field_0x13 = 1`");
        (1..=40).find(|_| fx.frame().contains(&unit))
    };
    assert_eq!(run(false), Some(20));
    assert_eq!(run(true), None);
}


/// `BattleUnit_Order` ends in `BattleUnits_RebuildFromFigures()` (00470000.c:4412): an AI order
/// ages every unit's `orderLock` (00480000.c:3522). Ablation: `rebuild_from_figures` in `Units::order`.
#[test]
fn an_ai_order_ages_every_units_order_lock() {
    let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 4);
    let (unit, other) = (fx.ai_unit, fx.human_unit);
    fx.units.get_mut(other).order_lock = 10;
    let mut world = World {
        units: &mut fx.units,
        figures: &mut fx.figures,
        positions: &fx.positions,
        field: &fx.field,
        ai: &mut fx.ai,
    };
    world.set_target(unit, 40, 45);
    world.set_target(unit, 40, 46);
    assert_eq!(fx.units.get(other).order_lock, 8);
    assert_eq!(fx.ai.orders, vec![(unit, 40, 45), (unit, 40, 46)]);
}
