//! The TCP transport, over real sockets.
//!
//! **Every test in this file opens sockets, and every one of them runs
//! on a bare checkout.** Nothing here is `#[ignore]`d, nothing is gated
//! on an environment variable, nothing needs a game install and nothing
//! needs a second machine: both ends are on the loopback interface, in
//! this process, driven by the test. That was the standing condition
//! for writing a socket implementation at all — `src/transport.rs`
//! spent a long time arguing that a transport whose tests are skipped
//! by default is code that looks finished and has never worked — and it
//! turns out to cost nothing.
//!
//! Two mechanics make that true and are worth copying:
//!
//! * **Port 0.** Every listener asks the OS for a free port, so nothing
//!   here collides with a running game, with another test, or with the
//!   same test running twice. `DEFAULT_PORT` is never bound.
//! * **`127.0.0.1`, never `0.0.0.0`.** Binding the loopback address
//!   specifically keeps the Windows firewall out of it; binding all
//!   interfaces raises a prompt the first time, which is exactly the
//!   "environment-dependent test" this file exists to avoid.
//!
//! # Why there is a clock in this file and nowhere else
//!
//! `std::time` and `std::thread::sleep` appear below, and they appear
//! in `src/tcp.rs`'s neighbourhood and nowhere else in the crate. That
//! boundary is the point.
//!
//! Delivery is the kernel's business: when a byte written on one socket
//! becomes readable on another is scheduler-dependent, and no amount of
//! care makes it otherwise. So the socket tests wait, and waiting needs
//! a clock. What must *not* happen is that quantity reaching the
//! simulation — D-5 — and the tests below assert precisely that it does
//! not: [`the_socket_changes_the_timing_and_not_the_checksums`] runs
//! one schedule over real sockets and the identical schedule over the
//! in-process [`Loopback`], and requires the two to produce the same
//! checksum for every tick. The network decides *when*; it never
//! decides *what*.
//!
//! That is why `tests/lockstep.rs` remains the more important file. It
//! has no clock at all, so it can assert things about ordering that a
//! socket test can only sample.
//!
//! [`the_socket_changes_the_timing_and_not_the_checksums`]:
//!     fn.the_socket_changes_the_timing_and_not_the_checksums.html

mod common;

use std::io::Write;
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use common::{Order, ToySim};
use l2_net::{
    decode_all, frame, Advance, Canonical, Config, Endpoint, Fixed, FrameReader, HaltReason,
    Hello, Loopback, Message, Mismatch, PeerId, PlayerSlot, Session, SessionError, TcpTransport,
    Tick, Transport, TransportError, MAX_FRAME, MAX_OUTBOX,
};

/// How long any test will wait for the loopback interface before
/// calling it a failure.
///
/// Absurdly generous — everything here completes in milliseconds — because
/// the only thing this bound is for is turning a hang into a readable
/// failure on a machine under load. A tight timeout would make the
/// suite flaky, which is worse than slow.
const PATIENCE: Duration = Duration::from_secs(30);

/// A listener on an OS-assigned loopback port, and its address.
fn host() -> (TcpTransport, SocketAddr) {
    let host = TcpTransport::listen("127.0.0.1:0").expect("binding a loopback port");
    let addr = host.local_addr().expect("a listener knows its address");
    (host, addr)
}

fn deadline() -> Instant {
    Instant::now() + PATIENCE
}

fn breathe() {
    // One millisecond, so a spin loop is not a busy loop. Nothing in
    // the crate depends on this number; it only trades CPU for latency
    // in the tests.
    std::thread::sleep(Duration::from_millis(1));
}

/// Poll until a message arrives, or fail.
#[track_caller]
fn wait_for_message(net: &mut TcpTransport) -> (PeerId, Vec<u8>) {
    let until = deadline();
    loop {
        if let Some(message) = net.poll() {
            return message;
        }
        assert!(Instant::now() < until, "nothing arrived within {PATIENCE:?}");
        breathe();
    }
}

/// Accept until `count` peers are connected, or fail.
///
/// Uses `accept_pending` rather than `poll` so that a message racing
/// the connection is not swallowed.
#[track_caller]
fn wait_for_peers(net: &mut TcpTransport, count: usize) {
    let until = deadline();
    while net.peers().len() < count {
        net.accept_pending().expect("accepting");
        assert!(
            Instant::now() < until,
            "only {} of {count} peers connected within {PATIENCE:?}",
            net.peers().len()
        );
        breathe();
    }
}

/// Poll until the transport reports a departure, or fail.
#[track_caller]
fn wait_for_departure(net: &mut TcpTransport) -> Vec<PeerId> {
    let until = deadline();
    loop {
        while net.poll().is_some() {}
        let gone = net.take_disconnected();
        if !gone.is_empty() {
            return gone;
        }
        assert!(Instant::now() < until, "no departure within {PATIENCE:?}");
        breathe();
    }
}

// --- the socket itself -----------------------------------------------

#[test]
fn a_message_crosses_a_real_socket_intact() {
    let (mut host, addr) = host();
    let mut client = TcpTransport::connect(addr, PeerId(7)).expect("connecting");

    client.send(PeerId(7), b"move 7 to 34,12").expect("sending");
    assert_eq!(wait_for_message(&mut host), (PeerId(0), b"move 7 to 34,12".to_vec()));

    // And back the other way, on the same connection.
    host.send(PeerId(0), b"acknowledged").expect("sending");
    assert_eq!(wait_for_message(&mut client), (PeerId(7), b"acknowledged".to_vec()));
}

/// The property the [`Transport`] trait promises and a stream does not:
/// `send` takes one message and `poll` returns one message. Sizes
/// chosen so that several fit in one kernel read and one spans several.
#[test]
fn message_boundaries_survive_the_stream() {
    let (mut host, addr) = host();
    let mut client = TcpTransport::connect(addr, PeerId(0)).expect("connecting");

    let messages: Vec<Vec<u8>> = vec![
        b"first".to_vec(),
        Vec::new(),
        vec![0x5au8; 100_000],
        b"x".to_vec(),
        vec![0xa5u8; 3],
    ];
    for message in &messages {
        client.send(PeerId(0), message).expect("sending");
    }

    let mut received = Vec::new();
    while received.len() < messages.len() {
        received.push(wait_for_message(&mut host).1);
    }
    assert_eq!(received, messages);
    assert_eq!(host.poll(), None, "nothing extra came out of the stream");
}

/// A partial frame arriving across two reads — the case that happens on
/// the day of the demo, here against a real socket rather than a
/// `FrameReader` fed by hand.
///
/// Every split point is tried, including the three inside the
/// four-byte length prefix. The `None` assertion cannot race: fewer
/// bytes than one whole message have been written, so no complete
/// message can exist however much of it the kernel has delivered.
#[test]
fn a_frame_split_across_two_writes_is_reassembled() {
    let (mut host, addr) = host();
    let mut raw = TcpStream::connect(addr).expect("connecting");
    raw.set_nodelay(true).expect("nodelay");
    wait_for_peers(&mut host, 1);
    let peer = host.peers()[0];

    let payload = b"a tick packet".to_vec();
    let framed = frame(&payload).expect("framing");
    for split in 1..framed.len() {
        raw.write_all(&framed[..split]).expect("writing the first part");
        raw.flush().expect("flushing");
        for _ in 0..5 {
            assert_eq!(host.poll(), None, "half a message came out, split at {split}");
        }
        raw.write_all(&framed[split..]).expect("writing the rest");
        raw.flush().expect("flushing");
        assert_eq!(
            wait_for_message(&mut host),
            (peer, payload.clone()),
            "the message did not reassemble, split at {split}"
        );
    }
}

/// §7's most expensive detail. Nagle costs most of a round trip per
/// tick and presents as "the network is slow".
#[test]
fn nagle_is_off_on_both_ends() {
    let (mut host, addr) = host();
    let client = TcpTransport::connect(addr, PeerId(0)).expect("connecting");
    wait_for_peers(&mut host, 1);

    assert_eq!(client.nodelay(PeerId(0)), Some(true), "the connecting side");
    assert_eq!(host.nodelay(host.peers()[0]), Some(true), "the accepting side");
}

/// Errata 7, which is the reason `peers()` is on the trait: the
/// transport knows who is connected, and a caller keeping its own list
/// is a caller whose list drifts — which surfaces as one player
/// silently not receiving turns.
#[test]
fn peers_reflects_who_is_connected_without_the_caller_keeping_a_list() {
    let (mut host, addr) = host();
    assert_eq!(host.peers(), vec![], "nobody has connected yet");

    let first = TcpTransport::connect(addr, PeerId(0)).expect("connecting");
    wait_for_peers(&mut host, 1);
    assert_eq!(host.peers(), vec![PeerId(0)]);
    assert_eq!(host.take_joined(), vec![PeerId(0)]);

    let _second = TcpTransport::connect(addr, PeerId(0)).expect("connecting");
    wait_for_peers(&mut host, 2);
    assert_eq!(host.peers(), vec![PeerId(0), PeerId(1)], "in id order, always");
    assert_eq!(host.take_joined(), vec![PeerId(1)], "only the new one");

    drop(first);
    assert_eq!(wait_for_departure(&mut host), vec![PeerId(0)]);
    assert_eq!(host.peers(), vec![PeerId(1)], "the survivor, and only the survivor");

    // Ids are never reused: a reconnecting player is the same
    // `PlayerSlot` on a new `PeerId`, and handing out the dead one
    // would let a stale reference address the new connection.
    let _third = TcpTransport::connect(addr, PeerId(0)).expect("connecting");
    wait_for_peers(&mut host, 2);
    assert_eq!(host.peers(), vec![PeerId(1), PeerId(2)]);
}

/// Late join at the transport level: a peer that connects long after
/// the host started polling is picked up by the ordinary drain, with no
/// special call.
#[test]
fn a_peer_that_connects_late_is_accepted_by_poll() {
    let (mut host, addr) = host();
    for _ in 0..50 {
        assert_eq!(host.poll(), None);
        assert_eq!(host.peers(), vec![]);
    }

    let mut client = TcpTransport::connect(addr, PeerId(0)).expect("connecting");
    client.send(PeerId(0), b"sorry I'm late").expect("sending");
    assert_eq!(wait_for_message(&mut host), (PeerId(0), b"sorry I'm late".to_vec()));
    assert_eq!(host.peers(), vec![PeerId(0)]);
}

/// A clean close is not a loss. Everything sent before it is delivered
/// first, and the departure is announced only once the last of it has
/// been handed over — which is the difference between a session ending
/// tidily and one that throws away its peer's final tick packets and
/// reports a phantom stall.
#[test]
fn a_clean_close_delivers_what_was_sent_before_it() {
    let (mut host, addr) = host();
    let mut client = TcpTransport::connect(addr, PeerId(0)).expect("connecting");

    client.send(PeerId(0), b"last words").expect("sending");
    client.flush().expect("flushing");
    drop(client);

    assert_eq!(wait_for_message(&mut host).1, b"last words".to_vec());
    assert_eq!(wait_for_departure(&mut host), vec![PeerId(0)]);
    assert_eq!(host.peers(), vec![]);
}

/// A peer that dies halfway through writing a packet.
///
/// The truncated bytes are unusable and go either way; the point is
/// that it is reported. Silently discarding them leaves a session
/// waiting forever for a tick — which is *correct* behaviour with no
/// clock, and therefore indistinguishable from an ordinary stall unless
/// somebody says the stream ended mid-message.
#[test]
fn a_peer_that_dies_mid_message_says_so_rather_than_stalling_silently() {
    let (mut host, addr) = host();
    let mut raw = TcpStream::connect(addr).expect("connecting");
    wait_for_peers(&mut host, 1);

    let framed = frame(b"half of a tick packet").expect("framing");
    raw.write_all(&framed[..6]).expect("writing half");
    raw.flush().expect("flushing");
    drop(raw);

    let until = deadline();
    let mut faults = Vec::new();
    while faults.is_empty() {
        while host.poll().is_some() {}
        faults = host.take_faults();
        assert!(Instant::now() < until, "the truncation was never reported");
        breathe();
    }
    assert_eq!(faults.len(), 1);
    assert_eq!(faults[0].0, Some(PeerId(0)));
    match &faults[0].1 {
        TransportError::Io(detail) => {
            assert!(detail.contains("incomplete message"), "unhelpful detail: {detail}");
        }
        other => panic!("expected an Io fault naming the truncation, got {other:?}"),
    }
    assert_eq!(host.peers(), vec![]);
}

#[test]
fn sending_to_a_departed_peer_fails_instead_of_vanishing() {
    let (mut host, addr) = host();
    let client = TcpTransport::connect(addr, PeerId(0)).expect("connecting");
    wait_for_peers(&mut host, 1);
    drop(client);
    wait_for_departure(&mut host);

    assert_eq!(host.send(PeerId(0), b"anyone?"), Err(TransportError::NoSuchPeer(PeerId(0))));
}

#[test]
fn sending_to_a_peer_that_never_existed_is_an_error() {
    let (mut host, _addr) = host();
    assert_eq!(host.send(PeerId(9), b"x"), Err(TransportError::NoSuchPeer(PeerId(9))));
}

/// The reason [`MAX_FRAME`] exists: a length prefix is attacker- or
/// bug-controlled, and believing it means allocating a gigabyte before
/// noticing. The stream is unrecoverable afterwards — we no longer know
/// where the next message starts — so the connection goes.
#[test]
fn a_hostile_length_prefix_kills_the_connection_instead_of_allocating() {
    let (mut host, addr) = host();
    let mut raw = TcpStream::connect(addr).expect("connecting");
    wait_for_peers(&mut host, 1);

    raw.write_all(&u32::MAX.to_le_bytes()).expect("writing a lie");
    raw.flush().expect("flushing");

    let until = deadline();
    let mut faults = Vec::new();
    while faults.is_empty() {
        while host.poll().is_some() {}
        faults = host.take_faults();
        assert!(Instant::now() < until, "the bad prefix was never noticed");
        breathe();
    }
    assert_eq!(faults.len(), 1);
    assert_eq!(faults[0].0, Some(PeerId(0)), "attributed to the peer that sent it");
    assert!(
        matches!(faults[0].1, TransportError::FrameTooLong { .. }),
        "expected FrameTooLong, got {:?}",
        faults[0].1
    );
    assert_eq!(host.peers(), vec![], "the connection is not usable after that");
}

#[test]
fn a_message_over_the_frame_limit_is_refused_before_it_is_sent() {
    let (mut host, addr) = host();
    let _client = TcpTransport::connect(addr, PeerId(0)).expect("connecting");
    wait_for_peers(&mut host, 1);

    let huge = vec![0u8; MAX_FRAME + 1];
    assert_eq!(
        host.send(PeerId(0), &huge),
        Err(TransportError::FrameTooLong { len: MAX_FRAME + 1 })
    );
    // And the connection is still perfectly good.
    host.send(PeerId(0), b"still here").expect("sending");
}

/// A message far larger than any kernel send buffer, which is the only
/// way to exercise the partial-write path: `send` must own the whole
/// message even when the kernel takes a fraction of it, and the
/// remainder must go out later without the caller doing anything.
#[test]
fn a_large_message_survives_a_partial_write() {
    let (mut host, addr) = host();
    let mut client = TcpTransport::connect(addr, PeerId(0)).expect("connecting");

    let payload: Vec<u8> = (0..900_000u32).map(|i| (i % 251) as u8).collect();
    client.send(PeerId(0), &payload).expect("sending");

    // The receiver has to be pumped for the sender's outbox to drain,
    // which is what makes this a real partial write rather than a
    // memcpy: both ends are in this thread, so nothing moves unless the
    // test moves it.
    let until = deadline();
    let mut received = None;
    while received.is_none() {
        received = host.poll().map(|(_, bytes)| bytes);
        client.flush().expect("flushing");
        assert!(Instant::now() < until, "the large message never arrived");
    }
    assert_eq!(received.unwrap(), payload);
    assert_eq!(client.pending_out(PeerId(0)), 0, "nothing was left stuck in the outbox");
}

/// A peer that has stopped reading must not be able to grow our memory
/// without bound. At [`MAX_OUTBOX`] the connection is declared dead,
/// because four mebibytes of unread backlog — four times the largest
/// message the design has — is a peer that is not coming back.
#[test]
fn an_unread_backlog_is_bounded_rather_than_unbounded() {
    let (mut host, addr) = host();
    // Never polled, so nothing is ever read from this end.
    let _client = TcpTransport::connect(addr, PeerId(0)).expect("connecting");
    wait_for_peers(&mut host, 1);

    let message = vec![0u8; 1 << 20];
    let mut refused = None;
    // Sixteen mebibytes is well past any plausible socket buffer, so
    // the cap is what stops this and not the test's patience.
    for _ in 0..16 {
        if let Err(e) = host.send(PeerId(0), &message) {
            refused = Some(e);
            break;
        }
    }
    let refused = refused.expect("the backlog cap was never reached");
    assert!(
        matches!(refused, TransportError::Io(_)),
        "expected a reported fault, got {refused:?}"
    );
    assert!(host.pending_out(PeerId(0)) <= MAX_OUTBOX);
    assert_eq!(host.peers(), vec![], "the peer is written off, not retried forever");
    assert_eq!(host.take_faults().len(), 1);
}

/// A broadcast during a peer's departure still reaches everyone else.
///
/// This is errata 7 arriving through the other door. The trait's
/// default `broadcast` uses `?`, so the first failing peer aborts the
/// loop and every peer *after* it in the list is silently skipped: the
/// host broadcasts a turn, player 2 has just dropped, and players 3, 4
/// and 5 never receive it. [`TcpTransport`] overrides it to attempt
/// every peer and report afterwards.
///
/// Note what this test can and cannot pin down. Whether the send to the
/// dropped peer actually fails depends on whether the RST has arrived
/// yet, which is not ours to schedule — so sometimes this exercises the
/// override and sometimes it exercises the happy path. The assertion is
/// the part that must hold either way: the live peer gets the message.
#[test]
fn a_broadcast_reaches_the_living_when_one_peer_is_going() {
    let (mut host, addr) = host();
    let leaving = TcpTransport::connect(addr, PeerId(0)).expect("connecting");
    let mut staying = TcpTransport::connect(addr, PeerId(0)).expect("connecting");
    wait_for_peers(&mut host, 2);
    assert_eq!(host.peers(), vec![PeerId(0), PeerId(1)], "the leaver is first in the list");

    drop(leaving);
    let _ = host.broadcast(b"turn 4 begins");

    assert_eq!(wait_for_message(&mut staying).1, b"turn 4 begins".to_vec());
}

/// The handshake over a real socket, which is the only thing that tells
/// a caller *who* a [`PeerId`] is.
///
/// The transport deliberately cannot answer that: a peer id is where
/// bytes come from and a [`PlayerSlot`] is who someone is in the game,
/// and in the star topology §5 describes they differ permanently. So
/// the binding between them is made here, out of a [`Hello`] that also
/// carries D-12's version, ruleset and seed checks — because two peers
/// who disagree about any of those will desync immediately and with a
/// baffling symptom.
#[test]
fn the_handshake_crosses_the_same_socket_and_binds_a_slot_to_a_peer() {
    let (mut host, addr) = host();
    let mut client = TcpTransport::connect(addr, PeerId(0)).expect("connecting");
    // The host has no peer to address until it has accepted one, and
    // accepting happens inside `poll`/`accept_pending`. A host that
    // sends before draining gets `NoSuchPeer`, which is the correct
    // answer and a surprising one the first time.
    wait_for_peers(&mut host, 1);

    let mine = Hello {
        protocol: l2_net::PROTOCOL_VERSION,
        engine: "l2-net test build".to_string(),
        ruleset_hash: 0x1111_2222_3333_4444,
        seed: 0xfeed_beef,
        slot: PlayerSlot::new(0),
    };
    let theirs = Hello { slot: PlayerSlot::new(1), ..mine.clone() };

    host.send(PeerId(0), &Canonical::bytes_of(&Message::Hello(mine.clone()))).unwrap();
    client.send(PeerId(0), &Canonical::bytes_of(&Message::Hello(theirs.clone()))).unwrap();

    let (peer, bytes) = wait_for_message(&mut host);
    let Message::Hello(received) = decode_all::<Message>(&bytes).expect("a Hello") else {
        panic!("expected a Hello");
    };
    assert_eq!(received, theirs, "the handshake survived the socket byte for byte");
    assert_eq!(mine.check(&received), vec![], "two matching peers have nothing to report");

    // This is the binding, and it is the caller's to make: peer 0 on
    // this transport is player 1 in the game.
    assert_eq!((peer, received.slot), (PeerId(0), PlayerSlot::new(1)));

    // And the check that earns the handshake its place: a peer running
    // a different mod set is caught before tick 0 rather than desyncing
    // an hour in.
    let modded = Hello { ruleset_hash: 0xdead_beef, ..received };
    assert_eq!(
        mine.check(&modded),
        vec![Mismatch::Ruleset { ours: 0x1111_2222_3333_4444, theirs: 0xdead_beef }]
    );

    let (_, bytes) = wait_for_message(&mut client);
    assert!(matches!(decode_all::<Message>(&bytes), Ok(Message::Hello(_))));
}

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
        // core rather than about the test's timing.
        let at = self.session.execution_tick();
        if self.issued_through.is_none_or(|last| at > last) {
            self.issued_through = Some(at);
            if let Some(order) = schedule(self.slot.index(), at) {
                self.session.issue(order.payload());
            }
        }

        while let Some(packet) = self.session.next_packet() {
            // Note there is no `frame()` here: the transport's contract
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
        // job (`Session`'s own docs say so); here there is no pacing on
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
