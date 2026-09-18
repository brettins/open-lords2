//! **`BattleMan_Step`'s friendly arm** (`0x0048F1DD`, `local_10 == 0`) and the
//! verdict it asks for, `BattleMen_SwapPlaces` (`0x0049005F`): who swaps, who
//! waits, and who falls through to the side-step.
//!
//! Each refusal has its own test, because the arm only holds together whole:
//! landed one at a time, each was measured against the fought seam's verdict
//! and each looked like a loss. Q2 retired that yardstick — the seam's verdict
//! is the autocalc's (`FUN_0043B622`) — so the decompilation is the only
//! authority these tests answer to.

use crate::figure::State;
use crate::runner::*;
use crate::Troop;

/// One unit of two swordsmen against a pikeman, on a bare field.
fn two_swordsmen() -> (BattleRunner, usize, usize) {
    let mut r = BattleRunner::deploy(
        blank_field(),
        &[(Troop::Swordsmen, 2)],
        &[(Troop::Pikemen, 1)],
    );
    let two: Vec<usize> =
        (0..r.fighters.len()).filter(|&i| r.fighters[i].troop == Troop::Swordsmen).collect();
    let (a, b) = (two[0], two[1]);
    assert_eq!(r.unit_of(a), r.unit_of(b), "one unit of swordsmen");
    // `if (other.ownerIsHuman == 0 && !siege) return 1;` — the head of
    // `BattleMen_SwapPlaces`. Outside a siege only the player's men swap.
    for f in [a, b] {
        let s = r.fighters[f].sim;
        r.sim.figures[s].owner_is_human = true;
    }
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
    r.occupant[y as usize * DIM + x as usize] = Some(i as u16);
}

/// Walk `a` into `b`'s cell through the mover, which is where the arm is.
fn walk_into(r: &mut BattleRunner, a: usize, b: usize) {
    let (x, y) = (r.fighters[b].x, r.fighters[b].y);
    r.enter(a, Pos::new(x, y));
}

/// **Two men of one unit going different ways exchange cells.** The one and
/// only path to `uVar6 = 2`: same troop type, both under 7, `other` in no
/// refusing state, and the two targets apart.
#[test]
fn men_of_one_unit_with_different_targets_swap() {
    let (mut r, a, b) = two_swordsmen();
    r.fighters[a].target = (10, 5);
    r.fighters[b].target = (40, 40);

    walk_into(&mut r, a, b);

    assert_eq!((r.fighters[a].x, r.fighters[a].y), (10, 9), "they exchanged cells");
    assert_eq!((r.fighters[b].x, r.fighters[b].y), (10, 10));
}

/// **An AI-owned man is never swapped with in a field battle.**
/// `00490000.c:21-23`: `if (other.ownerIsHuman == 0) { if (!siege) return 1; …`
/// — answer 1, which is `local_10 = 3` and the side-step, not the wait. The
/// two guards under it (`cur.side == 4`, `cur.weaponClass == 0`) are a siege's
/// only, and no field battle reaches them.
///
/// **The ablation.** Drop the head of [`BattleRunner::swap_verdict`] and the
/// two AI men exchange cells here.
#[test]
fn an_ai_owned_comrade_is_not_swapped_with_on_a_field() {
    let (mut r, a, b) = two_swordsmen();
    for f in [a, b] {
        let s = r.fighters[f].sim;
        r.sim.figures[s].owner_is_human = false;
    }
    assert!(!r.siege.is_siege, "a field battle");
    r.fighters[a].target = (10, 5);
    r.fighters[b].target = (40, 40);

    walk_into(&mut r, a, b);

    assert_eq!((r.fighters[b].x, r.fighters[b].y), (10, 9), "he kept his cell");
    assert_eq!(r.fighters[a].delay, 0, "answer 1 is the side-step, not the wait");
}

/// **A man swapped out of his cell stands for a frame or two** —
/// `other.state = 1; other.delayState = 3; other.delay = (other & 1) + 1`.
///
/// **The ablation.** Drop the `delay` write in [`BattleRunner::swap_places`]
/// and he takes his own step in the same tick he was pushed aside — two cells
/// in one frame, which nothing in the original can do, and `l2-game`'s
/// `no_drawn_man_ever_jumps_half_a_cell_in_one_tick` catches it as a 32 px
/// teleport.
#[test]
fn the_man_pushed_aside_waits_a_frame() {
    let (mut r, a, b) = two_swordsmen();
    r.fighters[a].target = (10, 5);
    r.fighters[b].target = (40, 40);

    walk_into(&mut r, a, b);

    assert_eq!(r.fighters[b].delay, (b & 1) as u8 + 1, "`delay = (other & 1) + 1`");
    assert_eq!(r.fighters[a].delay, 0, "only the man pushed aside waits");
    let held = (r.fighters[b].x, r.fighters[b].y);
    r.step_one(b);
    assert_eq!((r.fighters[b].x, r.fighters[b].y), held, "he did not move on his own turn");
    assert_eq!(r.fighters[b].anim, Motion::Idle, "state 1 stands");
}

/// **Two men walking to the same cell do not swap; the blocked one waits.**
/// `if (cur.tgX == other.tgX && cur.tgY == other.tgY) { FUN_004928A3();
/// return 0; }` — the call an empty stub — and `BattleMan_Step` turns that 0
/// into `cur.state = 1; cur.delay = (other & 1) + 1`.
///
/// **The ablation.** This is the test ours had the *other* way round: swap on
/// a shared target and a marching unit churns through itself, the men trading
/// places every frame instead of closing on the enemy.
#[test]
fn a_shared_target_refuses_the_swap_and_the_mover_waits() {
    let (mut r, a, b) = two_swordsmen();
    r.fighters[a].target = (10, 5);
    r.fighters[b].target = (10, 5);

    walk_into(&mut r, a, b);

    assert_eq!((r.fighters[a].x, r.fighters[a].y), (10, 10), "he stood");
    assert_eq!((r.fighters[b].x, r.fighters[b].y), (10, 9), "and so did his comrade");
    assert_eq!(r.fighters[a].delay, (b & 1) as u8 + 1, "`cur.delay = (other & 1) + 1`");
    assert!(r.fighters[a].path.is_empty(), "`cur.onRoute = 0`, and no route asked for");
}

/// **State 8 — `Chasing` — refuses the swap.** `00490000.c:41-42`, above the
/// target test: a man in free pursuit (`BattleMan_StateChase`, `0x00483E55`) is
/// never pulled backwards out of his cell.
///
/// **The ablation.** Delete the `State::Chasing` arm of
/// [`BattleRunner::swap_verdict`] and this pair swaps.
#[test]
fn a_chasing_comrade_refuses_the_swap() {
    let (mut r, a, b) = two_swordsmen();
    r.fighters[a].target = (10, 5);
    r.fighters[b].target = (40, 40);
    let sim_b = r.fighters[b].sim;
    r.sim.figures[sim_b].state = State::Chasing;

    walk_into(&mut r, a, b);

    assert_eq!((r.fighters[a].x, r.fighters[a].y), (10, 10), "state 8 is not swapped with");
    assert_eq!(r.fighters[a].delay, (b & 1) as u8 + 1, "so the mover waits instead");
}

/// **State 3 — walking — refuses the swap.** The original never yanks a man
/// out of a cell he is half-way into; [`Progress::free`] is our reading of it
/// (`[D]` on the reading, **[V]** on the refusal).
#[test]
fn a_comrade_mid_crossing_refuses_the_swap() {
    let (mut r, a, b) = two_swordsmen();
    r.fighters[a].target = (10, 5);
    r.fighters[b].target = (40, 40);
    r.fighters[b].progress.begin_crossing();
    assert!(!r.fighters[b].progress.free);

    walk_into(&mut r, a, b);

    assert_eq!((r.fighters[a].x, r.fighters[a].y), (10, 10), "state 3 is not swapped with");
    assert_eq!(r.fighters[a].delay, (b & 1) as u8 + 1);
}

/// **A corpse — state 2 — is neither swapped with nor waited for.** The
/// refusal is `BattleMan_Step`'s outer guard, `if (other.state != 2 && cur.unit
/// == other.unit)`, which a dead comrade fails outright: the arm is skipped and
/// `local_10 = 3` side-steps him.
///
/// **The ablation.** Drop `is_alive` from [`BattleRunner::friendly_step`]'s
/// guard and the living man stands still in front of a body for ever.
#[test]
fn a_dead_comrade_is_side_stepped_not_waited_for() {
    let (mut r, a, b) = two_swordsmen();
    r.fighters[a].target = (10, 9);
    r.fighters[b].target = (40, 40);
    let sim_b = r.fighters[b].sim;
    r.sim.figures[sim_b].men = 0;
    r.sim.figures[sim_b].state = State::Dead;

    walk_into(&mut r, a, b);

    assert_eq!(r.fighters[a].delay, 0, "no wait for a body");
    assert_ne!((r.fighters[a].x, r.fighters[a].y), (10, 10), "he shuffled past it");
}

/// **A blocker from another unit falls straight through to the side-step.**
/// The outer guard is `cur.unit == other.unit`; an archer of another unit
/// never reaches `BattleMen_SwapPlaces` at all.
#[test]
fn a_comrade_of_another_unit_is_side_stepped() {
    let mut r = BattleRunner::deploy(
        blank_field(),
        &[(Troop::Swordsmen, 1), (Troop::Archers, 1)],
        &[(Troop::Pikemen, 1)],
    );
    let find = |t: Troop| r.fighters.iter().position(|f| f.troop == t).unwrap();
    let (a, b) = (find(Troop::Swordsmen), find(Troop::Archers));
    assert_ne!(r.unit_of(a), r.unit_of(b), "two units");
    place(&mut r, a, 10, 10);
    place(&mut r, b, 10, 9);
    r.fighters[a].target = (10, 9);
    r.fighters[b].target = (40, 40);

    walk_into(&mut r, a, b);

    assert_eq!(r.fighters[a].delay, 0, "no wait across units");
    assert_ne!((r.fighters[a].x, r.fighters[a].y), (10, 10), "he shuffled round him");
}
