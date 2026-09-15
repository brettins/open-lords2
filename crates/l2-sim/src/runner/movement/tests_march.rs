//! The three things the player saw on the first build of the side-step, as
//! tests: men stopping short of the enemy, men walking backwards, one man
//! jittering between two cells. Each drives `BattleRunner::step_one` — the
//! decision section of `BattleMan_Step` (`0x0048F1DD`) — tick by tick and
//! reads the cells the renderer would draw.

use crate::runner::*;
use crate::Troop;

/// Four men a side on an empty field.
fn two_columns() -> BattleRunner {
    BattleRunner::deploy(
        blank_field(),
        &[(Troop::Swordsmen, 4)],
        &[(Troop::Pikemen, 4)],
    )
}

fn side_figures(r: &BattleRunner, side: Side) -> Vec<usize> {
    (0..r.fighters.len()).filter(|&i| r.fighters[i].side == side).collect()
}

/// Put figure `i` on `(x, y)` with the occupant grid kept true.
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

/// A column of attackers at `x = 10` and a column of defenders at `x = 20`,
/// four rows apiece. `targets` says where each attacker is sent.
fn face_off(targets: impl Fn(usize) -> (u8, u8)) -> (BattleRunner, Vec<usize>) {
    let mut r = two_columns();
    let a = side_figures(&r, SIDE_A);
    let b = side_figures(&r, SIDE_B);
    for (n, &i) in a.iter().enumerate() {
        place(&mut r, i, 10, 10 + n as u8);
        r.fighters[i].target = targets(n);
    }
    for (n, &i) in b.iter().enumerate() {
        place(&mut r, i, 20, 10 + n as u8);
        r.fighters[i].target = (20, 10 + n as u8);
    }
    (r, a)
}

/// Tick the attackers `ticks` times, recording each one's cell every tick.
/// The defenders are never stepped, so they stand as a wall.
fn march(r: &mut BattleRunner, a: &[usize], ticks: usize) -> Vec<Vec<(u8, u8)>> {
    let mut trail: Vec<Vec<(u8, u8)>> = vec![Vec::new(); a.len()];
    for _ in 0..ticks {
        for (n, &i) in a.iter().enumerate() {
            r.step_one(i);
            trail[n].push((r.fighters[i].x, r.fighters[i].y));
        }
    }
    trail
}

fn cheb(p: (u8, u8), q: (u8, u8)) -> i32 {
    chebyshev(p.0 as i16, p.1 as i16, q.0 as i16, q.1 as i16)
}

/// The cells a man actually landed on, in order, without the repeats a
/// sub-cell crossing puts in the trail.
fn landings(t: &[(u8, u8)]) -> Vec<(u8, u8)> {
    let mut out: Vec<(u8, u8)> = Vec::new();
    for &c in t {
        if out.last() != Some(&c) {
            out.push(c);
        }
    }
    out
}

/// **Stopping short.** Each man is sent at the enemy opposite him over ten
/// cells of open field; every one of them must end in contact. This is the
/// test `Path_DetourTooLong` (`0x00472227`) has to stay under: the give-up
/// writes `tgX, tgY = mapX, mapY`, and a man who gives up on a clear line
/// stands in the middle of the field.
#[test]
fn every_man_with_a_clear_line_closes_to_contact() {
    let (mut r, a) = face_off(|n| (20, 10 + n as u8));
    let trail = march(&mut r, &a, 600);
    for (n, &i) in a.iter().enumerate() {
        let end = *trail[n].last().unwrap();
        assert!(
            cheb(end, (20, 10 + n as u8)) <= 1,
            "man {n} stopped at {end:?}, short of the enemy at {:?}",
            (20, 10 + n as u8)
        );
        assert!(r.fighters[i].target != (end.0, end.1) || cheb(end, (20, 10 + n as u8)) <= 1);
    }
}

/// **Walking backwards.** A man may be pushed one cell the wrong way — the
/// side-step's fifth round is `dir ± 4`, the direction away from the target —
/// but never twice running. Two consecutive landings that both increase the
/// distance to his own target is a man walking away.
#[test]
fn no_man_retreats_from_his_target_on_two_landings_running() {
    for (label, targets) in [
        ("one man each", (|n: usize| (20u8, 10 + n as u8)) as fn(usize) -> (u8, u8)),
        ("all at one cell", (|_| (20u8, 12u8)) as fn(usize) -> (u8, u8)),
    ] {
        let (mut r, a) = face_off(targets);
        let goals: Vec<(u8, u8)> = a.iter().map(|&i| r.fighters[i].target).collect();
        let trail = march(&mut r, &a, 600);
        for (n, t) in trail.iter().enumerate() {
            let cells = landings(t);
            let mut away = 0;
            for w in cells.windows(2) {
                away = match cheb(w[1], goals[n]) > cheb(w[0], goals[n]) {
                    true => away + 1,
                    false => 0,
                };
                assert!(away <= 1, "{label}: man {n} walked backwards {w:?}, trail {cells:?}");
            }
        }
    }
}

/// **The jitter, in a whole battle.** Two armies marched at each other by the
/// AI for 1,200 ticks, the fixture `battle_picture`'s `march()` builds. The
/// swap arm of `BattleMan_Step` (`0x0048F1DD`) is what a blocked pair reaches
/// here, and an exchange every other tick is both a jitter and a 32 px drawn
/// jump.
#[test]
fn no_man_alternates_between_two_cells_in_a_fought_battle() {
    let mut r = BattleRunner::deploy(
        blank_field(),
        &[(Troop::Swordsmen, 8), (Troop::Pikemen, 6)],
        &[(Troop::Macemen, 8), (Troop::Peasants, 6)],
    );
    let mut trail: Vec<Vec<(u8, u8)>> = vec![Vec::new(); r.fighters.len()];
    for _ in 0..1_200 {
        for (i, t) in trail.iter_mut().enumerate() {
            if r.is_alive(i) {
                let c = (r.fighters[i].x, r.fighters[i].y);
                if t.last() != Some(&c) {
                    t.push(c);
                }
            }
        }
        r.step();
    }
    let mut jitters = 0;
    for cells in trail.iter() {
        for w in cells.windows(4) {
            if w[0] == w[2] && w[1] == w[3] && w[0] != w[1] {
                jitters += 1;
            }
        }
    }
    assert_eq!(jitters, 0, "men exchanging two cells over and over");
}

/// **The jitter.** A man who alternates two cells — A, B, A, B — is stepping
/// and being pushed back every tick. Four landings on two cells is the
/// shortest run that shows it.
#[test]
fn no_man_alternates_between_two_cells() {
    for targets in [
        (|n: usize| (20u8, 10 + n as u8)) as fn(usize) -> (u8, u8),
        (|_| (20u8, 12u8)) as fn(usize) -> (u8, u8),
    ] {
        let (mut r, a) = face_off(targets);
        let trail = march(&mut r, &a, 600);
        for (n, t) in trail.iter().enumerate() {
            let cells = landings(t);
            for w in cells.windows(4) {
                assert!(
                    !(w[0] == w[2] && w[1] == w[3] && w[0] != w[1]),
                    "man {n} jittered between {:?} and {:?}, trail {cells:?}",
                    w[0],
                    w[1]
                );
            }
        }
    }
}
