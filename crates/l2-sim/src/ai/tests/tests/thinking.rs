#![allow(unused_imports)]
use super::*;
use super::marching::*;
use super::siege::*;
use super::reform::*;
use super::super::*;
use crate::figure::{SIDE_A, SIDE_B};
use crate::unit::CATEGORY_OF_TROOP;

#[test]
fn a_unit_thinks_once_every_two_hundred_frames_and_not_before() {
    let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 4);
    fx.ai.multiplayer = true;
    for _ in 0..199 {
        fx.frame();
        assert_eq!(fx.action(), Action::NoThink, "thought early");
        assert_eq!(fx.units.get(fx.ai_unit).orders, 0);
    }
    fx.frame();
    assert_ne!(fx.action(), Action::NoThink, "should have thought on frame 200");
    assert_eq!(fx.units.get(fx.ai_unit).orders, 1, "one think, one script step");
    for _ in 0..199 {
        fx.frame();
        assert_eq!(fx.units.get(fx.ai_unit).orders, 1, "and not again until 400");
    }
    fx.frame();
    assert_eq!(fx.units.get(fx.ai_unit).orders, 2);
}

#[test]
fn a_unit_in_melee_stops_thinking_entirely() {
    let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 4);
    fx.ai.multiplayer = true;
    fx.figures[0].state = State::Melee;
    for _ in 0..1_000 {
        fx.frame();
        assert_eq!(fx.action(), Action::NoThink);
    }
    assert_eq!(fx.units.get(fx.ai_unit).orders, 0, "five thinks' worth of frames, none taken");
    fx.figures[0].state = State::Idle;
    fx.frames(200);
    assert!(fx.units.get(fx.ai_unit).orders > 0);
}

#[test]
fn four_handlers_do_not_gate_on_melee() {
    for (troop, side) in [
        (Troop::Catapults, SIDE_B),
        (Troop::SiegeTowers, SIDE_B),
        (Troop::BatteringRams, SIDE_B),
        (Troop::Pikemen, SIDE_A),
    ] {
        let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 2);
        fx.ai.is_siege = true;
        fx.ai.multiplayer = true;
        let u = fx
            .units
            .create(3, false, side, CATEGORY_OF_TROOP[troop.index()])
            .unwrap();
        let mut f = Figure::new(troop, side, 4);
        f.unit = u as u16;
        f.owner = 3;
        f.state = State::Melee;
        fx.figures.push(f);
        fx.positions.push((30, 40));
        fx.frames(250);
        assert!(
            fx.units.get(u).orders > 0,
            "{troop:?} should keep thinking in melee"
        );
    }
}

#[test]
fn the_two_fast_siege_defenders_think_every_hundred_frames() {
    let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 2);
    fx.ai.is_siege = true;
    fx.ai.multiplayer = true;
    let foot = fx.units.create(3, false, SIDE_A, 2).unwrap();
    let melee = fx.units.create(3, false, SIDE_A, 3).unwrap();
    for (i, u) in [foot, melee].into_iter().enumerate() {
        let mut f = Figure::new(Troop::Pikemen, SIDE_A, 4);
        f.unit = u as u16;
        f.owner = 3;
        fx.figures.push(f);
        fx.positions.push((30 + i as u8, 40));
    }
    fx.frames(100);
    assert_eq!(fx.units.get(foot).orders, 1);
    assert_eq!(fx.units.get(melee).orders, 1);
    assert_eq!(fx.units.get(fx.ai_unit).orders, 0);
}

#[test]
fn a_human_controlled_unit_is_never_given_an_order() {
    let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 4);
    fx.ai.multiplayer = true;
    fx.frames(2_000);
    assert_eq!(fx.units.get(fx.human_unit).orders, 0);
    assert_eq!(fx.ai.last_action[fx.human_unit], Action::NoThink);
    assert!(fx.units.get(fx.ai_unit).orders > 0, "but the AI's did");
}


#[test]
fn the_strength_advantage_is_weighted_ai_men_over_weighted_human_men() {
    let mut fx = Fixture::new(Troop::Knights, Troop::Peasants, 4);
    fx.ai.multiplayer = true;
    fx.ai.update_strength_advantage(&fx.figures);
    assert_eq!(fx.ai.strength_advantage, 300);

    let mut even = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 4);
    even.ai.multiplayer = true;
    even.ai.update_strength_advantage(&even.figures);
    assert_eq!(even.ai.strength_advantage, 0, "identical armies are even");
}

#[test]
fn an_unopposed_army_reads_minus_one_hundred() {
    let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 4);
    fx.ai.multiplayer = true;
    for f in fx.figures.iter_mut().filter(|f| f.owner_is_human) {
        f.take_hits(60_000);
    }
    fx.ai.update_strength_advantage(&fx.figures);
    assert_eq!(fx.ai.strength_advantage, -100);
}

#[test]
fn the_jitter_spans_minus_ten_to_plus_twenty_one() {
    let mut ai = Ai::new(0x1234_5678);
    let figures: Vec<Figure> = Vec::new();
    let (mut low, mut high) = (i32::MAX, i32::MIN);
    let mut seen = [false; 32];
    for _ in 0..5_000 {
        ai.update_strength_advantage(&figures);
        let j = ai.strength_advantage + 100;
        low = low.min(j);
        high = high.max(j);
        seen[(j + 10) as usize] = true;
    }
    assert_eq!(low, -10);
    assert_eq!(high, 21);
    assert!(seen.iter().all(|s| *s), "every one of the 32 values should occur");
}

#[test]
fn the_same_seed_gives_the_same_jitter_and_a_different_seed_does_not() {
    let roll = |seed: u64| {
        let mut ai = Ai::new(seed);
        let figures: Vec<Figure> = Vec::new();
        (0..50)
            .map(|_| {
                ai.update_strength_advantage(&figures);
                ai.strength_advantage
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(roll(7), roll(7));
    assert_ne!(roll(7), roll(8));
}

#[test]
fn the_multiplayer_flag_removes_the_jitter_and_the_draw() {
    let mut ai = Ai::new(99);
    ai.multiplayer = true;
    let before = ai.rng.clone();
    let figures: Vec<Figure> = Vec::new();
    for _ in 0..20 {
        ai.update_strength_advantage(&figures);
        assert_eq!(ai.strength_advantage, -100, "no jitter at all");
    }
    assert_eq!(ai.rng, before, "and the generator was not advanced");
}

/// `Battle_Start` (`0x004778A0`, 00470000.c:3063-3072): a network game
/// advances the pair instead of drawing it, lane wrapping at 3 and group
/// at 1. Ablation: draw in the `multiplayer` arm and the cycle breaks.
#[test]
fn a_network_battle_steps_the_lane_and_the_group_instead_of_drawing_them() {
    let mut prev = (0, 0);
    let seen: Vec<(usize, usize)> = (0..5)
        .map(|_| {
            let ai = Ai::new_for_battle(99, true, prev);
            prev = (ai.approach_lane, ai.rally_group);
            prev
        })
        .collect();
    assert_eq!(seen, vec![(1, 1), (2, 0), (3, 1), (0, 0), (1, 1)]);
    let drawn = Ai::new_for_battle(99, false, (0, 0));
    assert_eq!(drawn.approach_lane & 1, drawn.rally_group, "one value, masked twice");
}

#[test]
fn the_advantage_is_recomputed_every_hundred_and_first_frame() {
    let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 4);
    let start = fx.ai.rng.clone();
    for _ in 0..100 {
        fx.frame();
    }
    assert_eq!(fx.ai.rng, start, "not yet");
    fx.frame();
    assert_ne!(fx.ai.rng, start, "the hundred-and-first frame draws");
}


