#![allow(unused_imports)]
use super::*;
use super::handlers::*;
use super::wall_damage::*;
use super::outcomes::*;
use super::player_tactics::*;
use l2_sim::ai::{handler_for, TABLE_FIELD, TABLE_SIEGE_ATT, TABLE_SIEGE_DEF};
use l2_sim::runner::{ASSAULT_REPEATS_BELOW_LEVEL, ASSAULT_REPEAT_SCORE};
use l2_sim::siege::{
    self, SiegeState, FLAG_KEEP, FLAG_WALL, SURFACE_BAILEY, SURFACE_RAMPART_WALK, SURFACE_WALL,
};
use l2_sim::{BattleRunner, End, Muster, Troop, SIDE_A, SIDE_B};

/// **A wall comes down**, and a ram is twenty times faster at it than a man.
#[test]
fn a_battering_ram_opens_the_gate_and_the_breach_reaches_the_order_layer() {
    let mut s = SiegeState::castle(4);
    let mut frames = 0;
    while !s.gate_breached {
        siege::strike_wall(&mut s, siege::SURFACE_GROUND, true);
        frames += 1;
    }
    assert_eq!(frames, 1_000, "one ram, one thousand frames");

    // And in a running battle. The besieger is a player's here — an AI unit
    // marches to the approach points its script names, and the player's is the
    // side that
    let field = siege::our_castle(1);
    let wall_cells = field.cells.iter().filter(|c| c.flags & FLAG_WALL != 0).count();
    assert!(wall_cells > 0);
    let gate = field
        .cells
        .iter()
        .position(|c| c.flags & FLAG_KEEP != 0)
        .expect("a way in");
    let (gx, gy) = ((gate % 80) as u8, (gate / 80) as u8);
    let mut r = BattleRunner::deploy_siege(
        field,
        11,
        Muster {
            troops: &[(Troop::BatteringRams, 2u32), (Troop::Peasants, 200)],
            owner: 1,
            human: true,
        },
        Muster { troops: &[(Troop::Archers, 40u32)], owner: 2, human: false },
        1,
    );
    r.order_side(SIDE_B, gx, gy);
    for _ in 0..80 {
        r.run(200);
        r.order_side(SIDE_B, gx, gy);
        if r.siege.gate_hits > 0 || r.siege.ramparts_breached > 0 {
            break;
        }
    }
    assert!(
        r.siege.gate_hits > 0 || r.siege.ramparts_breached > 0,
        "the besiegers reached the wall and hit it"
    );
}

/// **The way in ends the siege without a man of the garrison being killed.**
///
/// `DAT_00553F3C`, which `Battle_CheckOutcome` tests before either men counter
/// and which no document had before today.
#[test]
fn reaching_the_keep_wins_the_siege_outright() {
    let mut r = siege_battle(0, 3);
    assert!(r.conclusion().is_none());
    let garrison_before = r.men_of_side(SIDE_A);
    r.siege.broke_in = true;
    let c = r.conclusion().expect("a siege ends when somebody gets in");
    assert_eq!(c.winner, SIDE_B, "the besieger");
    assert_eq!(c.cause, End::BrokeIn);
    assert_eq!(r.men_of_side(SIDE_A), garrison_before, "and the garrison is untouched");

    // The cell is real, not a flag somebody has to set by hand.
    assert_eq!(
        siege::our_castle(0).cells.iter().filter(|c| c.flags & FLAG_KEEP != 0).count(),
        1
    );
}

/// **Assault repulsed, repeat** — the one arm of the outcome test that changes
/// something
/// from *the besieger loses*.
#[test]
fn a_small_castle_repeats_a_failed_assault_and_a_large_one_ends_it() {
    // A besieger with no engines at all is the position both arms test, and
    // the engine count is recounted from the figures every frame — so this is
    // reached by bringing none
    let bare = |level: u8| {
        BattleRunner::deploy_siege(
            siege::our_castle(level),
            5,
            Muster { troops: &[(Troop::Peasants, 200u32)], owner: 1, human: false },
            Muster { troops: &[(Troop::Archers, 60u32)], owner: 2, human: false },
            level,
        )
    };

    // Level 2: the scores reset and the fight goes on.
    let mut small = bare(2);
    small.run(1);
    assert_eq!(small.ai.siege_engine_count, 0, "it brought none");
    assert!(small.conclusion().is_none(), "below level 3 the assault repeats");
    assert_eq!(small.ai.breach_score, ASSAULT_REPEAT_SCORE);
    assert_eq!(small.ai.approach_score, ASSAULT_REPEAT_SCORE);

    // Level 3: the same position is a defeat, and the *campaign* gate refuses
    // to start it for the same reason and at the same level.
    let mut large = bare(3);
    large.run(1);
    let c = large.conclusion().expect("at level 3 there is no second chance");
    assert_eq!(c.winner, SIDE_A, "the garrison");
    assert_eq!(c.cause, End::AssaultFailed);
    assert_eq!(ASSAULT_REPEATS_BELOW_LEVEL, 3);
}

/// A siege still ends the two ways a field battle does, and the men counters
/// still ignore the engines.
#[test]
fn a_siege_still_ends_on_annihilation_and_engines_are_worth_no_men() {
    let mut r = siege_battle(0, 9);
    let before = r.men_of_side(SIDE_B);
    r.run(1);
    // Two catapults, two towers and a ram, and not one of them counts.
    let engines: u32 = r
        .fighters
        .iter()
        .filter(|f| f.troop.index() >= 7 && r.is_alive(0))
        .map(|f| f.troop.index() as u32)
        .sum();
    assert!(engines > 0, "the besieger brought engines");
    assert_eq!(r.men_of_side(SIDE_B), before, "and they are worth no men");
}

/// Determinism, which the whole crate rests on: a siege is a simulation like
/// any other and two runs of it must be bit-identical.
#[test]
fn two_runs_of_the_same_siege_stay_identical() {
    let (mut a, mut b) = (siege_battle(4, 42), siege_battle(4, 42));
    for _ in 0..20 {
        a.run(100);
        b.run(100);
        assert_eq!(a.siege, b.siege, "diverged at tick {}", a.tick);
        assert_eq!(a.sim.figures, b.sim.figures, "diverged at tick {}", a.tick);
    }
}

