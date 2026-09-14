#![allow(unused_imports)]
use super::*;
use super::simulation::*;
use std::io::Write;
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};
use common::{Order, ToySim};
use l2_net::{
    decode_all, frame, Advance, Canonical, Config, Endpoint, Fixed, FrameReader, HaltReason,
    Hello, Loopback, Message, Mismatch, PeerId, PlayerSlot, Session, SessionError, TcpTransport,
    Tick, Transport, TransportError, MAX_FRAME, MAX_OUTBOX,
};

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
/// the day of the demo.
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

/// A clean close is not a loss.
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
    // which is what makes this a real partial write
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
/// dropped peer depends on whether the RST has arrived
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
        quirks: 0,
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
    // a different mod set is caught before tick 0
    // an hour in.
    let modded = Hello { ruleset_hash: 0xdead_beef, ..received };
    assert_eq!(
        mine.check(&modded),
        vec![Mismatch::Ruleset { ours: 0x1111_2222_3333_4444, theirs: 0xdead_beef }]
    );

    let (_, bytes) = wait_for_message(&mut client);
    assert!(matches!(decode_all::<Message>(&bytes), Ok(Message::Hello(_))));
}

