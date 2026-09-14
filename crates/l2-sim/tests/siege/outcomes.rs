#![allow(unused_imports)]
use super::*;
use super::handlers::*;
use super::breaches_and_assaults::*;
use super::wall_damage::*;
use super::player_tactics::*;
use l2_sim::ai::{handler_for, TABLE_FIELD, TABLE_SIEGE_ATT, TABLE_SIEGE_DEF};
use l2_sim::runner::{ASSAULT_REPEATS_BELOW_LEVEL, ASSAULT_REPEAT_SCORE};
use l2_sim::siege::{
    self, SiegeState, FLAG_KEEP, FLAG_WALL, SURFACE_BAILEY, SURFACE_RAMPART_WALK, SURFACE_WALL,
};
use l2_sim::{BattleRunner, End, Muster, Troop, SIDE_A, SIDE_B};

/// **848 men against a garrison of two figures, at every castle level.**
///
/// This is the exact position the branch started from, and it is written as a
/// *long* battle on purpose. The four defects it found were all invisible to a
/// 600-frame test — the whole suite's previous longest siege — because every
/// one of them is an order
/// wrong:
///
/// | | |
/// |---|---|
/// | a breach was **one cell wide** | `Wall_Smash` (`FUN_0049694F`) opens a 9 × 9 |
/// | the approach score started at **0** | `Battlefield_BuildCastle` writes 500, and 0 only when there is a ditch |
/// | the wall stood at **elevation 2** | the builder's structure code 8 writes 1, and a step of 2 cannot be climbed |
/// | the rampart walk sat **against the wall** | `Wall_Collapse` scores only bailey neighbours, so a catapult earned nothing |
///
/// Before them the besieger stood in the field for 400,000 frames. After them
/// every level ends, and the *cause* differs by level.
/// the three different ways in are all live.
#[test]
fn a_besieger_with_eight_hundred_men_takes_a_castle_held_by_two() {
    // `Missile_Step`'s counting arm against a wall plays `FUN_004262CF(0xF)`,
    // `cathit.wav`, so a listener must be told each time a catapult's shot is
    // counted. Summed over the five levels because which way in a level takes
    // differs. Ablation: delete `self.sim.cues.wall_struck()` in
    // `BattleRunner::strike_wall_with_shot`.
    let mut walls_struck = 0;
    for level in 0..=4u8 {
        let mut r = storming_party(level);
        assert_eq!(r.men_of_side(SIDE_B), 848, "the besieger, level {level}");
        assert!(r.living(SIDE_A) <= 2, "a garrison of at most two figures");

        let mut ticks = 0;
        // `g_attackersOnWall` — side-4 figures standing on surface 5, which
        // after `Wall_Smash` is the breach and the bailey behind it. It going
        // positive *is* "the assault was pressed home", and it is recounted
        // from the figures' own cells every frame
        // this test can reach.
        let mut got_inside = 0;
        let end = loop {
            r.run(500);
            ticks += 500;
            got_inside = got_inside.max(r.ai.attackers_on_wall);
            if let Some(c) = r.conclusion() {
                break c;
            }
            assert!(
                ticks < 200_000,
                "level {level}: 848 men still outside after {ticks} frames — \
                 approach {} breach {} gate {} ramparts {} engines {} inside {got_inside}",
                r.ai.approach_score,
                r.ai.breach_score,
                r.siege.gate_breached,
                r.siege.ramparts_breached,
                r.ai.siege_engine_count,
            );
        };
        assert_eq!(end.winner, SIDE_B, "level {level}: the besieger takes it");
        assert!(
            got_inside > 0 || end.cause == End::BrokeIn,
            "level {level}: the besieger won without a man ever getting in — \
             {:?} after {ticks} frames",
            end.cause
        );
        println!(
            "level {level}: {:?} at {ticks} frames, inside {got_inside}, \
             gate {} ramparts {} breach {} wall damage {} moat {}",
            end.cause,
            r.siege.gate_breached,
            r.siege.ramparts_breached,
            r.ai.breach_score,
            r.siege.wall_damage,
            r.siege.moat_filled,
        );
        assert!(
            r.sim.cues.loosed(l2_sim::WeaponClass::Catapult) > 0,
            "level {level}: the storming party's catapults never fired"
        );
        walls_struck += r.sim.cues.walls_struck();
    }
    assert!(walls_struck > 0, "no catapult shot was ever counted against a wall");
}

/// And the other direction.
/// either: **the same 848 men lose to a garrison that outnumbers them.**
///
/// The pair is the real assertion. One of them alone passes for a model that
/// has stopped simulating.
#[test]
fn the_same_besieger_is_thrown_off_a_castle_held_in_strength() {
    let mut r = BattleRunner::deploy_siege(
        siege::our_castle(4),
        99,
        Muster { troops: STORMING_PARTY, owner: 1, human: false },
        Muster { troops: &[(Troop::Archers, 2_000u32)], owner: 2, human: false },
        4,
    );
    let mut ticks = 0;
    let end = loop {
        r.run(500);
        ticks += 500;
        if let Some(c) = r.conclusion() {
            break c;
        }
        assert!(ticks < 200_000, "neither side could finish it");
    };
    assert_eq!(end.winner, SIDE_A, "the garrison holds");
}

