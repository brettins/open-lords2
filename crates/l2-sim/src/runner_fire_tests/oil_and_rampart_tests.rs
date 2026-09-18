#![allow(unused_imports)]
use super::*;
use super::bridge_and_tower_tests::*;
use super::wood_and_proving_tests::*;
use super::*;
use crate::cue::Cues;
use crate::fire::{SURFACE_BRIDGE, SURFACE_BURNING, SURFACE_WOODLAND, SURFACE_WOOD_BURNING, SURFACE_WOOD_CATCHING};
use crate::proving;
use crate::terrain::flag;

/// `BattleUnit_Order`'s oil loop, `FUN_0047A814` and `Missile_UpdateAll`'s
/// class-7 arm. The five lives are pinned as the literals the original
/// produces on the stream's first frame in flight — `ticksFlown` 5, so an
/// argument of `4 × 32 + bias` against `0x280`, and one frame already counted
/// off because every fire is in a higher slot than the stream: **471** for the
/// centre and the north, **511** south, **486** east, **501** west.
#[test]
fn a_pot_ordered_off_the_rampart_pours_and_the_ground_under_the_stream_burns() {
    let mut r = proving::deploy();
    while r.tick < proving::POUR_TICK {
        proving::orders(&mut r);
        r.step();
    }
    let pot = proving::fighter_of(&r, SIDE_A, Troop::Oil).unwrap();
    assert!(r.is_alive(pot), "the pot is standing before the order");
    assert_eq!((r.fighters[pot].x, r.fighters[pot].y), proving::POT_AT);
    assert_eq!(r.sim.cues.oil_poured(), 0);

    proving::orders(&mut r);
    assert!(!r.is_alive(pot), "FUN_0047A814 writes state 2: the pot is spent by its own pour");
    assert_eq!(r.sim.cues.oil_poured(), 1);
    assert_eq!(r.fighters[pot].facing, 4, "turned along the longer axis, south");
    let stream = r.missiles.iter().map(|(_, m)| *m).find(|m| m.class == crate::missile::CLASS_OIL);
    let stream = stream.expect("a stream in the air");
    assert_eq!((stream.ticks_flown, stream.cell_y), (4, 31), "four launch steps, two cells out");

    r.step();
    let expect = [((35, 31), 471), ((35, 30), 471), ((35, 32), 511), ((36, 31), 486), ((34, 31), 501)];
    for ((x, y), life) in expect {
        assert_eq!(r.field.cells[cell(x, y)].surface, SURFACE_BURNING, "({x}, {y}) is alight");
        assert_eq!(fire_at(&r, x, y).map(|m| m.ttl), Some(life), "({x}, {y})'s life");
    }
    assert_eq!(fire_at(&r, 35, 30).unwrap().saved_surface, crate::siege::SURFACE_WALL);

    for _ in 0..20 {
        proving::orders(&mut r);
        r.step();
    }
    assert_eq!(r.missiles.iter().filter(|(_, m)| m.class == crate::missile::CLASS_OIL).count(), 0);
    for y in 30..=38 {
        assert_eq!(r.field.cells[cell(35, y)].surface, SURFACE_BURNING, "row {y}");
    }
    assert_ne!(r.field.cells[cell(35, 39)].surface, SURFACE_BURNING, "row 39 is past the range");
    assert_eq!(fire_at(&r, 35, 37).unwrap().saved_surface, 0, "open ground");
}

#[test]
fn the_peasants_under_the_oil_burn_four_hits_a_frame_and_die_of_it() {
    let mut r = proving::deploy();
    assert_eq!(r.battle_size_class(), 0);
    let in_fire = |r: &BattleRunner| -> Vec<usize> {
        (0..r.fighters.len())
            .filter(|&i| {
                r.fighters[i].troop == Troop::Peasants
                    && r.is_alive(i)
                    && r.field.cells[cell(r.fighters[i].x, r.fighters[i].y)].surface == SURFACE_BURNING
            })
            .collect()
    };
    while in_fire(&r).is_empty() && r.tick < proving::POUR_TICK + 40 {
        proving::orders(&mut r);
        r.step();
    }
    let burning = in_fire(&r);
    assert!(!burning.is_empty(), "the pour landed on somebody by frame {}", r.tick);
    let i = burning[0];
    let sim = r.fighters[i].sim;
    let (men, hits) = (r.sim.figures[sim].men, r.sim.figures[sim].hits);
    r.step();
    let after = (r.sim.figures[sim].men, r.sim.figures[sim].hits);
    assert!(
        after == (men, hits + 4) || after == (men - 1, hits + 4 - 100),
        "four hits a frame: {men}/{hits} -> {after:?}"
    );

    for _ in 0..200 {
        proving::orders(&mut r);
        r.step();
    }
    assert!(!r.is_alive(i), "a hundred hits a man, four men, a hundred frames");
    assert!(r.sim.cues.burn_deaths(SIDE_B) >= 1);
    assert_eq!(r.sim.cues.burn_deaths(SIDE_A), 0);
    assert_eq!(r.sim.cues.melee_deaths(SIDE_B) + r.sim.cues.missile_deaths(), 0, "fire, and only fire");
}

#[test]
fn an_oil_fire_goes_out_and_the_wall_comes_back() {
    let fresh = proving::field();
    let mut r = proving::run(proving::POUR_TICK + 1);
    let life = fire_at(&r, 35, 30).unwrap().ttl;
    assert_eq!(life, 471);
    for _ in 0..(life - 2) {
        proving::orders(&mut r);
        r.step();
    }
    assert_eq!(r.field.cells[cell(35, 30)].surface, SURFACE_BURNING, "one frame before");
    r.step();
    assert_eq!(r.field.cells[cell(35, 30)], fresh.cells[cell(35, 30)], "the frame the count reads 2");
    assert_eq!(r.field.cells[cell(35, 30)].surface, crate::siege::SURFACE_WALL);
    assert!(r.blocked[cell(35, 30)], "still a curtain nobody walks through");
    r.step();
    assert!(fire_at(&r, 35, 30).is_none(), "and the record is gone a frame later");
}

/// **A green ablation, and it is the finding.** Returning `None` from
/// `adjacent_oil` alone leaves this green: the swordsman then commits the step
/// into the pot's cell, and `enter_cell`'s contact arm pours on the same frame
/// at the same cell. The two paths are the original's one road seen from both
/// ends — the direction chosen, and the step refused with 999 — so only
/// ablating **both** turns it red, which it does.
#[test]
fn a_man_who_steps_at_a_pot_of_oil_is_poured_on() {
    let mut r = BattleRunner::empty(blank_field(), DEFAULT_SEED);
    let pot = stand(&mut r, Troop::Oil, SIDE_A, 1, false, (40, 40));
    let man = stand(&mut r, Troop::Swordsmen, SIDE_B, 2, false, (40, 44));
    r.fighters[man].target = (40, 36);
    r.settle();
    let mut poured_at = None;
    for _ in 0..400 {
        r.step();
        if r.sim.cues.oil_poured() > 0 {
            poured_at = Some((r.fighters[man].x, r.fighters[man].y));
            break;
        }
    }
    assert_eq!(poured_at, Some((40, 41)), "poured on the frame he stepped at it, one cell away");
    assert!(!r.is_alive(pot));
    for _ in 0..3 {
        r.step();
    }
    let sim = r.fighters[man].sim;
    assert!(
        r.sim.figures[sim].hits > 0 || r.sim.figures[sim].men < 4,
        "he is standing in his own pour"
    );
}


