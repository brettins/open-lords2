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

/// **The draw is held for the whole reload.** `BattleMan_FireMissile`
/// (`0x00483337`) calls `Anim_DrawBow` at its tail on every tick the figure has
/// a target, and `swingTimer` — [`crate::Figure::reload_counter`] — steps once
/// a tick under it, so `l2_view::drawbow`'s curve walks the three poses.
///
/// **Ablation**: raise the bow on the acquisition tick only, as we did, and
/// `stand` takes the pose back on the very next tick — `held` is 1, and poses
/// 11 and 12 never reach the screen.
#[test]
fn the_bow_stays_drawn_across_the_reload_and_the_swing_timer_walks_the_curve() {
    let mut r = pair(Troop::Archers, Troop::Peasants, 10);
    let sim = r.fighters[0].sim;
    let (mut held, mut swings) = (0u32, std::collections::BTreeSet::new());
    for _ in 0..200 {
        r.run(1);
        if r.fighters[0].anim == Motion::Shooting {
            held += 1;
            swings.insert(r.sim.figures[sim].reload_counter.min(0x4F));
        }
    }
    assert!(held > 30, "the bow was up for {held} ticks of 200");
    // Both curves are flat over any short run; 24 apart is the archer's step
    // from pose 12 to 11, so the drawn pose cannot be constant.
    let (lo, hi) = (*swings.iter().next().unwrap(), *swings.iter().last().unwrap());
    assert!(hi - lo >= 24, "swingTimer only covered {lo}…{hi}");
}

/// **The fidget belongs to `Anim_StandA2` alone** (`0x004872AE`): it is the one
/// function that writes `+0x0B`, and it does not reset it on arrival.
///
/// **Ablation**: reset `fidget` in `march`, `strike` or `shoot`, or write
/// `facing_drawn = facing` on entering idle as we did, and a man who takes one
/// step never reaches his period — a standing rank stops shuffling.
#[test]
fn only_standing_touches_the_fidget_counter() {
    let mut r = pair(Troop::Swordsmen, Troop::Swordsmen, 12);
    r.fighters[0].fidget = 100;
    r.fighters[0].facing_drawn = 5;
    r.march(0, true);
    assert_eq!(r.fighters[0].fidget, 100, "marching reset the counter");
    r.strike(0, 3);
    assert_eq!(r.fighters[0].fidget, 100, "striking reset the counter");
    r.shoot(0);
    assert_eq!(r.fighters[0].fidget, 100, "drawing reset the counter");
    // Standing is the only writer, and it counts up.
    r.fighters[0].facing_drawn = 5;
    r.stand(0);
    assert_eq!(r.fighters[0].fidget, 101);
    assert_eq!(r.fighters[0].facing_drawn, 5, "standing rewrote the drawn facing");
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

/// **A barred man stands where he wanted to go.** `BattleMan_StateWalk`
/// (`0x0048314E`, `00480000.c:1359`) runs `Anim_Walk` before it knows the step
/// was refused, so `facingDrawn` and the walk phase have already moved;
/// `Anim_StandA2` only takes the frame back.
///
/// **Ablation**: returning early from `march` when `!moved`, as this used to,
/// leaves both behind.
#[test]
fn a_refused_step_still_turns_the_drawn_facing() {
    let mut r = pair(Troop::Peasants, Troop::Peasants, 30);
    r.fighters[0].facing_drawn = 7;
    r.fighters[0].facing = 3;
    r.fighters[0].phase = 0;
    r.march(0, false);
    assert_eq!(r.fighters[0].anim, Motion::Idle, "a refused step stands");
    assert_eq!(r.fighters[0].facing_drawn, 3, "Anim_Walk wrote the facing first");
    assert_eq!(r.fighters[0].phase, 1, "and stepped the walk phase");
}

/// **Only the swinging man's phase moves** — `Anim_StrikeA2`'s `animPhase + 1`
/// sits inside `if (role == 1)` (`00480000.c:2509`), and `Anim_StandA2` and
/// `Anim_DrawBowA2` never touch it.
///
/// **Ablation**: the unconditional `phase += 1` this used to have at the top of
/// `step_one` moves the defender's counter and the stander's alike.
#[test]
fn the_defenders_strike_phase_is_frozen_and_a_stander_never_steps_it() {
    let mut r = pair(Troop::Swordsmen, Troop::Swordsmen, 1);
    let (a, b) = (r.fighters[0].sim, r.fighters[1].sim);
    r.sim.figures[a].role = crate::Role::Attacking;
    r.sim.figures[b].role = crate::Role::Defending;
    r.fighters[0].phase = 4;
    r.fighters[1].phase = 4;
    for _ in 0..6 {
        r.strike(0, 2);
        r.strike(1, 6);
    }
    assert_eq!(r.fighters[0].phase, 10, "the attacker swings");
    assert_eq!(r.fighters[1].phase, 4, "the defender's cycle is frozen");
    r.fighters[0].phase = 7;
    for _ in 0..6 {
        r.stand(0);
        r.shoot(0);
    }
    assert_eq!(r.fighters[0].phase, 7, "standing and drawing leave it alone");
}

/// **A wall-batterer swings.** `BattleMan_StateAttackWall` (`0x00483A88`) sets
/// `role = 1` on the line before `Anim_Strike` (`00480000.c:1556`).
///
/// **Ablation**: dropping that line leaves him at the default
/// [`crate::Role::Defending`], which the renderer draws standing.
#[test]
fn a_man_hitting_a_wall_takes_the_attacking_role() {
    let mut r = pair(Troop::Swordsmen, Troop::Swordsmen, 30);
    let sim = r.fighters[1].sim;
    r.sim.figures[sim].role = crate::Role::Defending;
    let (x, y) = (r.fighters[1].x, r.fighters[1].y);
    let dst = y as usize * DIM + (x as usize + 1);
    r.field.cells[dst].flags |= crate::siege::FLAG_WALL;
    r.field.cells[y as usize * DIM + x as usize].surface = crate::siege::SURFACE_BAILEY;
    assert!(r.strike_castle(1, dst), "the wall arm did not run");
    assert_eq!(r.sim.figures[sim].role, crate::Role::Attacking);
    assert_eq!(r.fighters[1].anim, Motion::Attacking);
}

/// **No engine runs a man's cycle.** `Anim_StrikeA2` (`00480000.c:2477`) and
/// `Anim_WalkA2` (`2773`) gate on `troopType < 7`; over 7 they call
/// `FUN_00488793` / `FUN_00488436`, which write no `facingDrawn` and step
/// `animPhase` for the oil pot alone, in both (`3364`, `3327`) — `+= 1`
/// **clamped** at 0x2F, not `% 48`, and the seed `(index * 9 + x * 16) & 0x3F`
/// reaches 63. A ram's counter never moves.
#[test]
fn a_siege_engine_keeps_its_counter_out_of_the_walk_and_strike_arms() {
    let mut r = pair(Troop::BatteringRams, Troop::Oil, 4);
    for i in [0, 1] {
        r.fighters[i].phase = 7;
        r.fighters[i].facing_drawn = 2;
    }
    r.strike(0, 5);
    r.march(1, true);
    assert_eq!((r.fighters[0].anim, r.fighters[0].phase), (Motion::Attacking, 7));
    assert_eq!((r.fighters[1].anim, r.fighters[1].phase), (Motion::Walking, 8), "the pot bubbles while it walks");
    assert_eq!((r.fighters[0].facing_drawn, r.fighters[1].facing_drawn), (2, 2));
    r.fighters[1].phase = 63;
    r.stand(1);
    assert_eq!(r.fighters[1].phase, 0, "0x2F is a clamp, not a wrap");
    r.stand(1);
    assert_eq!(r.fighters[1].phase, 1);
}
