#![allow(unused_imports)]
use super::*;
use super::flow_tests::*;
use super::divergence_tests::*;
use common::{Order, ToySim};
use l2_net::{
    decode_all, frame, Advance, Canonical, Config, Endpoint, FrameReader, HaltReason, Loopback,
    Message, PeerId, PlayerSlot, Fixed, Session, SessionError, Tick, Transport,
};

// --- the property everything else rests on ----------------------------

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

/// The other half of §6's claim. One peer's damage rolls are off by
/// one — a peer that is *almost* right, which is what a real desync
/// looks like.
#[test]
fn a_perturbed_peer_is_caught_and_both_sides_halt() {
    let mut table = Table::new(Config::battle(), 2, 7);
    table.peers[1].sim.bias = 1;

    // Nothing diverges until the biased code path
    // is the honest shape of the problem: a desync detector cannot see
    // a difference that has not been computed yet.
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

/// §6: halt. A halted session must not step again no matter how much it
/// is pumped, because continuing turns a reproducible bug into an
/// unreproducible one.
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

/// The first halt reason sticks: a session that desynced and was then
/// torn down must still say it desynced.
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

// --- the network's opinion must not matter -----------------------------

/// §4: commands are concatenated in a fixed order, never in arrival
/// order, because arrival order is the network's opinion and it differs
/// per peer. Latency and reordering must therefore change *nothing*
/// about the result — only how long it takes.
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

    // A delayed peer gets fewer ticks done in the same number of
    // rounds, but every tick it did complete must be identical.
    for (name, run) in [("delayed", &delayed), ("jumbled", &jumbled), ("both", &both)] {
        assert!(!run.is_empty(), "{name} produced nothing");
        assert_eq!(
            run[..],
            clean[..run.len()],
            "{name} produced different checksums from the clean run"
        );
    }
}

// --- stalling ----------------------------------------------------------

/// §4: if a peer's packet has not arrived, the simulation blocks. It
/// does not extrapolate and it does not guess, because proceeding is a
/// desync and a desync is worse than a pause.
#[test]
fn a_stalled_link_pauses_the_session_and_it_resumes_intact() {
    let mut table = Table::new(Config::battle(), 2, 3);
    table.run(10);
    let reached = table.peers[0].session.tick();
    assert!(reached > Tick(4));

    // Peer 0 stops hearing from peer 1 — nothing lost, everything
    // late, which is what a quiet peer looks like over a reliable
    // transport.
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

    // And it resumes, having missed nothing.
    table.net.hold(PeerId(0), false);
    table.run(20);
    assert!(table.peers[0].session.tick() > reached, "the backlog must let it catch up");
    let caught_up = table.peers[0].hashes.len();
    assert_eq!(table.peers[0].hashes, table.peers[1].hashes[..caught_up]);
}

/// The same experiment with a link that *discards* instead of holding.
///
/// The session never recovers
/// defect: tick `N` cannot be simulated without every peer's commands
/// for it, nothing in the design retransmits, and §4 says plainly that
/// proceeding without them is a desync. This is the concrete form of
/// §7's third requirement — reliable, ordered delivery — and the reason
/// the transport may not be a bare UDP socket.
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

/// The timeout §4 asks for is the caller's to impose, because this
/// crate has no clock (D-5). What it offers is a way to say so.
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

