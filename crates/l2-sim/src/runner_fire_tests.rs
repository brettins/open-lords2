//! **Fire, oil and the siege tower, each against a battle built so it must
//! happen** — and the determinism proof extended to a siege that uses all of
//! them.
//!
//! A child of [`crate::runner`], so a test can stand a man on a chosen cell
//! the way `firing_line` does. The siege is [`crate::proving`]'s, and every
//! literal asserted below is the original's: a frame count, a fire's life, a
//! cell's height, read out of the routine the test names — never computed from
//! the constant under test.

use super::*;
use crate::cue::Cues;
use crate::fire::{SURFACE_BRIDGE, SURFACE_BURNING, SURFACE_WOODLAND, SURFACE_WOOD_BURNING, SURFACE_WOOD_CATCHING};
use crate::proving;
use crate::terrain::flag;

fn cell(x: u8, y: u8) -> usize {
    y as usize * DIM + x as usize
}

/// The fire record burning over `(x, y)`, if there is one.
fn fire_at(r: &BattleRunner, x: u8, y: u8) -> Option<crate::missile::Missile> {
    r.missiles
        .iter()
        .map(|(_, m)| *m)
        .find(|m| m.class == crate::missile::CLASS_FIRE && (m.cell_x, m.cell_y) == (x as i16, y as i16))
}

/// Stand one figure on a cell by hand, as `firing_line` does.
fn stand(r: &mut BattleRunner, troop: Troop, side: Side, owner: u8, human: bool, at: (u8, u8)) -> usize {
    let sim = r.sim.add(troop, side, 4).unwrap();
    r.sim.figures[sim].owner = owner;
    r.sim.figures[sim].owner_is_human = human;
    r.fighters.push(Fighter {
        sim,
        troop,
        side,
        x: at.0,
        y: at.1,
        target: at,
        facing: 0,
        progress: Progress::default(),
        anim: Motion::Idle,
        phase: 0,
        path: Vec::new(),
        barred: 0,
        hold: 0,
        reroutes: 0,
        moat_cell: None,
        moat_load: 0,
        polar: 0,
        corpse: 0,
    });
    let i = r.fighters.len() - 1;
    r.occupant[cell(at.0, at.1)] = Some(i as u16);
    i
}

// ------------------------------------------------------------------ boiling oil

/// **A pot ordered down off the rampart pours, dies, and sets a cross of
/// ground burning under the stream every frame it flies.**
///
/// `BattleUnit_Order`'s oil loop, `FUN_0047A814` and `Missile_UpdateAll`'s
/// class-7 arm. The five lives are pinned as the literals the original
/// produces on the stream's first frame in flight — `ticksFlown` 5, so an
/// argument of `4 × 32 + bias` against `0x280`, and one frame already counted
/// off because every fire is in a higher slot than the stream: **471** for the
/// centre and the north, **511** south, **486** east, **501** west.
///
/// Ablation: zero the `bias` column of `OIL_CROSS` and the centre reads 511.
#[test]
fn a_pot_ordered_off_the_rampart_pours_and_the_ground_under_the_stream_burns() {
    let mut r = proving::deploy();
    while r.tick < proving::POUR_TICK {
        proving::orders(&mut r);
        r.step();
    }
    let pot = proving::fighter_of(&r, SIDE_A, Troop::Oil).unwrap();
    assert!(r.is_alive(pot), "the pot is standing before the order");
    assert_eq!((r.fighters[pot].x, r.fighters[pot].y), proving::POT_AT);
    assert_eq!(r.sim.cues.oil_poured(), 0);

    proving::orders(&mut r);
    assert!(!r.is_alive(pot), "FUN_0047A814 writes state 2: the pot is spent by its own pour");
    assert_eq!(r.sim.cues.oil_poured(), 1);
    assert_eq!(r.fighters[pot].facing, 4, "turned along the longer axis, south");
    let stream = r.missiles.iter().map(|(_, m)| *m).find(|m| m.class == crate::missile::CLASS_OIL);
    let stream = stream.expect("a stream in the air");
    assert_eq!((stream.ticks_flown, stream.cell_y), (4, 31), "four launch steps, two cells out");

    r.step();
    let expect = [((35, 31), 471), ((35, 30), 471), ((35, 32), 511), ((36, 31), 486), ((34, 31), 501)];
    for ((x, y), life) in expect {
        assert_eq!(r.field.cells[cell(x, y)].surface, SURFACE_BURNING, "({x}, {y}) is alight");
        assert_eq!(fire_at(&r, x, y).map(|m| m.ttl), Some(life), "({x}, {y})'s life");
    }
    assert_eq!(fire_at(&r, 35, 30).unwrap().saved_surface, crate::siege::SURFACE_WALL);

    // Sixteen frames of range, half a cell a frame: the stream's last cross is
    // centred on row 37, whose southern arm is row 38, and it is gone on the
    // seventeenth frame.
    for _ in 0..20 {
        proving::orders(&mut r);
        r.step();
    }
    assert_eq!(r.missiles.iter().filter(|(_, m)| m.class == crate::missile::CLASS_OIL).count(), 0);
    for y in 30..=38 {
        assert_eq!(r.field.cells[cell(35, y)].surface, SURFACE_BURNING, "row {y}");
    }
    assert_ne!(r.field.cells[cell(35, 39)].surface, SURFACE_BURNING, "row 39 is past the range");
    assert_eq!(fire_at(&r, 35, 37).unwrap().saved_surface, 0, "open ground");
}

/// **Men in the fire burn, and a figure that loses its last man to it is
/// cued by its own side** — `BattleMan_BurnTick`, from the pour above.
///
/// Every peasant standing on a burning cell takes **four** hits a frame —
/// three at size class 0 and one for a human's — and nothing else can have
/// hurt them: the garrison's archer is out of range of everybody.
#[test]
fn the_peasants_under_the_oil_burn_four_hits_a_frame_and_die_of_it() {
    let mut r = proving::deploy();
    assert_eq!(r.battle_size_class(), 0);
    let in_fire = |r: &BattleRunner| -> Vec<usize> {
        (0..r.fighters.len())
            .filter(|&i| {
                r.fighters[i].troop == Troop::Peasants
                    && r.is_alive(i)
                    && r.field.cells[cell(r.fighters[i].x, r.fighters[i].y)].surface == SURFACE_BURNING
            })
            .collect()
    };
    while in_fire(&r).is_empty() && r.tick < proving::POUR_TICK + 40 {
        proving::orders(&mut r);
        r.step();
    }
    let burning = in_fire(&r);
    assert!(!burning.is_empty(), "the pour landed on somebody by frame {}", r.tick);
    let i = burning[0];
    let sim = r.fighters[i].sim;
    let (men, hits) = (r.sim.figures[sim].men, r.sim.figures[sim].hits);
    r.step();
    let after = (r.sim.figures[sim].men, r.sim.figures[sim].hits);
    assert!(
        after == (men, hits + 4) || after == (men - 1, hits + 4 - 100),
        "four hits a frame: {men}/{hits} -> {after:?}"
    );

    for _ in 0..200 {
        proving::orders(&mut r);
        r.step();
    }
    assert!(!r.is_alive(i), "a hundred hits a man, four men, a hundred frames");
    assert!(r.sim.cues.burn_deaths(SIDE_B) >= 1);
    assert_eq!(r.sim.cues.burn_deaths(SIDE_A), 0);
    assert_eq!(r.sim.cues.melee_deaths(SIDE_B) + r.sim.cues.missile_deaths(), 0, "fire, and only fire");
}

/// **A fire goes out by itself and gives the ground back.** The first cross's
/// northern arm is on the curtain, surface 8, and lives 472 frames; on the
/// frame its count reads 2 the curtain is a curtain again — its flags, which
/// the fire never touched, included.
#[test]
fn an_oil_fire_goes_out_and_the_wall_comes_back() {
    let fresh = proving::field();
    let mut r = proving::run(proving::POUR_TICK + 1);
    let life = fire_at(&r, 35, 30).unwrap().ttl;
    assert_eq!(life, 471);
    for _ in 0..(life - 2) {
        proving::orders(&mut r);
        r.step();
    }
    assert_eq!(r.field.cells[cell(35, 30)].surface, SURFACE_BURNING, "one frame before");
    r.step();
    assert_eq!(r.field.cells[cell(35, 30)], fresh.cells[cell(35, 30)], "the frame the count reads 2");
    assert_eq!(r.field.cells[cell(35, 30)].surface, crate::siege::SURFACE_WALL);
    assert!(r.blocked[cell(35, 30)], "still a curtain nobody walks through");
    r.step();
    assert!(fire_at(&r, 35, 30).is_none(), "and the record is gone a frame later");
}

/// **A man who steps at a pot of oil is poured on.** `Melee_AdjacentEnemyDir`
/// does not skip siege engines, so a swordsman walking past a pot at his own
/// height steps at it, and the 999 arm hands the pot a `Melee_Tick`.
///
/// **A green ablation, and it is the finding.** Returning `None` from
/// `adjacent_oil` alone leaves this green: the swordsman then commits the step
/// into the pot's cell, and `enter_cell`'s contact arm pours on the same frame
/// at the same cell. The two paths are the original's one road seen from both
/// ends — the direction chosen, and the step refused with 999 — so only
/// ablating **both** turns it red, which it does.
#[test]
fn a_man_who_steps_at_a_pot_of_oil_is_poured_on() {
    let mut r = BattleRunner::empty(blank_field(), DEFAULT_SEED);
    let pot = stand(&mut r, Troop::Oil, SIDE_A, 1, false, (40, 40));
    let man = stand(&mut r, Troop::Swordsmen, SIDE_B, 2, false, (40, 44));
    r.fighters[man].target = (40, 36);
    r.settle();
    let mut poured_at = None;
    for _ in 0..400 {
        r.step();
        if r.sim.cues.oil_poured() > 0 {
            poured_at = Some((r.fighters[man].x, r.fighters[man].y));
            break;
        }
    }
    assert_eq!(poured_at, Some((40, 41)), "poured on the frame he stepped at it, one cell away");
    assert!(!r.is_alive(pot));
    // The stream runs from the pot through him, so his cell is in a cross.
    for _ in 0..3 {
        r.step();
    }
    let sim = r.fighters[man].sim;
    assert!(
        r.sim.figures[sim].hits > 0 || r.sim.figures[sim].men < 4,
        "he is standing in his own pour"
    );
}

// ------------------------------------------------------------- the bridge fire

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
/// Ablation, by insertion because there is no reader to delete: make
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
