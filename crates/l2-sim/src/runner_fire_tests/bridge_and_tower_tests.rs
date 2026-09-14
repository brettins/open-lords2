#![allow(unused_imports)]
use super::*;
use super::oil_and_rampart_tests::*;
use super::wood_and_proving_tests::*;
use super::*;
use crate::cue::Cues;
use crate::fire::{SURFACE_BRIDGE, SURFACE_BURNING, SURFACE_WOODLAND, SURFACE_WOOD_BURNING, SURFACE_WOOD_CATCHING};
use crate::proving;
use crate::terrain::flag;

/// **A besieger stepping onto a bridge sets it alight, and the fire walks five
/// cells along it.** `Cell_TryEnter`'s first statement.
///
/// The swordsman walks north up column 50; the bridge is rows 40 to 46. He
/// steps onto 46, which burns, and so do 45 … 41 — five rings — while 40 is
/// out of reach and stays a bridge until he reaches it. A burning bridge is
/// flattened and cleared: height 0, no flags.
#[test]
fn a_besieger_stepping_onto_a_bridge_sets_it_alight() {
    let mut r = proving::deploy();
    let man = proving::fighter_of(&r, SIDE_B, Troop::Swordsmen).unwrap();
    while r.sim.cues.bridges_fired() == 0 && r.tick < 1_000 {
        proving::orders(&mut r);
        r.step();
    }
    assert_eq!(r.sim.cues.bridges_fired(), 1, "by frame {}", r.tick);
    assert_eq!((r.fighters[man].x, r.fighters[man].y), (proving::BRIDGE_X, 46));
    for y in 41..=46 {
        let c = r.field.cells[cell(proving::BRIDGE_X, y)];
        assert_eq!((c.surface, c.elevation, c.flags), (SURFACE_BURNING, 0, 0), "row {y}");
    }
    assert_eq!(r.field.cells[cell(proving::BRIDGE_X, 40)].surface, SURFACE_BRIDGE);
    assert_eq!(fire_at(&r, proving::BRIDGE_X, 46).unwrap().ttl, 519, "0x280 - 0x78, one frame counted");
    assert_eq!(fire_at(&r, proving::BRIDGE_X, 41).unwrap().ttl, 519 + 28, "each ring seven frames longer");

    // He burns on it, four hits a frame, and dies of it before he is across.
    let sim = r.fighters[man].sim;
    for _ in 0..140 {
        proving::orders(&mut r);
        r.step();
    }
    assert!(!r.sim.figures[sim].is_alive(), "a swordsman does not cross a burning bridge");
    assert!(r.sim.cues.burn_deaths(SIDE_B) >= 1);

    // And when it goes out, a bridge does not come back.
    for _ in 0..600 {
        proving::orders(&mut r);
        r.step();
    }
    for y in 41..=46 {
        let c = r.field.cells[cell(proving::BRIDGE_X, y)];
        assert_eq!((c.surface, c.elevation), (crate::fire::SURFACE_BURNT_BRIDGE, 0), "row {y} burnt out");
    }
}

/// **An arrow over a bridge sets it alight too**, and is spent by it: its
/// countdown becomes 8 and it hits nothing more.
#[test]
fn an_arrow_crossing_a_bridge_sets_it_alight_and_is_spent() {
    let mut field = blank_field();
    field.cells[cell(30, 40)].surface = SURFACE_BRIDGE;
    let mut r = BattleRunner::empty(field, DEFAULT_SEED);
    let archer = stand(&mut r, Troop::Archers, SIDE_A, 1, false, (20, 40));
    stand(&mut r, Troop::Peasants, SIDE_B, 2, false, (34, 40));
    r.settle();
    let _ = archer;
    for _ in 0..200 {
        r.step();
        if r.sim.cues.bridges_fired() > 0 {
            break;
        }
    }
    assert_eq!(r.sim.cues.bridges_fired(), 1);
    assert_eq!(r.field.cells[cell(30, 40)].surface, SURFACE_BURNING);
    let arrow = r.missiles.iter().map(|(_, m)| *m).find(|m| m.class == 1).expect("the arrow");
    assert!((1..=8).contains(&arrow.ttl), "spent, and counting down from 8: {}", arrow.ttl);
    assert_eq!(r.sim.cues.missile_hits(WeaponClass::Bow), 0, "and it hits nobody on the far side");
}

// ------------------------------------------------------------- the siege tower

/// **A tower whose leading edge meets a wall two high docks, is destroyed, and
/// leaves a ramp** — `FUN_00491492` and `FUN_004921E5`.
///
/// Every height and flag below is a literal of those two bodies, for a tower
/// facing north at `(25, 32)` against a curtain at row 30.
#[test]
fn a_tower_docks_against_a_wall_two_high_and_becomes_a_ramp() {
    let mut r = proving::deploy();
    let tower = proving::fighter_of(&r, SIDE_B, Troop::SiegeTowers).unwrap();
    let (breach, approach) = (r.ai.breach_score, r.ai.approach_score);
    while r.sim.cues.towers_docked() == 0 && r.tick < 1_500 {
        proving::orders(&mut r);
        r.step();
    }
    assert_eq!(r.sim.cues.towers_docked(), 1, "docked by frame {}", r.tick);

    let (x, y) = proving::TOWER_DOCKS_AT;
    let tsim = r.fighters[tower].sim;
    assert_eq!((r.sim.figures[tsim].men, r.sim.figures[tsim].is_alive()), (0, false), "BattleMan_Destroy");
    assert_eq!(r.occupant_of(x, y), None);
    assert_eq!(r.survivors(SIDE_B)[Troop::SiegeTowers.index()], 0, "a docked tower is not a survivor");

    let at = |x: u8, y: u8| r.field.cells[cell(x, y)];
    assert_eq!((at(x, 30).elevation, at(x, 30).flags, at(x, 30).gfx), (2, 0, 1), "the wall top, open");
    assert_eq!(at(x, 31).elevation, 2, "the top step");
    assert_eq!(at(x, 32).elevation, 1, "the tower's own cell");
    assert_eq!(at(x, 33).elevation, 1, "the foot");
    for (fx, fy) in [(24, 31), (26, 31), (24, 32), (26, 32), (24, 33), (26, 33)] {
        assert_ne!(at(fx, fy).flags & flag::IMPASSABLE, 0, "({fx}, {fy}) walls the ramp");
    }
    let frames: Vec<u8> = (31..=33).flat_map(|fy| (24..=26).map(move |fx| (fx, fy))).map(|(fx, fy)| at(fx, fy).gfx).collect();
    assert_eq!(frames, [64, 65, 66, 72, 73, 74, 80, 81, 82], "DAT_004D9DD0's first row");
    assert_eq!(r.ai.breach_score - breach, 3);
    assert_eq!(r.ai.approach_score - approach, 4);
    assert!(r.ai_field.defence_posts.contains(&cell(x, 30)), "FUN_0048EE46 files the wall top");
    assert!(!r.blocked[cell(x, 30)] && r.blocked[cell(24, 32)], "and the blocked map has it");
}

/// **And a knight can walk up it onto the wall**, which is what a tower is
/// for. Before the dock he could not: two levels at once is refused.
///
/// Ablation: return `false` from `dock_tower` and he stands at the foot.
#[test]
fn a_knight_climbs_the_ramp_a_docked_tower_leaves() {
    let r = proving::run(1_000);
    let knight = proving::fighter_of(&r, SIDE_B, Troop::Knights).unwrap();
    assert_eq!((r.fighters[knight].x, r.fighters[knight].y), proving::KNIGHT_TO);
    assert_eq!(r.field.cells[cell(proving::KNIGHT_TO.0, proving::KNIGHT_TO.1)].elevation, 2);
}

/// **A tower docks only against exactly two**: a wall one high or three high
/// is no dock, and nor is a wall two high behind a step exactly one high.
#[test]
fn a_tower_docks_only_against_ground_exactly_two_high() {
    for (wall, step, docks) in [(2u8, 0u8, true), (1, 0, false), (3, 0, false), (2, 1, false), (2, 2, true)] {
        let mut field = blank_field();
        field.cells[cell(40, 38)].elevation = wall;
        field.cells[cell(40, 39)].elevation = step;
        assert_eq!(
            crate::siege::tower_dock_site(&field, 40, 40, 0).is_some(),
            docks,
            "wall {wall}, step {step}"
        );
    }
    // The search starts from the polar facing and turns clockwise by two.
    let mut field = blank_field();
    field.cells[cell(42, 40)].elevation = 2;
    field.cells[cell(38, 40)].elevation = 2;
    assert_eq!(crate::siege::tower_dock_site(&field, 40, 40, 0).map(|d| d.0), Some(2));
    assert_eq!(crate::siege::tower_dock_site(&field, 40, 40, 4).map(|d| d.0), Some(6));
}

/// `FUN_00488436`'s troop-8 arm, every row.
#[test]
fn a_towers_polar_facing_is_the_nearest_orthogonal_and_holds_on_a_diagonal() {
    use crate::siege::tower_polar;
    for d in [0u8, 2, 4, 6] {
        assert_eq!(tower_polar(d, 0), d);
    }
    assert_eq!((tower_polar(1, 2), tower_polar(1, 0), tower_polar(1, 6)), (2, 0, 0));
    assert_eq!((tower_polar(3, 2), tower_polar(3, 4)), (2, 4));
    assert_eq!((tower_polar(5, 6), tower_polar(5, 4)), (6, 4));
    assert_eq!((tower_polar(7, 6), tower_polar(7, 0)), (6, 0));
}

/// **Any figure in a tower's leading edge stops it**, a friend included —
/// `Cell_TryEnterEngine` counts figures without asking whose.
#[test]
fn a_friend_in_a_towers_leading_edge_stops_it() {
    let mut r = BattleRunner::empty(blank_field(), DEFAULT_SEED);
    let tower = stand(&mut r, Troop::SiegeTowers, SIDE_B, 2, false, (40, 50));
    stand(&mut r, Troop::Peasants, SIDE_B, 2, false, (41, 48));
    r.fighters[tower].target = (40, 30);
    r.settle();
    r.run(400);
    assert_eq!((r.fighters[tower].x, r.fighters[tower].y), (40, 50), "the peasant is in the edge");
}

// --------------------------------------------------------- the rampart too high

/// **A catapult cannot shoot down a wall four high.** `Missile_Step`'s class-3
/// arm counts a hit only below 4, and plays `catmiss.wav` above it.
///
/// The same shot at the same wall three high is counted — which is the
/// ablation this test carries inside it.
#[test]
fn a_catapult_shot_at_a_rampart_four_high_is_not_counted() {
    let r = proving::run(400);
    let wall = cell(proving::CATAPULT_AT.0, proving::HIGH_WALL_Y);
    assert!(r.sim.cues.walls_missed() >= 2, "{:?}", r.sim.cues);
    assert_eq!(r.sim.cues.walls_struck(), 0);
    // The count is cell byte `+0` itself — `Missile_Step`'s `cell.terrain++`.
    let fresh = proving::deploy();
    assert_eq!(
        r.field.cells[wall].terrain, fresh.field.cells[wall].terrain,
        "nothing counted against the cell"
    );
    assert_ne!(r.field.cells[wall].flags & crate::siege::FLAG_WALL, 0, "and it stands");

    let mut lower = proving::deploy();
    let seed = lower.field.cells[wall].terrain;
    for x in proving::HIGH_WALL_X {
        lower.field.cells[cell(x, proving::HIGH_WALL_Y)].elevation = 3;
    }
    for _ in 0..400 {
        proving::orders(&mut lower);
        lower.step();
    }
    assert!(lower.sim.cues.walls_struck() >= 2, "{:?}", lower.sim.cues);
    assert_eq!(lower.sim.cues.walls_missed(), 0);
    assert!(lower.field.cells[wall].terrain >= seed + 2, "the cell byte counted the hits");
}

// ----------------------------------------------------------------- the wood fire

