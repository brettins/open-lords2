#![allow(unused_imports)]
use super::*;
use super::outcome::*;
use super::deploy::*;
use super::missiles::*;
use super::super::*;
use crate::ai::Action;

#[test]
fn a_unit_thinks_once_every_two_hundred_frames_and_not_before() {
    let mut r = small_battle();
    let swords = r.units.live().next().unwrap();
    assert_eq!(r.units.get(swords).category, 3, "the swordsman unit");

    r.run(199);
    assert_eq!(r.units.get(swords).orders, 0, "no think inside the first 200 frames");
    r.step();
    assert_eq!(r.units.get(swords).orders, 1, "the 200th frame is the first think");
    assert_eq!(r.ai.last_action[swords], Action::DoNothing);

    r.run(800);
    assert_eq!(r.units.get(swords).orders, 5, "five thinks by frame 1000");
    assert!(
        matches!(r.ai.last_action[swords], Action::ToEnemyEnd(_)),
        "the fifth think should march: {:?}",
        r.ai.last_action[swords]
    );
    let marching = r
        .fighters
        .iter()
        .enumerate()
        .filter(|&(i, _)| r.unit_of(i) == swords)
        .any(|(_, f)| f.target != (f.x, f.y));
    assert!(marching, "the unit was ordered but no figure was given a destination");
}

#[test]
fn the_strength_advantage_is_recomputed_every_hundred_and_first_frame() {
    let mut r = small_battle();
    assert_eq!(r.ai.strength_advantage, 0, "nothing computed before the first frame");
    r.run(100);
    assert_eq!(r.ai.advantage_timer, 100, "counted up, not yet fired");
    assert_eq!(r.ai.strength_advantage, 0);
    r.step();
    assert_eq!(r.ai.advantage_timer, 0, "fired on frame 101");
    assert!(
        (52..=83).contains(&r.ai.strength_advantage),
        "advantage {} outside 62 plus the jitter",
        r.ai.strength_advantage
    );
    assert!(r.ai.strength_advantage > crate::ai::AGGRESSION_THRESHOLD);
}

#[test]
fn contact_dissolves_the_ai_unit_and_not_the_players() {
    let mut r = small_battle();
    for _ in 0..30_000 {
        r.step();
        if r.fighters.iter().any(|f| f.anim == Motion::Attacking) {
            break;
        }
    }
    assert!(
        r.fighters.iter().any(|f| f.anim == Motion::Attacking),
        "the armies never met"
    );
    let chasing = |side: Side, r: &BattleRunner| {
        r.fighters
            .iter()
            .enumerate()
            .filter(|(i, f)| f.side == side && r.is_alive(*i))
            .filter(|(i, _)| r.sim.figures[r.fighters[*i].sim].state == State::Chasing)
            .count()
    };
    assert!(chasing(SIDE_B, &r) > 0, "the AI unit did not dissolve on contact");
    assert_eq!(chasing(SIDE_A, &r), 0, "a player's unit must not be dissolved");
}

#[test]
fn a_battle_ends_with_one_side_standing() {
    let mut r = BattleRunner::deploy(
        blank_field(),
        &[(Troop::Knights, 8)],
        &[(Troop::Peasants, 8)],
    );
    for _ in 0..60_000 {
        r.step();
        if r.is_decided() {
            break;
        }
    }
    assert!(r.is_decided(), "battle never resolved");
    assert_eq!(r.living(SIDE_A), 0, "knights should have beaten peasants");
    assert!(r.living(SIDE_B) > 0);
    for (i, f) in r.fighters.iter().enumerate() {
        if !r.is_alive(i) {
            assert_eq!(f.anim, Motion::Dying);
        }
    }
}

#[test]
fn two_runs_of_the_same_battle_stay_identical() {
    let mut a = small_battle();
    let mut b = small_battle();
    for _ in 0..3_000 {
        a.step();
        b.step();
        assert_eq!(a.fighters, b.fighters, "diverged at tick {}", a.tick);
        assert_eq!(a.sim, b.sim, "simulation diverged at tick {}", a.tick);
        assert_eq!(a.units, b.units, "units diverged at tick {}", a.tick);
        assert_eq!(a.ai, b.ai, "the AI diverged at tick {}", a.tick);
    }
}

#[test]
fn a_different_seed_reaches_a_different_strength_advantage() {
    let armies = || {
        (
            vec![(Troop::Swordsmen, 4)],
            vec![(Troop::Swordsmen, 4)],
        )
    };
    let run = |seed: u64| {
        let (a, b) = armies();
        let mut r = BattleRunner::deploy_armies(
            blank_field(),
            seed,
            Army { troops: &a, owner: 1, human: false },
            Army { troops: &b, owner: 2, human: true },
        );
        r.run(101);
        r.ai.strength_advantage
    };
    assert_ne!(run(1), run(2), "the seed does not reach the advantage");
}

/// `Battle_UpdateStrengthAdvantage` (`0x0047FC01`) runs every hundred and
/// first frame and adds `(rand & 0x1F) - 10`, a span of 31 either side of
/// nothing. Two to one is an advantage of ±50 … 100, so no draw can carry
/// it across [`AGGRESSION_THRESHOLD`] (5) and every unit takes the same
/// branch in every battle — the aggressive one in `unit_order_advance`
/// (`ai.rs`), the cautious stand below it. An even fight sits inside the
/// jitter's reach and is the only place a seed decides anything.
///
/// The even-fight half then goes red too, every seed on one branch.
#[test]
fn a_lopsided_fight_draws_its_jitter_and_the_threshold_swallows_it() {
    let run = |seed: u64, ai_figs: u16, human_figs: u16| {
        let a = vec![(Troop::Swordsmen, ai_figs)];
        let b = vec![(Troop::Swordsmen, human_figs)];
        let mut r = BattleRunner::deploy_armies(
            blank_field(),
            seed,
            Army { troops: &a, owner: 1, human: false },
            Army { troops: &b, owner: 2, human: true },
        );
        let fresh = r.ai.rng.clone();
        r.run(101);
        let adv = r.ai.strength_advantage;
        (adv, r.ai.rng != fresh, adv > crate::ai::AGGRESSION_THRESHOLD)
    };

    for (ai_figs, human_figs) in [(40u16, 20u16), (20, 40)] {
        let (adv1, drew1, branch1) = run(1, ai_figs, human_figs);
        let (adv2, drew2, branch2) = run(2, ai_figs, human_figs);
        assert!(drew1 && drew2, "{ai_figs} v {human_figs}: the generator never advanced");
        assert_ne!(adv1, adv2, "{ai_figs} v {human_figs}: the seed did not reach the jitter");
        assert_eq!(branch1, branch2, "{ai_figs} v {human_figs}: two seeds, two branches");
        assert!(
            (adv1 - crate::ai::AGGRESSION_THRESHOLD).abs() > 31,
            "{ai_figs} v {human_figs}: advantage {adv1} is within the jitter's reach"
        );
    }

    let branches: Vec<bool> = (1..=7).map(|s| run(s, 30, 30).2).collect();
    assert!(
        branches.iter().any(|&b| b) && branches.iter().any(|&b| !b),
        "an even fight should split on the seed: {branches:?}"
    );
}

#[test]
fn a_wall_of_obstacles_is_routed_around_rather_than_walked_through() {
    let mut layer = vec![0u8; terrain::CELLS];
    layer[20 * DIM + 40] = 0x04;
    layer[60 * DIM + 40] = 0x0F;
    for x in 5..70 {
        layer[40 * DIM + x] = 0x02;
    }
    for x in 60..70 {
        layer[40 * DIM + x] = 0x00;
    }
    let field = terrain::build(&layer, 1);
    let mut r = BattleRunner::deploy(field, &[], &[(Troop::Knights, 3)]);
    r.order_side(SIDE_A, 40, 70);
    r.run(3_000);
    for f in &r.fighters {
        assert!(
            !r.field.at(f.x as usize, f.y as usize).impassable(),
            "a figure stands on impassable ground at {:?}",
            (f.x, f.y)
        );
    }
    assert!(
        r.fighters.iter().any(|f| f.y > 40),
        "nobody got past the wall: {:?}",
        r.fighters.iter().map(|f| (f.x, f.y)).collect::<Vec<_>>()
    );
    assert!(r.fighters.iter().any(|f| f.reroutes > 0), "nobody ever asked for a route");
}

#[test]
fn an_order_forms_the_unit_up_on_a_rectangle_around_the_destination() {
    let mut r = BattleRunner::deploy(blank_field(), &[], &[(Troop::Pikemen, 10)]);
    let unit = r.unit_of(0);
    r.order_unit(unit, 40, 40);
    let targets: Vec<(u8, u8)> = (0..r.fighters.len())
        .map(|i| r.fighters[i].target)
        .collect();
    let unique: std::collections::HashSet<_> = targets.iter().collect();
    assert_eq!(unique.len(), 10, "every figure needs its own slot: {targets:?}");
    let xs: Vec<u8> = targets.iter().map(|t| t.0).collect();
    let ys: Vec<u8> = targets.iter().map(|t| t.1).collect();
    let (x0, x1) = (*xs.iter().min().unwrap(), *xs.iter().max().unwrap());
    let (y0, y1) = (*ys.iter().min().unwrap(), *ys.iter().max().unwrap());
    assert_eq!((x1 - x0, y1 - y0), (4, 1), "a 5 x 2 rectangle");
    assert!((x0..=x1).contains(&40) && (y0..=y1).contains(&40), "centred on (40, 40)");
}

