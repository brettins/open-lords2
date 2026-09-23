#![allow(unused_imports)]
use super::*;
use super::outcome::*;
use super::ai_runs::*;
use super::missiles::*;
use super::super::*;
use crate::ai::Action;

#[test]
fn deployment_puts_every_figure_on_its_own_passable_cell() {
    let r = small_battle();
    assert_eq!(r.fighters.len(), 20);
    let mut seen = std::collections::HashSet::new();
    for f in &r.fighters {
        assert!(seen.insert((f.x, f.y)), "two figures on {:?}", (f.x, f.y));
        assert!(!r.field.at(f.x as usize, f.y as usize).impassable());
        assert!(f.x < 80 && f.y < 80);
    }
    assert_eq!(r.occupant.iter().filter(|o| o.is_some()).count(), 20);
}

#[test]
fn each_troop_group_becomes_a_unit_of_at_most_its_ceiling() {
    let r = small_battle();
    let live: Vec<usize> = r.units.live().collect();
    assert_eq!(live.len(), 4, "two units a side");
    let sizes: Vec<u8> = live.iter().map(|&u| r.units.get(u).figures).collect();
    assert_eq!(sizes, vec![6, 4, 6, 4]);
    let cats: Vec<u8> = live.iter().map(|&u| r.units.get(u).category).collect();
    assert_eq!(cats, vec![3, 1, 2, 2]);
    for f in &r.sim.figures {
        assert!(f.unit >= 1 && f.unit as usize <= MAX_UNITS, "figure with no unit");
    }
    assert!(!r.units.get(live[0]).human && r.units.get(live[0]).side == SIDE_B);
    assert!(r.units.get(live[2]).human && r.units.get(live[2]).side == SIDE_A);
}

#[test]
fn an_oversized_troop_group_splits_into_units_at_the_ceiling() {
    let r = BattleRunner::deploy(blank_field(), &[(Troop::Peasants, 20)], &[]);
    let live: Vec<usize> = r.units.live().collect();
    assert_eq!(live.len(), 2);
    assert_eq!(r.units.get(live[0]).figures, 12);
    assert_eq!(r.units.get(live[1]).figures, 8);
    let a = (r.units.get(live[0]).x, r.units.get(live[0]).y);
    let b = (r.units.get(live[1]).x, r.units.get(live[1]).y);
    assert_ne!(a, b, "both units deployed on the same slot");
}

#[test]
fn the_two_sides_deploy_at_opposite_markers_and_face_each_other() {
    let r = small_battle();
    let side4: Vec<u8> = r.fighters.iter().filter(|f| f.side == SIDE_B).map(|f| f.y).collect();
    let side0: Vec<u8> = r.fighters.iter().filter(|f| f.side == SIDE_A).map(|f| f.y).collect();
    assert!(!side4.is_empty() && !side0.is_empty());
    assert!(side0.iter().all(|&y| y < 40), "side 0 should be at the low end: {side0:?}");
    assert!(side4.iter().all(|&y| y > 40), "side 4 should be at the high end: {side4:?}");
    for f in &r.fighters {
        assert_eq!(f.facing, if f.y < 41 { 4 } else { 0 }, "at {:?}", (f.x, f.y));
    }
    for u in r.units.live() {
        let unit = r.units.get(u);
        assert_eq!((unit.target_x, unit.target_y), (unit.x, unit.y));
    }
}

/// `BattleMan_Step` (`0x0048F1DD`) calls `FUN_00491B1F` — which rewrites
/// `mapX`/`mapY` — the tick `Cell_TryEnter` says the cell is free, and
/// only then counts `walking` 1, 3 … 15. So an ordered pikeman is on the
/// next cell at once and stays there for 40 ticks.
#[test]
fn a_committed_man_is_on_the_next_cell_at_once_and_holds_it_for_the_delay() {
    let mut r = BattleRunner::deploy(
        blank_field(),
        &[(Troop::Pikemen, 1)],
        &[(Troop::Pikemen, 1)],
    );
    r.order_unit(r.unit_of(0), 40, 70);
    let start = (r.fighters[0].x, r.fighters[0].y);
    let mut moves = Vec::new();
    let mut at = start;
    for t in 1..=90 {
        r.step();
        let now = (r.fighters[0].x, r.fighters[0].y);
        if now != at {
            moves.push(t);
            at = now;
        }
    }
    assert_ne!(start, at, "he never moved");
    let first = moves[0];
    assert!(first <= 2, "the commit did not move him: first move at tick {first}");
    let gaps: Vec<u32> = moves.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(gaps.iter().all(|&g| g == 40), "a pikeman's cells came {gaps:?} ticks apart");
}

#[test]
fn a_knight_crosses_five_cells_while_a_pikeman_crosses_one() {
    fn cells_after_the_first(troop: Troop, ticks: u32) -> i32 {
        let mut r = BattleRunner::deploy(blank_field(), &[(troop, 1)], &[]);
        r.order_unit(r.unit_of(0), 40, 20);
        let start = r.fighters[0].y as i32;
        r.run(1);
        let committed = r.fighters[0].y as i32;
        assert_ne!(committed, start, "{troop:?} did not commit on its first tick");
        r.run(ticks);
        (r.fighters[0].y as i32 - committed).abs()
    }
    assert_eq!(cells_after_the_first(Troop::Knights, 40), 5);
    assert_eq!(cells_after_the_first(Troop::Pikemen, 40), 1);
}

#[test]
fn the_ai_marches_on_the_enemy_without_being_told_to() {
    let mut r = small_battle();
    let gap = |r: &BattleRunner| -> i32 {
        let a = r.fighters.iter().filter(|f| f.side == SIDE_B).map(|f| f.y as i32).min().unwrap();
        let b = r.fighters.iter().filter(|f| f.side == SIDE_A).map(|f| f.y as i32).max().unwrap();
        a - b
    };
    let before = gap(&r);
    let mut engaged = false;
    for _ in 0..30 {
        r.run(100);
        engaged |= r.fighters.iter().any(|f| f.anim == Motion::Attacking);
    }
    let after = gap(&r);
    assert!(after < before, "the armies did not close: {before} -> {after}");
    assert!(engaged, "nobody ever engaged");
}

