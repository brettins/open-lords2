#![allow(unused_imports)]
use super::*;
use super::missiles::firing_line;
use super::super::*;

/// Set the cells `xs` of the firing row to `flags` and `elevation`, and keep
/// the mover's map in step with them.
fn paint(r: &mut BattleRunner, xs: std::ops::RangeInclusive<usize>, flags: u8, elevation: u8) {
    for x in xs {
        let c = 40 * DIM + x;
        r.field.cells[c].flags |= flags;
        r.field.cells[c].elevation = elevation;
        r.sync_cell(c);
    }
}

#[test]
fn an_arrow_flies_over_a_river() {
    let mut r = firing_line(Troop::Archers, Troop::Peasants, 10);
    paint(&mut r, 23..=27, crate::terrain::flag::IMPASSABLE, 0);
    r.run(600);
    assert!(r.men_of_side(SIDE_B) < 4, "water stops a man, not an arrow");
}

#[test]
fn an_archer_on_a_wall_shoots_over_the_parapet() {
    let mut r = firing_line(Troop::Archers, Troop::Peasants, 10);
    paint(&mut r, 20..=20, 0, 1);
    paint(&mut r, 21..=22, crate::terrain::flag::IMPASSABLE | crate::siege::FLAG_WALL, 1);
    paint(&mut r, 24..=25, crate::terrain::flag::IMPASSABLE, 0);
    r.run(600);
    assert!(r.men_of_side(SIDE_B) < 4, "the parapet and the moat let the arrow pass");
}

#[test]
fn an_archer_on_the_ground_hits_a_man_on_the_wall() {
    let mut r = firing_line(Troop::Archers, Troop::Peasants, 10);
    paint(&mut r, 24..=25, crate::terrain::flag::IMPASSABLE, 0);
    paint(&mut r, 28..=29, crate::terrain::flag::IMPASSABLE | crate::siege::FLAG_WALL, 1);
    paint(&mut r, 30..=30, 0, 1);
    r.run(600);
    assert!(r.men_of_side(SIDE_B) < 4, "the moat and the wall face let the arrow pass");
}

/// Missile_Step 0x00492C8B 00490000.c:1242 `local_2c = 99` on a `0x80` cell, so :1253 keeps a
/// shot at launch elevation in the air; :1247 `blockedTicks > 0x20` frees it. Ablation: `freed_at`.
#[test]
fn a_hill_frees_an_arrow_only_when_its_counter_runs_out() {
    let mut r = firing_line(Troop::Archers, Troop::Peasants, 10);
    paint(&mut r, 24..=24, crate::terrain::flag::BLOCKED, 0);
    let slot = crate::missile::spawn(&mut r.missiles, 1, WeaponClass::Bow, 0, (20, 40), (24, 40), 50, 0).unwrap();
    let m = r.missiles.get_mut(slot);
    (m.cell_x, m.cell_y) = (24, 40);
    let mut flown = 0;
    while r.missile_cell_tests(slot) {
        flown += 1;
    }
    assert_eq!(flown, 0x20 - 4, "the counter is seeded 4 for shooter 0, and frees past 0x20");
}
