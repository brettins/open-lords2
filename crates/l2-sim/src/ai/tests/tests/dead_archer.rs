#![allow(unused_imports)]
use super::*;
use super::super::*;
use l2_net::{Quirk, Quirks};

/// Archer figure 0 is a state-2 body, `men` 0, its slot not yet freed.
fn order(quirks: Quirks, owner: u8) -> Figure {
    let mut fx = Fixture::new(Troop::Archers, Troop::Peasants, 2);
    fx.figures[0].men = 0;
    fx.figures[0].state = State::Dead;
    fx.figures[0].owner = owner;
    fx.ai.quirks = quirks;
    let mut world = World {
        units: &mut fx.units,
        figures: &mut fx.figures,
        positions: &fx.positions,
        field: &fx.field,
        ai: &mut fx.ai,
    };
    world.shoot_at_unit(1, 0);
    fx.figures[0]
}

fn fixed() -> Quirks {
    let mut q = Quirks::FAITHFUL;
    q.set_reproduced(Quirk::DeadArcherKeepsShooting, false);
    q
}

/// Order_ShootAtUnit 0x0048C974, 00480000.c:5137 tests `owner != 0` only.
/// Ablation: `is_alive()` in the faithful arm of `shoot_at_unit` leaves him down.
#[test]
fn a_body_still_counting_down_is_ordered_to_shoot_at_band_three() {
    let f = order(Quirks::FAITHFUL, 1);
    assert_eq!(f.state, State::Shooting);
    assert!(f.target.is_some(), "Missile_FindTarget found nobody");
    // 00480000.c:2082 band 3 is `missileDamage / 2`
    assert_eq!(f.strength_band(), 3);
}

#[test]
fn with_the_fix_the_body_stays_down() {
    let f = order(fixed(), 1);
    assert_eq!((f.state, f.target), (State::Dead, None));
}

/// BattleMan_Clear zeroes `owner`; the loop skips a freed slot either way.
#[test]
fn a_freed_body_is_not_ordered() {
    let f = order(Quirks::FAITHFUL, 0);
    assert_eq!(f.state, State::Dead);
}

/// Missile_FindTarget 0x004956CC tests `state != 2`, not `men`: the body that
/// stood up is a target. Ablation: `!f.is_alive()` in `find_missile_target`.
#[test]
fn a_body_that_stood_up_is_a_target() {
    let mut fx = Fixture::new(Troop::Archers, Troop::Archers, 1);
    fx.figures[1].men = 0;
    fx.figures[1].state = State::Shooting;
    let world = World {
        units: &mut fx.units,
        figures: &mut fx.figures,
        positions: &fx.positions,
        field: &fx.field,
        ai: &mut fx.ai,
    };
    assert_eq!(world.find_missile_target(0, 40, 0), Some(1));
}
