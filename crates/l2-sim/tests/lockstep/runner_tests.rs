#![allow(unused_imports)]
use super::*;
use super::basic::*;
use super::ai_battle::*;
use super::runner_battle::*;
use super::hashing::*;
use l2_net::{
    decode_all, frame, Advance, Canonical, Command, Config, FrameReader, Message, PeerId,
    PlayerSlot, Session, Simulation, TcpTransport, Tick, Transport,
};
use l2_sim::{Battle, Troop, SIDE_A, SIDE_B};
use l2_sim::ai::{self, Ai, AiField};
use l2_sim::unit::{Units, CATEGORY_OF_TROOP};
use l2_sim::{Figure, State};
use l2_sim::runner::{Army, BattleRunner};

const RUNNER_TICKS: Tick = Tick(700);

fn player_order(slot: u8, at: Tick) -> Option<Vec<u8>> {
    match (slot, at.0) {
        (1, 5) => Some(vec![OP_ORDER, 2, 40, 38]),
        _ => None,
    }
}

#[test]
fn two_peers_running_a_whole_battle_stay_bit_identical() {
    let mut peers = connected_pair(0x0B47_0001, &RunnerNetBattle::new);
    run_to(&mut peers, RUNNER_TICKS, player_order);

    let common = peers[0].hashes.len().min(peers[1].hashes.len());
    assert!(common >= RUNNER_TICKS.0 as usize, "only {common} ticks executed on both");
    assert_eq!(peers[0].hashes[..common], peers[1].hashes[..common], "the peers diverged");
    for peer in &peers {
        assert!(!peer.session.is_halted(), "{:?}", peer.session.halt_reason());
        assert!(peer.session.divergence().is_none());
    }
}

#[test]
fn both_the_players_order_and_the_ais_decision_reached_the_men() {
    let mut peers = connected_pair(0x0B47_0002, &RunnerNetBattle::new);
    run_to(&mut peers, RUNNER_TICKS, player_order);
    let sim = &peers[0].sim;
    let fresh = RunnerNetBattle::new();

    let ai_unit = sim.runner.units.live().next().unwrap();
    let player_unit = sim.runner.units.live().nth(1).unwrap();
    assert!(!sim.runner.units.get(ai_unit).human);
    assert!(sim.runner.units.get(player_unit).human);

    assert_eq!(sim.runner.units.get(ai_unit).orders, 3, "three thinks in 700 frames");
    assert_eq!(sim.runner.units.get(player_unit).orders, 0, "a player's unit never thinks");
    assert_eq!(fresh.runner.units.get(ai_unit).target_y, 50, "seeded on its own end");
    assert_eq!(
        sim.runner.units.get(ai_unit).target_y, 30,
        "the AI never marched on the enemy's deployment marker"
    );

    let moved = |r: &BattleRunner, side: l2_sim::Side| -> bool {
        r.fighters
            .iter()
            .zip(fresh.runner.fighters.iter())
            .any(|(a, b)| a.side == side && (a.x, a.y) != (b.x, b.y))
    };
    assert!(moved(&sim.runner, SIDE_B), "the AI's men never advanced");
    assert!(moved(&sim.runner, SIDE_A), "the ordered pikemen never advanced");
    assert_ne!(sim.runner.ai.rng, fresh.runner.ai.rng, "the jitter was never drawn");
}

#[test]
fn a_peer_whose_figure_stepped_wrong_is_caught() {
    let mut peers = connected_pair(0x0B47_0003, &RunnerNetBattle::new);
    peers[1].sim.nudge_at = Some(Tick(300));
    run_to(&mut peers, RUNNER_TICKS, player_order);

    assert!(
        peers.iter().any(|p| p.session.is_halted()),
        "a figure standing somewhere else ran to completion undetected"
    );
    let split = peers[0]
        .hashes
        .iter()
        .zip(&peers[1].hashes)
        .position(|(a, b)| a != b)
        .expect("no differing tick in the recorded history");
    assert_eq!(peers[0].hashes[split].0, Tick(300));
}

