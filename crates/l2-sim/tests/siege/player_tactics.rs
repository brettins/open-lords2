#![allow(unused_imports)]
use super::*;
use super::handlers::*;
use super::breaches_and_assaults::*;
use super::wall_damage::*;
use super::outcomes::*;
use l2_sim::ai::{handler_for, TABLE_FIELD, TABLE_SIEGE_ATT, TABLE_SIEGE_DEF};
use l2_sim::runner::{ASSAULT_REPEATS_BELOW_LEVEL, ASSAULT_REPEAT_SCORE};
use l2_sim::siege::{
    self, SiegeState, FLAG_KEEP, FLAG_WALL, SURFACE_BAILEY, SURFACE_RAMPART_WALK, SURFACE_WALL,
};
use l2_sim::{BattleRunner, End, Muster, Troop, SIDE_A, SIDE_B};

/// The besieger of the tests above: 848 men and four engines, and **not one
/// bowman**.
///
/// That is deliberate and it is the second thing this file learned tonight.
/// The first draft gave the besieger 148 archers, and at three of the five
/// castle levels it won with `breach_score` 0, `wall_damage` 0 and the wall
/// untouched — 148 archers simply shot two garrison figures off the rampart
/// from the open field.
/// proved a shooting match. `docs/agents.md`'s *"a check that passes for an
/// accidental reason looks exactly like one that passes"*: it went green for a
/// reason unrelated to anything the branch changed, and would have gone on
/// doing so with the whole assault path deleted.
///
/// With no missile troop at all the only ways to win are the three
/// `Battle_CheckOutcome`
/// requires getting through the wall.
pub(super) const STORMING_PARTY: &[(Troop, u32)] = &[
    (Troop::Peasants, 448),
    (Troop::Swordsmen, 300),
    (Troop::Knights, 100),
    (Troop::Catapults, 2),
    (Troop::BatteringRams, 2),
];

pub(super) fn storming_party(level: u8) -> BattleRunner {
    BattleRunner::deploy_siege(
        siege::our_castle(level),
        99,
        Muster { troops: STORMING_PARTY, owner: 1, human: false },
        Muster { troops: &[(Troop::Archers, 8u32)], owner: 2, human: false },
        level,
    )
}

/// **The approach score's two starting values, and the moat is what chooses.**
///
/// `Battlefield_BuildCastle` writes 500; the raster's moat byte `0xEE` hands
/// each ditch cell to `FUN_0047DCCE`, whose first statement puts it back to 0.
/// So a dry castle opens with the approach already *done* and a moated one
/// opens with everything to do — which is exactly the shape of
/// `Order_ToBreachOrStaging`'s three arms.
#[test]
fn a_dry_castle_opens_at_five_hundred_and_a_moated_one_at_zero() {
    for level in 0..=4u8 {
        let r = storming_party(level);
        let moated = r.field.cells.iter().any(|c| c.surface == siege::SURFACE_WATER);
        assert_eq!(moated, level >= 2, "level {level}");
        assert_eq!(
            r.ai.approach_score,
            if moated { 0 } else { siege::APPROACH_SCORE_START },
            "level {level}"
        );
    }
}

/// **A player's assault: a ram opens the gate, and the Charge button carries
/// it.**
///
/// The other half of the headline, and the one that
/// `Wall_Smash`. The AI besieger above wins through cells a *catapult*
/// collapsed one at a time; this one drives the 20,000-hit gate accumulator
/// with rams, which is the path that opens a nine-cell hole and leaves it at
/// the wall's own height.
///
/// Everything the player does here is a value: [`BattleRunner::order_side`] and
/// [`BattleRunner::charge_all`], the same two calls the battlefield screen
/// makes. Nothing sets a flag by hand.
#[test]
fn a_player_rams_the_gate_open_and_charges_through_the_breach() {
    let level = 1; // dry, so the men reach the wall without shovelling
    let field = siege::our_castle(level);
    let keep = field.cells.iter().position(|c| c.flags & FLAG_KEEP != 0).expect("a way in");
    let (kx, ky) = ((keep % 80) as u8, (keep / 80) as u8);
    let mut r = BattleRunner::deploy_siege(
        field,
        21,
        Muster {
            troops: &[(Troop::BatteringRams, 2u32), (Troop::Swordsmen, 600)],
            owner: 1,
            human: true,
        },
        Muster { troops: &[(Troop::Archers, 40u32)], owner: 2, human: false },
        level,
    );

    let wall_before = r.field.cells.iter().filter(|c| c.flags & FLAG_WALL != 0).count();
    let mut ticks = 0;
    let mut inside = 0;
    let end = loop {
        r.order_side(SIDE_B, kx, ky);
        r.charge_all(1);
        r.run(500);
        ticks += 500;
        inside = inside.max(r.ai.attackers_on_wall);
        if let Some(c) = r.conclusion() {
            break c;
        }
        assert!(
            ticks < 60_000,
            "the gate is {} after {ticks} frames, {} men inside, {} wall cells left",
            if r.siege.gate_breached { "open" } else { "shut" },
            inside,
            r.field.cells.iter().filter(|c| c.flags & FLAG_WALL != 0).count(),
        );
    };

    assert!(r.siege.gate_breached, "two rams open a gate");
    // **`Wall_Smash` is heard.** `FUN_0049694F` opens with
    // `Sound_PlayFile("bathit2.wav", 0, 0)`, so the record a listener reads has
    // to say a wall came down. Ablation: delete `self.sim.cues.wall_smashed()`
    // in `BattleRunner::strike_castle`.
    assert!(r.sim.cues.walls_smashed() > 0, "the breach went uncued: {:?}", r.sim.cues);
    let opened = wall_before - r.field.cells.iter().filter(|c| c.flags & FLAG_WALL != 0).count();
    assert!(
        opened >= 5,
        "the gate breach opens a 9 x 9 of wall, not one cell — {opened} came down"
    );
    assert!(inside > 0, "and the charge carried men through it");
    assert_eq!(end.winner, SIDE_B);
}

