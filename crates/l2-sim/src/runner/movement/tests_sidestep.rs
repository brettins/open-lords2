//! The two rules `BattleMan_Step` applies to a **blocked** man before and
//! after it reroutes him: `FUN_004904EC`'s side-step (`0x004904EC`) and
//! `Path_DetourTooLong`'s give-up (`0x00472227`). `docs/battle.md` §8.3a.

use crate::runner::*;
use crate::Troop;

/// Two comrades on side A, one enemy, on a field with no terrain.
fn field_battle() -> BattleRunner {
    BattleRunner::deploy(
        blank_field(),
        &[(Troop::Swordsmen, 1), (Troop::Archers, 1)],
        &[(Troop::Pikemen, 1)],
    )
}

/// The swordsman and the archer, in that order — two figures of one side, so
/// the blocker is a comrade and not an enemy.
fn comrades(r: &BattleRunner) -> [usize; 2] {
    let find = |t: Troop| r.fighters.iter().position(|f| f.troop == t).unwrap();
    let (a, b) = (find(Troop::Swordsmen), find(Troop::Archers));
    assert_eq!(r.fighters[a].side, r.fighters[b].side, "same army");
    [a, b]
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
    r.occupant[y as usize * DIM + x as usize] = Some(i as u16);
}

// --- the side-step, `FUN_004904EC` ---------------------------------------

/// A man one cell from his target, with a comrade standing on it, takes the
/// **first rotation** of the direction he wanted: N is taken, so NE.
///
/// **The ablation.** Delete the `side_step` call at the head of
/// [`BattleRunner::request_path`] and this figure does not move at all — it
/// asks for a path and waits, which is the defect the row was filed for.
#[test]
fn a_blocked_man_side_steps_into_the_first_free_rotation() {
    let mut r = field_battle();
    let a = comrades(&r);
    place(&mut r, a[0], 10, 10);
    place(&mut r, a[1], 10, 9);
    r.fighters[a[0]].target = (10, 9);

    r.request_path(a[0]);

    let f = &r.fighters[a[0]];
    assert_eq!(
        (f.x, f.y),
        (11, 9),
        "N was taken, so the next clockwise rotation"
    );
    assert_eq!(f.facing, 1, "`dirc = dir` from the mover's tail");
    assert_eq!(f.barred, 0, "a step that lands clears `barred`");
    assert!(f.path.is_empty(), "a side-step is not a route");
}

/// The anticlockwise walker gets its turn in the same round: with NE blocked
/// too, the third test is NW (`dir - 1`), not `dir + 2`.
#[test]
fn the_two_walkers_alternate_clockwise_then_anticlockwise() {
    let mut r = field_battle();
    let a = comrades(&r);
    place(&mut r, a[0], 10, 10);
    place(&mut r, a[1], 10, 9);
    r.fighters[a[0]].target = (10, 9);
    r.blocked[9 * DIM + 11] = true; // NE, the clockwise walker's first rotation

    r.request_path(a[0]);

    assert_eq!(
        (r.fighters[a[0]].x, r.fighters[a[0]].y),
        (9, 9),
        "NW, `dir - 1`"
    );
    assert_eq!(r.fighters[a[0]].facing, 7);
}

/// `field_0x169 < 2`: two cells out, a blocked man does not shuffle. He
/// reroutes instead, which is where he has always gone.
#[test]
fn the_side_step_does_not_fire_more_than_one_cell_from_the_target() {
    let mut r = field_battle();
    let a = comrades(&r);
    place(&mut r, a[0], 10, 10);
    place(&mut r, a[1], 10, 9);
    r.fighters[a[0]].target = (10, 8);

    r.request_path(a[0]);

    assert_eq!(
        (r.fighters[a[0]].x, r.fighters[a[0]].y),
        (10, 10),
        "he stood and searched"
    );
    assert_eq!(
        r.fighters[a[0]].reroutes, 1,
        "and the search is what he did"
    );
}

/// A man boxed in on all eight sides falls through to the pathfinder rather
/// than stepping onto anything.
#[test]
fn a_man_with_nowhere_to_step_does_not_side_step() {
    let mut r = field_battle();
    let a = comrades(&r);
    place(&mut r, a[0], 10, 10);
    place(&mut r, a[1], 60, 60);
    r.fighters[a[0]].target = (10, 9);
    for (dx, dy) in FACING_DELTA {
        r.blocked[(10 + dy) as usize * DIM + (10 + dx) as usize] = true;
    }

    r.request_path(a[0]);

    assert_eq!((r.fighters[a[0]].x, r.fighters[a[0]].y), (10, 10));
}

// --- the give-up, `Path_DetourTooLong` -----------------------------------

/// A wall across the field with one gap far to the right: the straight-line
/// distance to the far side is 2 and the only route is ~120 cells.
fn walled_off(r: &mut BattleRunner) {
    for x in 0..DIM {
        if x != 70 {
            r.blocked[11 * DIM + x] = true;
        }
    }
}

/// **The player's man gives up and stands where he is** — `tgX, tgY = mapX,
/// mapY`.
///
/// **The ablation.** Remove the `detour_too_long` arm from
/// [`BattleRunner::request_path`] and the figure keeps `target == (10, 12)`
/// and walks away with a hundred-odd waypoints, which is the original walking
/// the length of the field to reach a cell two away.
#[test]
fn a_players_man_gives_up_on_an_absurd_detour() {
    let mut r = field_battle();
    let a = comrades(&r);
    walled_off(&mut r);
    place(&mut r, a[0], 10, 10);
    place(&mut r, a[1], 60, 60);
    r.sim.figures[r.fighters[a[0]].sim].owner_is_human = true;
    r.fighters[a[0]].target = (10, 12);

    r.request_path(a[0]);

    let f = &r.fighters[a[0]];
    assert_eq!(
        f.target,
        (10, 10),
        "he abandoned the destination for his own cell"
    );
    assert!(f.path.is_empty(), "and kept none of the route");
    assert_eq!((f.x, f.y), (10, 10), "giving up is not moving");
}

/// The same detour, one guard away: `ownerIsHuman == 0` returns 0 before the
/// caps are even read, so **the AI never gives up**.
#[test]
fn an_ai_man_walks_the_whole_absurd_detour() {
    let mut r = field_battle();
    let a = comrades(&r);
    walled_off(&mut r);
    place(&mut r, a[0], 10, 10);
    place(&mut r, a[1], 60, 60);
    r.sim.figures[r.fighters[a[0]].sim].owner_is_human = false;
    r.fighters[a[0]].target = (10, 12);

    r.request_path(a[0]);

    let f = &r.fighters[a[0]];
    assert_eq!(f.target, (10, 12), "the destination stands");
    assert!(
        f.path.len() > 100,
        "and he has the long way round: {}",
        f.path.len()
    );
}

/// A route inside the field cap is kept, however awkward. The gap moved to
/// three cells away makes the detour about eight — over nothing.
#[test]
fn a_short_detour_is_not_too_long() {
    let mut r = field_battle();
    let a = comrades(&r);
    for x in 0..DIM {
        if x != 13 {
            r.blocked[11 * DIM + x] = true;
        }
    }
    place(&mut r, a[0], 10, 10);
    place(&mut r, a[1], 60, 60);
    r.sim.figures[r.fighters[a[0]].sim].owner_is_human = true;
    r.fighters[a[0]].target = (10, 12);

    r.request_path(a[0]);

    assert_eq!(
        r.fighters[a[0]].target,
        (10, 12),
        "eight cells is not absurd"
    );
    assert!(!r.fighters[a[0]].path.is_empty());
}

/// **A comrade of the figure's own unit is waited for, not walked around.**
/// `BattleMan_Step` returns at `SwapPlaces() == 0` with a 1–2 frame delay and
/// never reaches `FUN_004904EC`; only a blocker from another unit — or a corpse
/// — falls through to `local_10 = 3`.
///
/// **The ablation.** Pass `true` instead of `!comrade` in
/// [`BattleRunner::enter_cell`]'s friendly arm and this figure shuffles off its
/// slot. It is worth a test of its own: without the guard the side-step lets a
/// whole unit scatter out of formation to reach contact, and
/// `l2-game`'s `seam` battle swung from 4 attacker wins in 12 seeds to 10.
#[test]
fn a_comrade_of_the_same_unit_is_not_side_stepped_around() {
    let mut r = BattleRunner::deploy(
        blank_field(),
        &[(Troop::Swordsmen, 2)],
        &[(Troop::Pikemen, 1)],
    );
    let two: Vec<usize> =
        (0..r.fighters.len()).filter(|&i| r.fighters[i].troop == Troop::Swordsmen).collect();
    let (a, b) = (two[0], two[1]);
    assert_eq!(r.unit_of(a), r.unit_of(b), "one unit of swordsmen");
    place(&mut r, a, 10, 10);
    place(&mut r, b, 10, 9);
    r.fighters[a].target = (10, 9);
    // One destination for the two of them, so `BattleMen_SwapPlaces`
    // (`0x0049005F`) takes its opening `return 0` — the wait.
    r.fighters[b].target = (10, 9);

    // Through the mover, since the guard is in `enter_cell`'s friendly arm.
    r.enter(a, Pos::new(10, 9));

    assert_eq!((r.fighters[a].x, r.fighters[a].y), (10, 10), "he waits for his own man");
}

/// **A man swapped out of his cell stands for a frame or two.**
/// `BattleMan_Step`'s `BattleMen_SwapPlaces` arm (`0x0049005F`) gives him
/// `state = 1`, `delayState = 3` and `delay = (other & 1) + 1`, then returns 0.
///
/// **The ablation.** Drop the `delay` write in
/// [`BattleRunner::swap_places`] and he takes his own step in the same tick he
/// was pushed aside — two cells in one frame, which nothing in the original
/// can do, and `l2-game`'s `no_drawn_man_ever_jumps_half_a_cell_in_one_tick`
/// catches it as a 32 px teleport.
#[test]
fn a_swapped_man_stands_and_does_not_step_in_the_same_tick() {
    let mut r =
        BattleRunner::deploy(blank_field(), &[(Troop::Swordsmen, 2)], &[(Troop::Pikemen, 1)]);
    let two: Vec<usize> =
        (0..r.fighters.len()).filter(|&i| r.fighters[i].troop == Troop::Swordsmen).collect();
    let (a, b) = (two[0], two[1]);
    place(&mut r, a, 10, 10);
    place(&mut r, b, 10, 9);
    // Same troop, two destinations: `BattleMen_SwapPlaces` (`0x0049005F`)
    // passes its `cur.tgX == other.tgX && cur.tgY == other.tgY` refusal and
    // answers 2.
    r.fighters[a].target = (10, 5);
    r.fighters[b].target = (11, 5);

    r.enter(a, Pos::new(10, 9));

    assert_eq!((r.fighters[a].x, r.fighters[a].y), (10, 9), "they exchanged cells");
    assert_eq!((r.fighters[b].x, r.fighters[b].y), (10, 10));
    assert_eq!(r.fighters[b].delay, (b & 1) as u8 + 1, "`delay = (other & 1) + 1`");
    assert_eq!(r.fighters[a].delay, 0, "only the man pushed aside waits");

    let held = (r.fighters[b].x, r.fighters[b].y);
    r.step_one(b);
    assert_eq!((r.fighters[b].x, r.fighters[b].y), held, "he did not move on his own turn");
    assert_eq!(r.fighters[b].anim, Motion::Idle, "state 1 stands");
}
