//! **The three writers of [`Fighter::delay`]**, all in `BattleMan_Step`
//! (`0x0048F1DD`): the swap's push-aside, the refused swap's 1-2 frame wait
//! (`00480000.c:6459-6463`) and the blocked arm's `delay = 100`
//! (`00480000.c:6583-6586`).

use crate::runner::*;
use crate::Troop;

/// One unit of two swordsmen against a pikeman.
fn two_of_a_unit() -> (BattleRunner, usize, usize) {
    let mut r =
        BattleRunner::deploy(blank_field(), &[(Troop::Swordsmen, 2)], &[(Troop::Pikemen, 1)]);
    let two: Vec<usize> =
        (0..r.fighters.len()).filter(|&i| r.fighters[i].troop == Troop::Swordsmen).collect();
    let (a, b) = (two[0], two[1]);
    assert_eq!(r.unit_of(a), r.unit_of(b), "one unit");
    place(&mut r, a, 10, 10);
    place(&mut r, b, 10, 9);
    (r, a, b)
}

fn place(r: &mut BattleRunner, i: usize, x: u8, y: u8) {
    let old = r.fighters[i].y as usize * DIM + r.fighters[i].x as usize;
    if r.occupant[old] == Some(i as u16) {
        r.occupant[old] = None;
    }
    r.fighters[i].x = x;
    r.fighters[i].y = y;
    r.fighters[i].path.clear();
    r.fighters[i].hold = 0;
    r.fighters[i].barred = 0;
    r.fighters[i].delay = 0;
    r.occupant[y as usize * DIM + x as usize] = Some(i as u16);
}

fn human(r: &mut BattleRunner, i: usize) {
    r.sim.figures[r.fighters[i].sim].owner_is_human = true;
}

/// **A comrade filling the moat is stepped around, not swapped with.**
/// `00490000.c:46-48`: `other.state == 9` answers 1.
///
/// **The ablation.** Drop the `busy` arm of [`BattleRunner::swap_verdict`] and
/// the moat-filler is pulled off his cell into the walker's.
#[test]
fn a_comrade_filling_the_moat_is_not_swapped_off_his_cell() {
    let (mut r, a, b) = two_of_a_unit();
    let sim = r.fighters[b].sim;
    r.sim.figures[sim].state = crate::figure::State::FillingMoat;
    r.fighters[a].target = (10, 9);
    r.fighters[b].target = (11, 5);

    r.enter(a, Pos::new(10, 9));

    assert_eq!((r.fighters[b].x, r.fighters[b].y), (10, 9), "he kept his cell");
    assert_ne!((r.fighters[a].x, r.fighters[a].y), (10, 9), "and was not exchanged with");
    assert_eq!(r.fighters[a].delay, 0, "answer 1 is no wait");
}

/// **Another unit's man is never swapped with**, whoever owns him:
/// `00480000.c:6443` enters the arm only for `cur.unit == other.unit`.
#[test]
fn a_man_of_another_unit_is_not_swapped_with() {
    let (mut r, a, b) = two_of_a_unit();
    // `deploy` merges two entries of one troop into one unit, so the second
    // unit is made here.
    let sim = r.fighters[b].sim;
    r.sim.figures[sim].unit += 1;
    assert_ne!(r.unit_of(a), r.unit_of(b), "two units of one swordsman");
    human(&mut r, a);
    human(&mut r, b);
    r.fighters[a].target = (10, 9);
    r.fighters[b].target = (11, 5);

    r.enter(a, Pos::new(10, 9));

    assert_eq!((r.fighters[b].x, r.fighters[b].y), (10, 9), "the blocker kept his cell");
    assert_eq!(r.fighters[a].delay, 0, "and no wait was taken");
}

/// **No side-step and no route is `delay = 100`**, `00480000.c:6583-6586`.
///
/// **The ablation.** Drop the `PARK` write at the tail of
/// [`BattleRunner::request_path_with`] and the man searches again on the next
/// tick, which is the retry loop the oracle check named.
#[test]
fn a_man_with_no_side_step_and_no_route_parks_for_a_hundred_frames() {
    let (mut r, a, b) = two_of_a_unit();
    place(&mut r, b, 60, 60);
    for x in 0..DIM {
        r.blocked[11 * DIM + x] = true;
    }
    // Two cells away, so `field_0x169 < 2` fails and `FUN_004904EC` answers 8.
    r.fighters[a].target = (10, 12);

    r.request_path(a);

    assert!(r.fighters[a].path.is_empty(), "nothing came back");
    assert_eq!(r.fighters[a].delay, 100, "`delay = 100`");
}

/// Swordsman `a` and the pikeman locked in a mutual duel, state 4 both.
fn dueling() -> (BattleRunner, usize) {
    let (mut r, a, b) = two_of_a_unit();
    place(&mut r, b, 60, 60);
    let p = (0..r.fighters.len()).find(|&i| r.fighters[i].troop == Troop::Pikemen).unwrap();
    place(&mut r, p, 11, 10);
    let (sa, sp) = (r.fighters[a].sim, r.fighters[p].sim);
    for (me, other) in [(sa, sp), (sp, sa)] {
        r.sim.figures[me].state = crate::figure::State::Melee;
        r.sim.figures[me].opponent = Some(other);
    }
    (r, a)
}

/// Ablation: movement.rs's `delay = 0` for a man in state 4, BattleMan_StateMelee 00480000.c:1386-1402.
#[test]
fn a_delay_interrupted_by_a_duel_is_not_run_out_afterwards() {
    let (mut r, a) = dueling();
    r.fighters[a].delay = 50;
    r.step_one(a);
    assert_eq!(r.fighters[a].delay, 0);
}

/// Ablation: movement.rs's march for the man `Melee_Tick` killed, 00480000.c:1396.
#[test]
fn a_man_killed_mid_crossing_still_steps_that_tick() {
    let (mut r, a) = dueling();
    let sa = r.fighters[a].sim;
    r.sim.figures[sa].men = 1;
    r.sim.figures[sa].hits = 500;
    r.fighters[a].progress.free = false;
    let before = (r.fighters[a].progress.tick_counter, r.fighters[a].progress.substep);
    r.step_one(a);
    assert!(!r.is_alive(a), "Melee_Tick killed him");
    let after = (r.fighters[a].progress.tick_counter, r.fighters[a].progress.substep);
    assert_ne!(after, before, "BattleMan_Step(1) counted his crossing");
}

/// Ablation: stepping.rs's `count_men`, Battle_Frame 004b0000.c:6703.
#[test]
fn the_ais_census_is_taken_every_frame() {
    let (mut r, _, _) = two_of_a_unit();
    let before = r.ai.men_total;
    for f in r.sim.figures.iter_mut().filter(|f| !f.owner_is_human) {
        f.men = 1;
    }
    r.step();
    assert!(r.ai.men_total < before, "{} -> {}", before, r.ai.men_total);
}
