#![allow(unused_imports)]
use super::*;
use super::ai_battle::*;
use super::runner_battle::*;
use super::runner_tests::*;
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

#[test]
fn two_peers_simulate_the_same_battle_over_a_real_socket() {
    let mut peers = connected_pair(0x1025_1796, &NetBattle::new);
    run_to(&mut peers, TICKS, schedule);

    let common = peers[0].hashes.len().min(peers[1].hashes.len());
    assert!(
        common >= TICKS.0 as usize,
        "only {common} ticks were executed on both peers"
    );
    assert_eq!(
        peers[0].hashes[..common],
        peers[1].hashes[..common],
        "the two peers diverged"
    );

    assert_eq!(
        peers[0].sim.battle, peers[1].sim.battle,
        "state differs despite matching checksums"
    );
    for peer in &peers {
        assert!(!peer.session.is_halted(), "{:?}", peer.session.halt_reason());
        assert!(peer.session.divergence().is_none());
    }
}

#[test]
fn the_commands_actually_reached_the_simulation() {
    let mut peers = connected_pair(0x00ab_cdef, &NetBattle::new);
    run_to(&mut peers, TICKS, schedule);

    let fresh = lineup();
    assert_ne!(
        peers[0].sim.battle, fresh,
        "no order took effect - this test would pass on two idle sims"
    );
    let hurt = peers[0]
        .sim
        .battle
        .figures
        .iter()
        .zip(&fresh.figures)
        .any(|(now, before)| now.hits > 0 || now.men < before.men);
    assert!(hurt, "orders crossed the wire but nobody fought");
}

#[test]
fn a_rule_refusal_is_identical_on_both_peers() {
    let mut peers = connected_pair(0x0000_5ca7, &NetBattle::new);
    run_to(&mut peers, TICKS, schedule);

    for peer in &peers {
        let catapult = peer.sim.battle.figures[6];
        assert_eq!(catapult.troop, Troop::Catapults);
        assert_eq!(catapult.opponent, None, "the catapult was drawn into melee");
        assert_eq!(catapult.hits, 0, "the catapult was fought");
    }
    assert_eq!(peers[0].sim.battle, peers[1].sim.battle);
}

#[test]
fn a_peer_running_different_rules_is_caught() {
    let mut peers = connected_pair(0x0000_dead, &NetBattle::new);
    peers[1].sim.perturb_at = Some(Tick(30));
    run_to(&mut peers, TICKS, schedule);

    let caught = peers
        .iter()
        .filter_map(|p| p.session.divergence())
        .next()
        .expect("a divergence ran to completion undetected");
    assert!(
        caught.tick >= Tick(30),
        "reported {:?}, before the perturbation could have had an effect",
        caught.tick
    );

    let split = peers[0]
        .hashes
        .iter()
        .zip(&peers[1].hashes)
        .position(|(a, b)| a != b)
        .expect("no differing tick in the recorded history");
    assert_eq!(
        peers[0].hashes[split].0,
        Tick(30),
        "the first differing tick should be the perturbed one"
    );
}


