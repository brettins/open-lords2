#![allow(unused_imports)]
use super::*;
use super::oil_and_rampart_tests::*;
use super::bridge_and_tower_tests::*;
use super::*;
use crate::cue::Cues;
use crate::fire::{SURFACE_BRIDGE, SURFACE_BURNING, SURFACE_WOODLAND, SURFACE_WOOD_BURNING, SURFACE_WOOD_CATCHING};
use crate::proving;
use crate::terrain::flag;

/// A wood of 7 × 5 round `(40, 42)` with four of a human's peasants in it,
/// and an AI garrison's archer on open ground to the north.
fn a_wood_with_men_in_it(men: usize) -> (BattleRunner, usize) {
    let mut field = blank_field();
    for y in 40..=44 {
        for x in 37..=43 {
            field.cells[cell(x, y)].surface = SURFACE_WOODLAND;
        }
    }
    let mut r = BattleRunner::empty(field, DEFAULT_SEED);
    // The men first, so the sweep has counted them before the archer looses.
    for k in 0..men {
        stand(&mut r, Troop::Peasants, SIDE_B, 1, true, (38 + k as u8, 42));
    }
    let archer = stand(&mut r, Troop::Archers, SIDE_A, 2, false, (40, 30));
    let unit = r.units.create(2, false, SIDE_A, 1).unwrap();
    let sim = r.fighters[archer].sim;
    r.sim.figures[sim].unit = unit as u16;
    r.settle();
    (r, archer)
}

/// **An AI garrison shoots fire arrows at a human army hiding in a wood, and
/// the whole wood burns** — `BattleMan_FireMissile`'s `+0x44`, `Missile_Step`'s
/// first test, `FUN_00485861` and `FUN_004859E5`.
///
/// Ablation, carried inside: three men in the wood is not *more than three*,
/// and the same archer's arrows are only arrows.
#[test]
fn fire_arrows_set_a_wood_alight_under_a_human_army_and_it_burns_through() {
    let (mut r, _) = a_wood_with_men_in_it(4);
    let mut caught = false;
    for _ in 0..400 {
        r.step();
        if r.wood_fire {
            caught = true;
            break;
        }
    }
    assert!(caught, "a fire arrow lit the wood");
    // `FUN_004859E5` runs in the same frame as the arrow: the cell it lit is
    // already burning, and its woodland neighbours are catching — and nothing
    // else.
    let with = |s: u8| (0..DIM * DIM).filter(|&c| r.field.cells[c].surface == s).collect::<Vec<_>>();
    let burning = with(SURFACE_WOOD_BURNING);
    assert_eq!(burning.len(), 1, "one cell, where the arrow started its frame");
    let (bx, by) = ((burning[0] % DIM) as i32, (burning[0] / DIM) as i32);
    let mut expect: Vec<usize> = [(0, -1), (1, 0), (0, 1), (-1, 0)]
        .into_iter()
        .map(|(dx, dy)| ((by + dy) as usize) * DIM + (bx + dx) as usize)
        .filter(|&c| (37..=43).contains(&(c % DIM)) && (40..=44).contains(&(c / DIM)))
        .collect();
    expect.sort_unstable();
    assert_eq!(with(SURFACE_WOOD_CATCHING), expect, "its neighbours in the wood, one ring");

    // Seven columns wide and five deep: every cell burning within a handful of
    // frames, and the men in it burning.
    for _ in 0..12 {
        r.step();
    }
    let wood = (40..=44).flat_map(|y| (37..=43).map(move |x| cell(x, y)));
    for c in wood {
        assert_eq!(r.field.cells[c].surface, SURFACE_WOOD_BURNING, "cell {c}");
    }
    let hurt = (0..4).filter(|&i| {
        let f = &r.sim.figures[r.fighters[i].sim];
        f.hits > 0 || f.men < 4
    });
    assert_eq!(hurt.count(), 4, "every man in the wood is burning");
    assert_eq!(r.sim.cues.missile_hits(WeaponClass::Bow), 0, "and not one was hit by an arrow");

    for _ in 0..700 {
        r.step();
    }
    assert!(!r.wood_fire, "the spread stopped");
    assert!(
        (40..=44).flat_map(|y| (37..=43).map(move |x| cell(x, y))).all(|c| r.field.cells[c].surface == 0),
        "a burnt wood is surface 0"
    );

    let (mut three, _) = a_wood_with_men_in_it(3);
    three.run(400);
    assert!(!three.wood_fire, "three is not more than three");
    assert!(three.sim.cues.loosed(WeaponClass::Bow) > 0, "though the archer shot");
}

// ------------------------------------------------------------------ determinism

/// **The cue record cannot feed back into a siege that burns.** C166's proof
/// — two copies, one's record wiped every frame, every other field equal —
/// over the proving ground, which pours oil, docks a tower, burns a bridge,
/// kills men with fire and bounces catapult shots off a wall four high.
///
/// Ablation, by insertion: make
/// `update_man` skip the burn while `self.sim.cues.oil_poured() > 0` and this
/// goes red on the frame after the pour.
#[test]
fn a_siege_whose_cues_are_wiped_every_tick_is_the_same_siege() {
    let (mut heard, mut wiped) = (proving::deploy(), proving::deploy());
    for _ in 0..2_000u32 {
        proving::orders(&mut heard);
        proving::orders(&mut wiped);
        wiped.sim.cues = Cues::default();
        heard.step();
        wiped.step();
        let mut same = wiped.clone();
        same.sim.cues = heard.sim.cues;
        assert_eq!(heard, same, "the cues changed the siege at tick {}", heard.tick);
    }
    let c = heard.sim.cues;
    assert_eq!(c.oil_poured(), 1, "{c:?}");
    assert_eq!(c.towers_docked(), 1, "{c:?}");
    assert!(c.bridges_fired() >= 1, "{c:?}");
    assert!(c.burn_deaths(SIDE_B) >= 2, "{c:?}");
    assert!(c.walls_missed() >= 10, "{c:?}");
}

/// Two runs of the proving ground are the same run.
#[test]
fn two_runs_of_the_proving_ground_are_identical() {
    assert_eq!(proving::run(1_500), proving::run(1_500));
}

// ------------------------------------------------- the player's fire arrow

/// One human archer of side 0 north of the wood, its own unit, and one enemy
/// figure standing in the wood at `(40, 42)` — so an order onto that cell both
/// writes `targetCell` and puts the archer into state 17.
fn a_player_archer_and_a_man_in_the_wood() -> (BattleRunner, usize, usize) {
    let mut field = blank_field();
    for y in 40..=44 {
        for x in 37..=43 {
            field.cells[cell(x, y)].surface = SURFACE_WOODLAND;
        }
    }
    let mut r = BattleRunner::empty(field, DEFAULT_SEED);
    let archer = stand(&mut r, Troop::Archers, SIDE_A, 1, true, (40, 34));
    let unit = r.units.create(1, true, SIDE_A, 1).unwrap();
    let sim = r.fighters[archer].sim;
    r.sim.figures[sim].unit = unit as u16;
    stand(&mut r, Troop::Peasants, SIDE_B, 2, false, (40, 42));
    r.settle();
    (r, unit, archer)
}

/// **The player's fire arrow** — `BattleUnit_Order` (`0x00479E90`) writes
/// `targetCell` under its fifth argument, `BattleMan_StateCloseToAttack`
/// (`0x00484BF9`) copies it onto every arrow it looses, and `Missile_Step`'s
/// first test lights **that one cell** and clears `targetCell` again.
///
/// The order is the click on a woodland cell: `DAT_0053E874` set, no enemy
/// under the cursor. `targetCell` is the original's byte offset
/// `(y * 80 + x) * 8`.
///
/// Ablation: pass `woodland = false` — carried inside — and `targetCell` is 0,
/// the arrows carry `+0x44 = 0`, and no wood ever catches.
#[test]
fn an_ordered_volley_lights_the_cell_it_was_aimed_at_and_a_figure_in_it_burns() {
    let (mut r, unit, archer) = a_player_archer_and_a_man_in_the_wood();
    r.order_full(unit, 40, 42, None, true, Formation::Keep);
    assert_eq!(
        r.units.get(unit).target_cell,
        crate::fire::cell_byte_offset(40, 42),
        "(42 * 80 + 40) * 8"
    );

    let mut caught = false;
    for _ in 0..600 {
        r.step();
        if r.wood_fire {
            caught = true;
            break;
        }
    }
    assert!(caught, "the ordered volley lit the wood");
    assert_eq!(
        r.sim.figures[r.fighters[archer].sim].state,
        State::Shooting,
        "state 17 is what looses a fire arrow"
    );
    // `FUN_004859E5` has already run this frame, so the cell the arrow lit is
    // burning and its ring is catching — one cell was struck, not five.
    let lit: Vec<usize> =
        (0..DIM * DIM).filter(|&c| r.field.cells[c].surface == SURFACE_WOOD_BURNING).collect();
    assert_eq!(lit, vec![cell(40, 42)], "only the cell the order named");
    assert_eq!(r.units.get(unit).target_cell, 0, "Missile_Step clears it: one volley, one cell");

    // The man standing in it burns — `BattleMan_BurnTick`, three hits a frame
    // at size class 0 and no fourth because his owner is not a human.
    let man = r.fighters.iter().position(|f| f.side == SIDE_B).unwrap();
    let before = r.sim.figures[r.fighters[man].sim].hits;
    for _ in 0..10 {
        r.step();
    }
    let after = r.sim.figures[r.fighters[man].sim].hits;
    assert!(after > before, "a figure on the burning cell takes hits: {before} -> {after}");

    // Ablation: the same order without the woodland argument.
    let (mut plain, unit, _) = a_player_archer_and_a_man_in_the_wood();
    plain.order_full(unit, 40, 42, None, false, Formation::Keep);
    assert_eq!(plain.units.get(unit).target_cell, 0);
    for _ in 0..600 {
        plain.step();
    }
    assert!(!plain.wood_fire, "no targetCell, no fire arrow");
    assert!(plain.sim.cues.loosed(WeaponClass::Bow) > 0, "though the archer shot");
}

