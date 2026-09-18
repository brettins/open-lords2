#![allow(unused_imports)]
use super::*;
use super::session_tests::*;
use super::divergence_tests::*;
use common::{Order, ToySim};
use l2_net::{
    decode_all, frame, Advance, Canonical, Config, Endpoint, FrameReader, HaltReason, Loopback,
    Message, PeerId, PlayerSlot, Fixed, Session, SessionError, Tick, Transport,
};

#[test]
fn a_command_executes_after_the_input_delay() {
    let mut table = Table::new(Config::battle(), 2, 1);
    assert_eq!(table.peers[0].session.execution_tick(), Tick(2));

    table.peers[0].session.issue(Order::Attack { unit: 0 }.payload());
    assert_eq!(table.peers[0].session.staged(), 1);
    table.run(6);

    let replay = table.peers[0].session.replay().expect("recording");
    let commands: Vec<Tick> = replay.commands.iter().map(|(tick, _)| *tick).collect();
    assert_eq!(commands, vec![Tick(2)], "a command issued at tick 0 must run at tick 2");
    assert_eq!(table.peers[0].session.staged(), 0);
}

#[test]
fn the_first_packets_prime_the_pipeline() {
    let sim = ToySim::new(1, 2);
    let slots = [PlayerSlot::new(0), PlayerSlot::new(1)];
    let mut session = Session::new(Config::battle(), slots[0], &slots, 1, &sim);
    session.issue(Order::Attack { unit: 0 }.payload());

    let first = session.next_packet().expect("tick 0");
    let second = session.next_packet().expect("tick 1");
    let third = session.next_packet().expect("tick 2");
    assert_eq!(session.next_packet(), None, "one packet per tick and no more");

    assert_eq!(first.tick, Tick(0));
    assert!(first.commands.is_empty(), "nothing can execute before the delay has elapsed");
    assert_eq!(second.tick, Tick(1));
    assert!(second.commands.is_empty());
    assert_eq!(third.tick, Tick(2));
    assert_eq!(third.commands.len(), 1);
}

#[test]
fn a_packet_is_sent_every_tick_even_with_nothing_to_say() {
    let mut table = Table::new(Config::battle(), 2, 1);
    table.run(12);

    let simulated = table.peers[0].hashes.len() as u32;
    assert!(simulated >= 9, "only {simulated} ticks ran");
    let expected: Vec<Tick> = (0..table.peers[0].sealed.len() as u32).map(Tick).collect();
    assert_eq!(table.peers[0].sealed, expected);
    assert!(table.peers[0].sealed.len() as u32 >= simulated);

    while let Some(packet) = table.peers[0].session.next_packet() {
        assert!(packet.commands.is_empty());
        table.peers[0].sealed.push(packet.tick);
    }
    assert_eq!(
        *table.peers[0].sealed.last().unwrap(),
        table.peers[0].session.tick().plus(Config::battle().input_delay)
    );
}


#[test]
fn the_kingdom_layer_is_the_same_mechanism_with_different_numbers() {
    let mut table = Table::new(Config::kingdom(), 5, 0x10ad);
    assert_eq!(table.peers[0].session.execution_tick(), Tick(0));

    for turn in 0..8 {
        for player in 0..5 {
            table.peers[player]
                .session
                .issue(Order::Attack { unit: turn as u32 % 4 }.payload());
        }
        table.pump();
    }

    let first = &table.peers[0].hashes;
    assert!(first.len() >= 7, "only {} turns resolved", first.len());
    for peer in &table.peers {
        assert_eq!(&peer.hashes, first, "{} disagreed", peer.slot);
        assert!(!peer.session.is_halted());
    }

    let replay = table.peers[0].session.replay().unwrap();
    let turn_one = replay.commands_at(Tick(1));
    assert_eq!(turn_one.len(), 5);
    let slots: Vec<u8> = turn_one.iter().map(|c| c.slot.index()).collect();
    assert_eq!(slots, vec![0, 1, 2, 3, 4]);
}


#[test]
fn a_sparser_exchange_still_catches_a_divergence() {
    let mut config = Config::battle();
    config.exchange_every = 4;
    let mut table = Table::new(config, 2, 7);
    table.peers[1].sim.bias = 1;
    table.peers[0].session.issue(Order::Attack { unit: 0 }.payload());
    table.run(30);

    assert!(table.peers[0].session.is_halted());
    let divergence = table.peers[0].session.divergence().unwrap();
    assert_eq!(divergence.tick.0 % 4, 0, "only exchanged ticks can be compared");
    let history = table.peers[0].session.history();
    assert!(history.hash_at(divergence.tick.minus(1)).is_some());
}

#[test]
fn the_history_ring_holds_the_last_n_ticks_and_no_more() {
    let mut config = Config::battle();
    config.history = 8;
    let mut table = Table::new(config, 2, 2);
    table.run(40);

    let history = table.peers[0].session.history();
    assert_eq!(history.len(), 8);
    let last = history.last().unwrap().0;
    assert_eq!(history.earliest(), Some(last.minus(7)));
    assert!(history.hash_at(last).is_some());
    assert!(history.hash_at(last.minus(8)).is_none(), "older than the ring must be forgotten");
}

