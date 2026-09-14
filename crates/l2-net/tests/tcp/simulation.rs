#![allow(unused_imports)]
use super::*;
use super::transport::*;
use std::io::Write;
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};
use common::{Order, ToySim};
use l2_net::{
    decode_all, frame, Advance, Canonical, Config, Endpoint, Fixed, FrameReader, HaltReason,
    Hello, Loopback, Message, Mismatch, PeerId, PlayerSlot, Session, SessionError, TcpTransport,
    Tick, Transport, TransportError, MAX_FRAME, MAX_OUTBOX,
};

// --- a whole session, over sockets ------------------------------------

/// One peer: a session, its simulation, and its socket.
///
/// The loop in `pump` is the one `Session`'s own documentation
/// describes — issue, seal, send, drain, advance — with a real
/// transport substituted for the loopback and nothing else changed.
struct NetPeer {
    slot: PlayerSlot,
    session: Session,
    sim: ToySim,
    net: TcpTransport,
    /// Packets sealed before anyone was connected. A session with an
    /// input delay of two seals ticks 0, 1 and 2 immediately, and a
    /// peer who has not arrived yet cannot be sent them — so they wait
    /// here. This is the whole of "late join" at this layer, and it
    /// works because the transport is ordered: the backlog arrives in
    /// the order it was sealed.
    outbox: Vec<Vec<u8>>,
    hashes: Vec<(Tick, u64)>,
    errors: Vec<SessionError>,
    departed: Vec<PeerId>,
    issued_through: Option<Tick>,
}

impl NetPeer {
    fn new(slot: PlayerSlot, slots: &[PlayerSlot], seed: u64, net: TcpTransport) -> NetPeer {
        let sim = ToySim::new(seed, 6);
        let session = Session::new(Config::battle(), slot, slots, seed, &sim);
        NetPeer {
            slot,
            session,
            sim,
            net,
            outbox: Vec::new(),
            hashes: Vec::new(),
            errors: Vec::new(),
            departed: Vec::new(),
            issued_through: None,
        }
    }

    fn pump(&mut self, schedule: &dyn Fn(u8, Tick) -> Option<Order>) {
        // Commands are scheduled on the tick they will execute on, not
        // on how many rounds of the loop have gone by — the same rule
        // `tests/lockstep.rs` uses, and for the same reason: under
        // latency a keystroke lands on a later tick, which changes the
        // game correctly. Keying on the execution tick is what makes
        // "the socket changed nothing" a statement about the lockstep
        // core.
        let at = self.session.execution_tick();
        if self.issued_through.is_none_or(|last| at > last) {
            self.issued_through = Some(at);
            if let Some(order) = schedule(self.slot.index(), at) {
                self.session.issue(order.payload());
            }
        }

        while let Some(packet) = self.session.next_packet() {
            // Note; here there is no pacing on
            // is whole messages, and `TcpTransport` does the framing.
            self.outbox.push(Canonical::bytes_of(&Message::Tick(packet)));
        }

        // Errata 7: the connection list comes from the transport. This
        // peer never keeps one of its own, so it cannot drift.
        let peers = self.net.peers();
        if !peers.is_empty() {
            for bytes in self.outbox.drain(..) {
                for peer in &peers {
                    if let Err(TransportError::Io(detail)) = self.net.send(*peer, &bytes) {
                        panic!("unexpected transport failure: {detail}");
                    }
                }
            }
        }

        while let Some((_from, bytes)) = self.net.poll() {
            match decode_all::<Message>(&bytes).expect("a well-formed message") {
                Message::Tick(packet) => {
                    if let Err(error) = self.session.receive(packet) {
                        self.errors.push(error);
                    }
                }
                other => panic!("unexpected {other:?}"),
            }
        }
        self.departed.extend(self.net.take_disconnected());

        // Advancing until `Waiting` is right for a test and wrong for
        // live play — it runs the simulation as fast as the machine
        // allows, up to the input delay ahead. Pacing is the caller's
        // job (`Session`'s own docs say so)
        // purpose, so the socket is exercised as hard as it can be.
        while let Advance::Stepped { tick, hash } = self.session.advance(&mut self.sim) {
            self.hashes.push((tick, hash));
        }
    }
}

/// The schedule both the socket run and the loopback run are given.
fn schedule(slot: u8, tick: Tick) -> Option<Order> {
    match (slot, tick.0 % 7) {
        (0, 0) => Some(Order::Attack { unit: 1 }),
        (0, 3) => Some(Order::MoveTo {
            unit: 4,
            x: Fixed::from_int(tick.0 as i32 % 79),
            y: Fixed::from_int(11),
        }),
        (1, 2) => Some(Order::Attack { unit: 2 }),
        (1, 5) => Some(Order::MoveTo {
            unit: 3,
            x: Fixed::from_int(40),
            y: Fixed::from_int(tick.0 as i32 % 79),
        }),
        _ => None,
    }
}

/// Two peers on real sockets, connected before the session starts.
fn connected_pair(seed: u64) -> (NetPeer, NetPeer) {
    let slots = [PlayerSlot::new(0), PlayerSlot::new(1)];
    let (mut listener, addr) = host();
    let client_net = TcpTransport::connect(addr, PeerId(0)).expect("connecting");
    wait_for_peers(&mut listener, 1);
    (
        NetPeer::new(slots[0], &slots, seed, listener),
        NetPeer::new(slots[1], &slots, seed, client_net),
    )
}

/// Pump both peers until they have both simulated up to `target`.
#[track_caller]
fn run_to(a: &mut NetPeer, b: &mut NetPeer, target: Tick) {
    let until = deadline();
    while a.session.tick() < target || b.session.tick() < target {
        a.pump(&schedule);
        b.pump(&schedule);
        assert!(
            !a.session.is_halted() && !b.session.is_halted(),
            "a session halted early: {:?} / {:?}",
            a.session.halt_reason(),
            b.session.halt_reason()
        );
        assert!(
            Instant::now() < until,
            "stalled at {:?} and {:?}, waiting for {target}",
            a.session.tick(),
            b.session.tick()
        );
    }
}

/// The deliverable: two `Session`s exchanging real packets over real
/// sockets and reaching identical checksums, tick by tick.
#[test]
fn two_sessions_over_real_sockets_agree_on_every_tick() {
    let (mut a, mut b) = connected_pair(0xabc_def);
    run_to(&mut a, &mut b, Tick(60));

    assert_eq!(a.hashes, b.hashes, "two peers disagreed about some tick");
    assert_eq!(l2_net::state_hash(&a.sim), l2_net::state_hash(&b.sim));
    assert!(a.hashes.len() >= 60, "the session barely advanced");
    assert_eq!(a.errors, vec![]);
    assert_eq!(b.errors, vec![]);
    assert_eq!(a.sim.bad_commands, 0);
    assert_eq!(b.sim.bad_commands, 0);
    assert!(!a.session.is_halted());
    assert!(!b.session.is_halted());
    // Commands really did cross the wire. Without this the test could
    // pass on two idle peers agreeing about nothing: units 1 and 2 lose
    // hit points only through an `Attack` command, one from each side.
    assert!(a.sim.unit(1).expect("unit 1").hp < 100, "peer 0's attacks never arrived");
    assert!(a.sim.unit(2).expect("unit 2").hp < 100, "peer 1's attacks never arrived");
}

/// The same schedule over the in-process [`Loopback`], for comparison.
fn loopback_hashes(seed: u64, target: Tick) -> Vec<Vec<(Tick, u64)>> {
    let slots = [PlayerSlot::new(0), PlayerSlot::new(1)];
    let ids = [PeerId(0), PeerId(1)];
    let net = Loopback::with_peers(&ids);

    struct Local {
        slot: PlayerSlot,
        id: PeerId,
        endpoint: Endpoint,
        reader: FrameReader,
        session: Session,
        sim: ToySim,
        hashes: Vec<(Tick, u64)>,
        issued_through: Option<Tick>,
    }

    let mut peers: Vec<Local> = (0..2)
        .map(|i| {
            let sim = ToySim::new(seed, 6);
            Local {
                slot: slots[i],
                id: ids[i],
                endpoint: net.endpoint(ids[i]),
                reader: FrameReader::new(),
                session: Session::new(Config::battle(), slots[i], &slots, seed, &sim),
                sim,
                hashes: Vec::new(),
                issued_through: None,
            }
        })
        .collect();

    while peers.iter().any(|p| p.session.tick() < target) {
        for i in 0..2 {
            let at = peers[i].session.execution_tick();
            if peers[i].issued_through.is_none_or(|last| at > last) {
                peers[i].issued_through = Some(at);
                if let Some(order) = schedule(peers[i].slot.index(), at) {
                    peers[i].session.issue(order.payload());
                }
            }
            let mut sealed = Vec::new();
            while let Some(packet) = peers[i].session.next_packet() {
                sealed.push(frame(&Canonical::bytes_of(&Message::Tick(packet))).unwrap());
            }
            for bytes in sealed {
                let target_id = peers[1 - i].id;
                peers[i].endpoint.send(target_id, &bytes).unwrap();
            }
        }
        for peer in &mut peers {
            while let Some((_from, bytes)) = peer.endpoint.poll() {
                peer.reader.feed(&bytes);
            }
            while let Some(message) = peer.reader.next_message().unwrap() {
                match decode_all::<Message>(&message).unwrap() {
                    Message::Tick(packet) => peer.session.receive(packet).unwrap(),
                    other => panic!("unexpected {other:?}"),
                }
            }
            while let Advance::Stepped { tick, hash } = peer.session.advance(&mut peer.sim) {
                peer.hashes.push((tick, hash));
            }
        }
    }
    peers.into_iter().map(|p| p.hashes).collect()
}

/// D-5, stated as a test: the network decides *when* a tick happens and
/// never *what* it computes.
///
/// One schedule run twice — once through the kernel, once through an
/// in-process queue with entirely different arrival timing — must give
/// the same checksum for every tick. If a socket could ever change a
/// result, this is where it would show, and it is the assertion that
/// justifies `tests/lockstep.rs` continuing to test the interesting
/// cases with no network at all.
#[test]
fn the_socket_changes_the_timing_and_not_the_checksums() {
    let seed = 0x5eed_1234;
    let (mut a, mut b) = connected_pair(seed);
    run_to(&mut a, &mut b, Tick(40));

    let expected = loopback_hashes(seed, Tick(40));
    let common = 40;
    assert_eq!(a.hashes[..common], expected[0][..common], "sockets vs loopback, peer 0");
    assert_eq!(b.hashes[..common], expected[1][..common], "sockets vs loopback, peer 1");
}

/// A peer disconnecting mid-session.
///
/// The transport reports the departure; the *session* does nothing at
/// all, because it has no clock and no opinion about what a missing
/// peer means (D-5). Halting is the caller's decision, made here with
/// the transport's own connection list as the evidence — which is
/// exactly the arrangement errata 7 asks for.
#[test]
fn a_peer_that_disconnects_mid_session_is_seen_by_the_other() {
    let (mut a, mut b) = connected_pair(0x1234_5678);
    run_to(&mut a, &mut b, Tick(15));
    let agreed = b.hashes.len().min(a.hashes.len());
    assert!(agreed >= 15);

    // Player 2 alt-F4s.
    drop(b);

    let until = deadline();
    while a.departed.is_empty() {
        a.pump(&schedule);
        assert!(Instant::now() < until, "the departure was never noticed");
        breathe();
    }
    assert_eq!(a.departed, vec![PeerId(0)]);
    assert_eq!(a.net.peers(), vec![], "and the list the caller broadcasts to is now empty");

    // The session is *not* halted by any of that: waiting forever for a
    // peer's tick is correct behaviour with no clock. The caller
    // decides, and only the caller can.
    assert!(!a.session.is_halted());
    match a.session.advance(&mut a.sim) {
        Advance::Waiting { missing, .. } => assert_eq!(missing, vec![PlayerSlot::new(1)]),
        other => panic!("expected to be waiting for the departed peer, got {other:?}"),
    }
    a.session.halt(HaltReason::Left);
    assert!(matches!(a.session.halt_reason(), Some(HaltReason::Left)));
}

/// A peer that connects late.
///
/// The host's session starts sealing packets before anyone is there to
/// send them to; the client joins several rounds later; the backlog
/// goes out in order and both peers converge on identical checksums.
/// This is late join in the only form the design supports without a
/// state snapshot (§5) — the session has not advanced, because it
/// cannot advance without the other peer's tick 0.
#[test]
fn a_peer_that_connects_late_still_reaches_the_same_checksums() {
    let seed = 0xfeed_face;
    let slots = [PlayerSlot::new(0), PlayerSlot::new(1)];
    let (listener, addr) = host();
    let mut a = NetPeer::new(slots[0], &slots, seed, listener);

    for _ in 0..25 {
        a.pump(&schedule);
        assert_eq!(a.session.tick(), Tick(0), "nothing can advance alone");
        assert!(a.net.peers().is_empty());
    }
    assert_eq!(a.outbox.len(), 3, "ticks 0, 1 and 2 are sealed and waiting");

    let client_net = TcpTransport::connect(addr, PeerId(0)).expect("connecting late");
    let mut b = NetPeer::new(slots[1], &slots, seed, client_net);
    run_to(&mut a, &mut b, Tick(40));

    assert_eq!(a.hashes, b.hashes, "the late peer disagreed about some tick");
    assert_eq!(l2_net::state_hash(&a.sim), l2_net::state_hash(&b.sim));
    assert_eq!(a.errors, vec![]);
    assert_eq!(b.errors, vec![]);
    assert!(!a.session.is_halted());

    // And the result is the same one the no-network run produces: a
    // late connection changed the wall-clock timing and nothing else.
    let expected = loopback_hashes(seed, Tick(40));
    assert_eq!(a.hashes[..40], expected[0][..40]);
}

