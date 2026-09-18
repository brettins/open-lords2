#![allow(unused_imports)]
use super::*;
use super::validation_and_config::*;
use super::*;
use super::flow_tests::*;
use super::divergence_tests::*;
use common::{Order, ToySim};
use l2_net::{
    decode_all, frame, Advance, Canonical, Config, Endpoint, FrameReader, HaltReason, Loopback,
    Message, PeerId, PlayerSlot, Fixed, Session, SessionError, Tick, Transport,
};

#[test]
fn two_sessions_fed_identical_commands_agree_on_every_tick() {
    let mut table = Table::new(Config::battle(), 2, 0xabc_def);

    for round in 0..60 {
        if round % 7 == 0 {
            table.peers[0].session.issue(Order::Attack { unit: 1 }.payload());
        }
        if round % 5 == 0 {
            let order =
                Order::MoveTo { unit: 2, x: Fixed::from_int(40), y: Fixed::from_int(12) };
            table.peers[1].session.issue(order.payload());
        }
        table.pump();
    }

    assert!(!table.peers[0].session.is_halted(), "{:?}", table.peers[0].session.halt_reason());
    assert!(table.peers[0].hashes.len() > 50, "the session barely advanced");
    assert_eq!(
        table.peers[0].hashes, table.peers[1].hashes,
        "two peers disagreed about some tick"
    );
    assert!(table.all_agree());
    assert_eq!(table.peers[0].errors, vec![]);
    assert_eq!(table.peers[0].sim.bad_commands, 0);
}

#[test]
fn a_perturbed_peer_is_caught_and_both_sides_halt() {
    let mut table = Table::new(Config::battle(), 2, 7);
    table.peers[1].sim.bias = 1;

    table.run(4);
    assert!(!table.peers[0].session.is_halted(), "diverged before anything could differ");

    table.peers[0].session.issue(Order::Attack { unit: 0 }.payload());
    table.run(12);

    let divergence = table.peers[0].session.divergence().expect("peer 0 must notice");
    assert_eq!(
        table.peers[0].session.halt_reason(),
        Some(&HaltReason::Desync { tick: divergence.tick })
    );
    assert!(table.peers[1].session.is_halted(), "the other peer must notice too");
    assert_eq!(divergence.peer, PlayerSlot::new(1));
    assert_ne!(divergence.ours, divergence.theirs);
    assert_eq!(
        divergence.last_agreed,
        Some(divergence.tick.minus(1)),
        "with a checksum every tick the divergence is localised to one tick"
    );
}

#[test]
fn a_halted_session_never_steps_again() {
    let mut table = Table::new(Config::battle(), 2, 7);
    table.peers[1].sim.bias = 1;
    table.peers[0].session.issue(Order::Attack { unit: 0 }.payload());
    table.run(12);
    assert!(table.peers[0].session.is_halted());

    let frozen = table.peers[0].session.tick();
    let state = l2_net::state_hash(&table.peers[0].sim);
    table.run(20);
    assert_eq!(table.peers[0].session.tick(), frozen);
    assert_eq!(l2_net::state_hash(&table.peers[0].sim), state);
}

#[test]
fn the_first_halt_reason_wins() {
    let mut table = Table::new(Config::battle(), 2, 7);
    table.peers[1].sim.bias = 1;
    table.peers[0].session.issue(Order::Attack { unit: 0 }.payload());
    table.run(12);
    let reason = table.peers[0].session.halt_reason().cloned().unwrap();
    table.peers[0].session.halt(HaltReason::Left);
    assert_eq!(table.peers[0].session.halt_reason(), Some(&reason));
}


#[test]
fn latency_and_reordering_change_nothing_but_the_pace() {
    fn play(latency: u64, reorder: bool) -> Vec<(Tick, u64)> {
        let mut table = Table::new(Config::battle(), 2, 0x5eed);
        table.net.set_latency(PeerId(0), latency);
        table.net.set_reorder(PeerId(0), reorder);
        table.net.set_reorder(PeerId(1), reorder);
        table.run_scheduled(200, |slot, tick| match (slot, tick.0 % 12) {
            (0, 0) => Some(Order::Attack { unit: 1 }),
            (1, 5) => Some(Order::Attack { unit: 2 }),
            (1, 9) => {
                Some(Order::MoveTo { unit: 0, x: Fixed::from_int(8), y: Fixed::from_int(8) })
            }
            _ => None,
        });
        assert!(!table.peers[0].session.is_halted());
        assert_eq!(table.peers[0].hashes, table.peers[1].hashes);
        table.peers[0].hashes.clone()
    }

    let clean = play(0, false);
    assert!(clean.len() > 100);
    let delayed = play(3, false);
    let jumbled = play(0, true);
    let both = play(3, true);

    for (name, run) in [("delayed", &delayed), ("jumbled", &jumbled), ("both", &both)] {
        assert!(!run.is_empty(), "{name} produced nothing");
        assert_eq!(
            run[..],
            clean[..run.len()],
            "{name} produced different checksums from the clean run"
        );
    }
}


#[test]
fn a_stalled_link_pauses_the_session_and_it_resumes_intact() {
    let mut table = Table::new(Config::battle(), 2, 3);
    table.run(10);
    let reached = table.peers[0].session.tick();
    assert!(reached > Tick(4));

    table.net.hold(PeerId(0), true);
    table.peers[0].waited_for.clear();
    table.run(20);

    assert_eq!(table.peers[0].session.tick(), reached, "the session must not advance on a guess");
    assert!(!table.peers[0].session.is_halted(), "a stall is not a halt: this crate has no clock");
    assert!(!table.peers[0].waited_for.is_empty(), "the stall must name who it is waiting for");
    assert!(
        table.peers[0].waited_for.iter().all(|s| *s == PlayerSlot::new(1)),
        "waiting for {:?}",
        table.peers[0].waited_for
    );

    table.net.hold(PeerId(0), false);
    table.run(20);
    assert!(table.peers[0].session.tick() > reached, "the backlog must let it catch up");
    let caught_up = table.peers[0].hashes.len();
    assert_eq!(table.peers[0].hashes, table.peers[1].hashes[..caught_up]);
}

#[test]
fn a_lossy_link_ends_the_session_which_is_why_the_transport_must_be_reliable() {
    let mut table = Table::new(Config::battle(), 2, 3);
    table.run(10);
    let reached = table.peers[0].session.tick();

    table.net.partition(PeerId(0), true);
    table.run(10);
    table.net.partition(PeerId(0), false);
    table.run(50);

    assert_eq!(
        table.peers[0].session.tick(),
        reached,
        "a dropped tick packet is unrecoverable: there is no retransmission and no skipping"
    );
    assert!(!table.peers[0].session.is_halted(), "it waits forever; the caller must time it out");
}

#[test]
fn a_timeout_is_something_the_caller_declares() {
    let mut table = Table::new(Config::battle(), 2, 3);
    table.run(5);
    table.net.partition(PeerId(0), true);
    table.run(5);
    let peer = &mut table.peers[0];
    peer.session.halt(HaltReason::Timeout);
    assert!(matches!(
        peer.session.advance(&mut peer.sim),
        Advance::Halted(HaltReason::Timeout)
    ));
}

