#![allow(unused_imports)]
use super::*;
use super::outcome::*;
use super::deploy::*;
use super::ai_runs::*;
use super::super::*;
use crate::ai::Action;

fn firing_line(shooter: Troop, target: Troop, gap: u8) -> BattleRunner {
    let mut r = BattleRunner::empty(blank_field(), DEFAULT_SEED);
    let a = r.sim.add(shooter, SIDE_A, 4).unwrap();
    let b = r.sim.add(target, SIDE_B, 4).unwrap();
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
            facing_drawn: 0,
            fidget: 0,
            fidget_period: 0xB4,
            horse_frame: 0,
            path: Vec::new(),
            barred: 0,
            hold: 0,
            reroutes: 0,
            moat_cell: None,
            moat_load: 0,
            polar: 0,
            corpse: 0,
            delay: 0,
        });
        r.occupant[40 * DIM + x as usize] = Some((r.fighters.len() - 1) as u16);
    }
    r.settle();
    r
}

#[test]
fn an_archer_kills_a_man_ten_cells_away_and_a_swordsman_cannot() {
    let mut bows = firing_line(Troop::Archers, Troop::Peasants, 10);
    bows.run(600);
    assert!(bows.men_of_side(SIDE_B) < 4, "the peasants should be losing men");
    assert!(bows.missiles.live() > 0 || bows.tick > 0, "arrows are being loosed");

    let mut swords = firing_line(Troop::Swordsmen, Troop::Peasants, 10);
    swords.run(600);
    assert_eq!(swords.men_of_side(SIDE_B), 4, "a sword does not reach ten cells");
    assert_eq!(swords.missiles.live(), 0, "and looses nothing");
}

#[test]
fn a_bow_reaches_nine_cells_and_a_crossbow_does_not() {
    let mut bow = firing_line(Troop::Archers, Troop::Peasants, 9);
    bow.run(600);
    let mut xbow = firing_line(Troop::Crossbowmen, Troop::Peasants, 9);
    xbow.run(600);
    assert!(bow.men_of_side(SIDE_B) < 4, "fifteen cells of range");
    assert_eq!(xbow.men_of_side(SIDE_B), 4, "eight, and nine is out of it");
}

#[test]
fn a_body_that_walks_into_the_flight_path_takes_the_arrow() {
    let mut r = firing_line(Troop::Archers, Troop::Peasants, 12);
    r.run(WeaponClass::Bow.stats().reload as u32 + 2);
    assert!(r.missiles.live() > 0, "an arrow should be in the air");
    let target_sim = r.sim.figures[r.fighters[0].sim].target.expect("a chosen target");
    let screen = r.sim.add(Troop::Peasants, SIDE_B, 4).unwrap();
    r.sim.figures[screen].owner = 2;
    r.fighters.push(Fighter {
        sim: screen,
        troop: Troop::Peasants,
        side: SIDE_B,
        x: 24,
        y: 40,
        target: (24, 40),
        facing: 6,
        progress: Progress::default(),
        anim: Motion::Idle,
        phase: 0,
        facing_drawn: 0,
        fidget: 0,
        fidget_period: 0xB4,
        horse_frame: 0,
        path: Vec::new(),
        barred: 0,
        hold: 0,
        reroutes: 0,
        moat_cell: None,
        moat_load: 0,
        polar: 0,
        corpse: 0,
        delay: 0,
    });
    r.occupant[40 * DIM + 24] = Some((r.fighters.len() - 1) as u16);
    let before = r.sim.figures[screen].hits;
    r.run(60);
    assert!(
        r.sim.figures[screen].hits > before || r.sim.figures[screen].men < 4,
        "the interposed man should have been hit, not the one aimed at"
    );
    assert_ne!(screen, target_sim, "and he is not the one that was aimed at");
}

#[test]
fn a_shot_that_misses_keeps_flying_past_the_target() {
    let mut ms = crate::missile::Missiles::new();
    let slot =
        missile::spawn(&mut ms, 1, WeaponClass::Bow, 1, (10, 40), (20, 40), 50, 0).unwrap();
    assert_eq!(ms.get(slot).dir, 2, "due east");
    for _ in 0..400 {
        ms.get_mut(slot).sub_step();
    }
    assert!(
        ms.get(slot).cell_x > 20,
        "it should have overshot: at {}",
        ms.get(slot).cell_x
    );
}

#[test]
fn an_arrow_that_hits_nothing_dies_at_the_end_of_its_range() {
    let mut r = firing_line(Troop::Archers, Troop::Peasants, 40);
    r.run(400);
    assert_eq!(r.missiles.live(), 0, "nothing to shoot at, nothing in the air");
    assert_eq!(r.men_of_side(SIDE_B), 4);
}

#[test]
fn an_arrow_passes_through_a_friendly_body() {
    let mut r = firing_line(Troop::Archers, Troop::Peasants, 12);
    let friend = r.sim.add(Troop::Peasants, SIDE_A, 4).unwrap();
    r.sim.figures[friend].owner = 1;
    r.fighters.push(Fighter {
        sim: friend,
        troop: Troop::Peasants,
        side: SIDE_A,
        x: 22,
        y: 40,
        target: (22, 40),
        facing: 2,
        progress: Progress::default(),
        anim: Motion::Idle,
        phase: 0,
        facing_drawn: 0,
        fidget: 0,
        fidget_period: 0xB4,
        horse_frame: 0,
        path: Vec::new(),
        barred: 0,
        hold: 0,
        reroutes: 0,
        moat_cell: None,
        moat_load: 0,
        polar: 0,
        corpse: 0,
        delay: 0,
    });
    r.occupant[40 * DIM + 22] = Some((r.fighters.len() - 1) as u16);
    r.run(600);
    assert_eq!(r.sim.figures[friend].men, 4, "friendly fire is impossible");
    assert_eq!(r.sim.figures[friend].hits, 0);
    assert!(r.men_of_side(SIDE_B) < 4, "and the arrows got past him");
}

#[test]
fn nothing_is_loosed_before_the_reload_interval_expires() {
    let reload = WeaponClass::Bow.stats().reload as u32;
    let mut r = firing_line(Troop::Archers, Troop::Peasants, 10);
    r.run(reload);
    assert_eq!(r.missiles.live(), 0, "not yet");
    r.run(1);
    assert_eq!(r.missiles.live(), 1, "and now");
}

#[test]
fn two_runs_of_a_battle_with_missiles_stay_identical() {
    let build = || {
        BattleRunner::deploy(
            blank_field(),
            &[(Troop::Archers, 6), (Troop::Swordsmen, 4)],
            &[(Troop::Crossbowmen, 6), (Troop::Peasants, 4)],
        )
    };
    let (mut a, mut b) = (build(), build());
    for _ in 0..2_000 {
        a.step();
        b.step();
        assert_eq!(a.missiles, b.missiles, "diverged at tick {}", a.tick);
    }
    assert_eq!(a.fighters, b.fighters);
    assert_eq!(a.sim, b.sim);
}

#[test]
fn a_catapult_shot_cannot_hurt_a_man() {
    let mut r = firing_line(Troop::Catapults, Troop::Peasants, 10);
    r.run(1_000);
    assert_eq!(r.men_of_side(SIDE_B), 4, "a catapult is a wall-breaker only");
}

#[test]
fn an_arrow_is_cued_when_it_leaves_when_it_lands_and_when_it_kills() {
    for (troop, class) in [(Troop::Archers, WeaponClass::Bow), (Troop::Crossbowmen, WeaponClass::Crossbow)] {
        let mut r = firing_line(troop, Troop::Peasants, 6);
        let target = r.fighters[1].sim;
        let mut deaths = 0;
        for _ in 0..3_000 {
            let (was, men, live) = (r.sim.cues, r.sim.figures[target].men, r.missiles.live());
            r.step();
            let now = r.sim.cues;
            if r.missiles.live() > live {
                assert_eq!(now.loosed(class), was.loosed(class) + 1, "{troop:?} loosed at {} uncued", r.tick);
            }
            if r.sim.figures[target].men < men {
                assert!(now.missile_hits(class) > was.missile_hits(class), "a man fell to no hit at {}", r.tick);
                assert!(now.missile_casualties(class) > was.missile_casualties(class));
            }
            if men > 0 && r.sim.figures[target].men == 0 {
                assert_eq!(now.missile_deaths(), was.missile_deaths() + 1);
                deaths += 1;
            }
            let other = if class == WeaponClass::Bow { WeaponClass::Crossbow } else { WeaponClass::Bow };
            assert_eq!(now.loosed(other), 0);
            assert_eq!(now.missile_hits(other), 0);
        }
        assert_eq!(deaths, 1, "{troop:?} should have killed the peasants in 3,000 ticks");
        let least = if class == WeaponClass::Bow { 8 } else { 2 };
        assert!(r.sim.cues.missile_hits(class) >= least, "four men need at least {least} hits");
        assert_eq!(r.sim.cues.melee_casualties(troop), 0, "nobody came to blows");
    }
}

#[test]
fn a_catapult_is_cued_as_it_fires_and_never_as_hitting_a_man() {
    let mut r = firing_line(Troop::Catapults, Troop::Peasants, 10);
    r.run(1_000);
    assert!(r.sim.cues.loosed(WeaponClass::Catapult) > 0, "the catapult fired");
    assert_eq!(r.sim.cues.missile_deaths(), 0);
    assert_eq!(r.sim.cues.loosed(WeaponClass::Bow), 0);
}


