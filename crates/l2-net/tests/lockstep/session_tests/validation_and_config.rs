#![allow(unused_imports)]
use super::*;
use super::session_lifecycle::*;
use super::*;
use super::flow_tests::*;
use super::divergence_tests::*;
use common::{Order, ToySim};
use l2_net::{
    decode_all, frame, Advance, Canonical, Config, Endpoint, FrameReader, HaltReason, Loopback,
    Message, PeerId, PlayerSlot, Fixed, Session, SessionError, Tick, Transport,
};

// --- input delay --------------------------------------------------------

// --- refusing bad packets ------------------------------------------------

fn lone_session() -> (Session, ToySim, [PlayerSlot; 2]) {
    let slots = [PlayerSlot::new(0), PlayerSlot::new(1)];
    let sim = ToySim::new(1, 2);
    let session = Session::new(Config::battle(), slots[0], &slots, 1, &sim);
    (session, sim, slots)
}

#[test]
fn a_packet_claiming_to_be_from_us_is_refused() {
    let (mut session, _sim, slots) = lone_session();
    let packet = l2_net::TickPacket::empty(Tick(0), slots[0]);
    assert_eq!(session.receive(packet), Err(SessionError::OwnSlot(slots[0])));
}

#[test]
fn a_packet_from_a_stranger_is_refused() {
    let (mut session, _sim, _slots) = lone_session();
    let packet = l2_net::TickPacket::empty(Tick(0), PlayerSlot::new(3));
    assert_eq!(session.receive(packet), Err(SessionError::UnknownSlot(PlayerSlot::new(3))));
}

#[test]
fn a_second_packet_for_a_tick_is_refused() {
    let (mut session, _sim, slots) = lone_session();
    session.receive(l2_net::TickPacket::empty(Tick(0), slots[1])).unwrap();
    assert_eq!(
        session.receive(l2_net::TickPacket::empty(Tick(0), slots[1])),
        Err(SessionError::Duplicate { slot: slots[1], tick: Tick(0) })
    );
}

/// An integrity check: a packet whose
/// contents contradict its header is far more likely to be a relaying
/// bug in our own host code than an attack.
#[test]
fn a_packet_carrying_someone_elses_command_is_refused() {
    let (mut session, _sim, slots) = lone_session();
    let mut packet = l2_net::TickPacket::empty(Tick(0), slots[1]);
    packet.commands.push(l2_net::Command::new(slots[0], 0, Vec::new()));
    assert_eq!(
        session.receive(packet),
        Err(SessionError::ForgedCommand { packet_from: slots[1], command_from: slots[0] })
    );
}

#[test]
fn a_packet_far_beyond_the_horizon_is_refused() {
    let (mut session, _sim, slots) = lone_session();
    let packet = l2_net::TickPacket::empty(Tick(10_000), slots[1]);
    assert!(matches!(session.receive(packet), Err(SessionError::TooFarAhead { .. })));
}

#[test]
fn a_packet_for_a_tick_already_run_is_reported() {
    let mut table = Table::new(Config::battle(), 2, 4);
    table.run(6);
    let simulated = table.peers[0].session.tick().minus(1);
    let stale = l2_net::TickPacket::empty(simulated, PlayerSlot::new(1));
    assert_eq!(
        table.peers[0].session.receive(stale),
        Err(SessionError::AlreadySimulated { tick: simulated })
    );
}

#[test]
fn session_errors_explain_themselves() {
    assert_eq!(
        SessionError::UnknownSlot(PlayerSlot::new(2)).to_string(),
        "player 2 is not in this session"
    );
}

// --- session bookkeeping --------------------------------------------------

#[test]
fn a_session_reports_what_it_was_built_with() {
    let (session, _sim, slots) = lone_session();
    assert_eq!(session.local_slot(), slots[0]);
    assert_eq!(session.slots(), &slots);
    assert_eq!(session.seed(), 1);
    assert_eq!(session.tick(), Tick(0));
    assert_eq!(session.config().input_delay, 2);
}

#[test]
#[should_panic(expected = "must be one of the session's slots")]
fn a_session_whose_local_player_is_not_in_it_is_a_bug() {
    let sim = ToySim::new(1, 1);
    let _ = Session::new(Config::battle(), PlayerSlot::new(3), &[PlayerSlot::new(0)], 1, &sim);
}

#[test]
fn slots_are_sorted_and_deduplicated() {
    let sim = ToySim::new(1, 1);
    let given = [PlayerSlot::new(2), PlayerSlot::new(0), PlayerSlot::new(2)];
    let session = Session::new(Config::battle(), PlayerSlot::new(0), &given, 1, &sim);
    assert_eq!(session.slots(), &[PlayerSlot::new(0), PlayerSlot::new(2)]);
}

#[test]
fn recording_can_be_turned_off() {
    let mut config = Config::battle();
    config.record = false;
    let mut table = Table::new(config, 2, 1);
    table.run(5);
    assert!(table.peers[0].session.replay().is_none());
    assert!(table.peers[0].session.dump(&table.peers[0].sim).is_none());
}

// --- the dump -------------------------------------------------------------


