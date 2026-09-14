//! The pose machine, driven by whole ticks — [`super::anim`].
//!
//! Each of the four symptoms the player reported on 2026-09-14 has a test here
//! and an ablation beside it that fails without the fix.

use super::*;
use crate::terrain::Battlefield;
use super::blank_field;

const SEED: u64 = 0x5EED;

fn field() -> Battlefield {
    blank_field()
}

/// One figure of each troop, `gap` cells apart on empty ground, facing each
/// other and already standing on their destinations — so the only things that
/// can happen are the ones under test.
fn pair(a_troop: Troop, b_troop: Troop, gap: u8) -> BattleRunner {
    let mut r = BattleRunner::empty(field(), SEED);
    let a = r.sim.add(a_troop, SIDE_A, 4).unwrap();
    let b = r.sim.add(b_troop, SIDE_B, 4).unwrap();
    r.sim.figures[a].owner = 1;
    r.sim.figures[b].owner = 2;
    for (sim, x) in [(a, 20u8), (b, 20 + gap)] {
        r.fighters.push(Fighter {
            sim,
            troop: r.sim.figures[sim].troop,
            side: r.sim.figures[sim].side,
            x,
            y: 40,
            target: (x, 40),
            facing: 2,
            progress: Progress::default(),
            anim: Motion::Idle,
            phase: 0,
            facing_drawn: 2,
            fidget: 0,
            fidget_period: 0xB4,
            path: Vec::new(),
            barred: 0,
            hold: 0,
            reroutes: 0,
            moat_cell: None,
            moat_load: 0,
            polar: 0,
            corpse: 0,
        });
        r.occupant[40 * DIM + x as usize] = Some((r.fighters.len() - 1) as u16);
    }
    r.settle();
    r
}

/// **The stuck attack.** Two men adjacent lock into a duel; kill one from
/// outside the melee and the survivor must put his sword down.
///
/// Ten ticks is the whole budget: the mutual check runs at the top of
/// `step_one`, so the tick after the opponent stops being alive is the tick the
/// pose goes back to standing.
#[test]
fn a_man_whose_opponent_dies_stops_striking_within_ten_ticks() {
    let mut r = pair(Troop::Swordsmen, Troop::Swordsmen, 1);
    for _ in 0..40 {
        r.run(1);
        if r.fighters[0].anim == Motion::Attacking {
            break;
        }
    }
    assert_eq!(r.fighters[0].anim, Motion::Attacking, "they never engaged");

    // The killing blow lands from somewhere else — a missile, a third man —
    // which is the case `melee::tick` does not release, and the case the player
    // saw.
    let victim = r.fighters[1].sim;
    r.sim.figures[victim].men = 0;
    r.sim.figures[victim].state = crate::State::Dead;
    for _ in 0..10 {
        r.run(1);
        if r.fighters[0].anim != Motion::Attacking {
            break;
        }
    }
    assert_ne!(r.fighters[0].anim, Motion::Attacking, "still hacking at a corpse");
    assert_eq!(r.fighters[0].anim, Motion::Idle);
}

/// **The archer's bow.** `BattleMan_FireMissile` acquires ten ticks before the
/// interval expires and looses when it does, so the draw must be showing while
/// the reload counter sits inside that window and the arrow must not be in the
/// air before it.
#[test]
fn an_archer_enters_shooting_before_the_arrow_is_loosed() {
    let mut r = pair(Troop::Archers, Troop::Peasants, 10);
    let mut drew_at = None;
    let mut loosed_at = None;
    for t in 0..200 {
        r.run(1);
        if drew_at.is_none() && r.fighters[0].anim == Motion::Shooting {
            drew_at = Some(t);
        }
        if loosed_at.is_none() && r.missiles.iter().next().is_some() {
            loosed_at = Some(t);
        }
    }
    let drew = drew_at.expect("the archer never drew his bow");
    let loosed = loosed_at.expect("the archer never shot");
    // Acquisition is at `counter == reload - 10` and the loose at
    // `counter > reload`, so the draw is those eleven ticks and no longer.
    let window = loosed - drew;
    assert!(drew < loosed, "drew at {drew}, loosed at {loosed}");
    assert!(
        (1..=crate::missile::ACQUIRE_LEAD as i32 + 1).contains(&window),
        "the draw is the acquisition window, not {window} ticks"
    );
}

/// **The jitter.** Over a whole battle no figure may change pose on two
/// consecutive ticks more than a handful of times — a pose that flips every
/// tick is the symptom, and the walk cycle itself is a *frame* change,
/// pose change, so it does not count here.
///
/// **Ablation**: writing `Motion::Walking` before the route is asked for, as
/// this used to, makes a barred figure alternate walk/stand with the
/// pathfinder's cadence and blows the bound.
#[test]
fn no_figure_alternates_pose_tick_by_tick() {
    let mut r = pair(Troop::Swordsmen, Troop::Pikemen, 12);
    let n = r.fighters.len();
    let mut last: Vec<Motion> = r.fighters.iter().map(|f| f.anim).collect();
    let mut flips = vec![0u32; n];
    let mut changed_last_tick = vec![false; n];
    for _ in 0..600 {
        r.run(1);
        for i in 0..n {
            let now = r.fighters[i].anim;
            let changed = now != last[i];
            if changed && changed_last_tick[i] {
                flips[i] += 1;
            }
            changed_last_tick[i] = changed;
            last[i] = now;
        }
    }
    let worst = flips.iter().max().copied().unwrap_or(0);
    assert!(worst <= 2, "a figure changed pose on consecutive ticks {worst} times");
}

/// **The stuck walk**, from the other side: a figure standing on its own
/// destination with nothing to fight is never drawn marching.
#[test]
fn a_man_on_his_destination_never_marches() {
    let mut r = pair(Troop::Peasants, Troop::Peasants, 30);
    for _ in 0..120 {
        r.run(1);
        assert_ne!(
            r.fighters[0].anim,
            Motion::Walking,
            "a man who has arrived marched on the spot"
        );
    }
}
