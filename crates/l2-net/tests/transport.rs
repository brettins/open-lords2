//! Framing and the in-process network.
//!
//! The framing tests matter more than they look. Nothing in this crate
//! opens a socket, so `FrameReader` is the one piece of a future TCP
//! transport that exists today — and message reassembly is where
//! stream transports go wrong: a length prefix split across two reads,
//! two messages arriving in one read, a message arriving in fifty
//! reads. All three are tested exhaustively below, because none of them
//! can be tested later without a network.

use l2_net::{frame, FrameReader, Loopback, PeerId, Transport, TransportError, MAX_FRAME};

// --- framing ----------------------------------------------------------

#[test]
fn a_frame_is_a_little_endian_length_and_the_payload() {
    assert_eq!(frame(b"abc").unwrap(), vec![3, 0, 0, 0, b'a', b'b', b'c']);
    assert_eq!(frame(b"").unwrap(), vec![0, 0, 0, 0]);
}

#[test]
fn an_oversized_frame_is_refused_rather_than_truncated() {
    let huge = vec![0u8; MAX_FRAME + 1];
    assert_eq!(frame(&huge), Err(TransportError::FrameTooLong { len: MAX_FRAME + 1 }));
}

#[test]
fn messages_come_back_out_whole() {
    let mut reader = FrameReader::new();
    reader.feed(&frame(b"first").unwrap());
    reader.feed(&frame(b"second").unwrap());
    assert_eq!(reader.next_message().unwrap(), Some(b"first".to_vec()));
    assert_eq!(reader.next_message().unwrap(), Some(b"second".to_vec()));
    assert_eq!(reader.next_message().unwrap(), None);
}

/// The case that happens on the day of the demo.
#[test]
fn a_stream_delivered_one_byte_at_a_time_yields_the_same_messages() {
    let mut stream = Vec::new();
    let payloads: Vec<Vec<u8>> =
        vec![b"a".to_vec(), Vec::new(), vec![7u8; 300], b"the last one".to_vec()];
    for payload in &payloads {
        stream.extend_from_slice(&frame(payload).unwrap());
    }

    let mut reader = FrameReader::new();
    let mut received = Vec::new();
    for byte in &stream {
        reader.feed(&[*byte]);
        while let Some(message) = reader.next_message().unwrap() {
            received.push(message);
        }
    }
    assert_eq!(received, payloads);
    assert_eq!(reader.buffered(), 0);
}

/// Every possible split point, not just the interesting-looking ones —
/// including splits inside the length prefix.
#[test]
fn every_split_of_a_stream_yields_the_same_messages() {
    let mut stream = Vec::new();
    for payload in [b"one".as_slice(), b"two".as_slice(), b"three".as_slice()] {
        stream.extend_from_slice(&frame(payload).unwrap());
    }
    for split in 0..=stream.len() {
        let mut reader = FrameReader::new();
        let mut received = Vec::new();
        reader.feed(&stream[..split]);
        while let Some(message) = reader.next_message().unwrap() {
            received.push(message);
        }
        reader.feed(&stream[split..]);
        while let Some(message) = reader.next_message().unwrap() {
            received.push(message);
        }
        assert_eq!(
            received,
            vec![b"one".to_vec(), b"two".to_vec(), b"three".to_vec()],
            "split at {split}"
        );
    }
}

#[test]
fn a_partial_message_is_held_not_returned() {
    let mut reader = FrameReader::new();
    reader.feed(&[10, 0, 0, 0, b'a']);
    assert_eq!(reader.next_message().unwrap(), None);
    assert_eq!(reader.buffered(), 5);
}

#[test]
fn a_hostile_length_prefix_is_refused_before_it_is_believed() {
    let mut reader = FrameReader::new();
    reader.feed(&u32::MAX.to_le_bytes());
    assert!(matches!(reader.next_message(), Err(TransportError::FrameTooLong { .. })));
}

/// A busy stream must not become quadratic in the number of messages.
/// The reader compacts rather than draining from the front each time.
#[test]
fn a_long_stream_of_small_messages_does_not_accumulate() {
    let mut reader = FrameReader::new();
    for _ in 0..50_000 {
        reader.feed(&frame(b"tick").unwrap());
        assert_eq!(reader.next_message().unwrap(), Some(b"tick".to_vec()));
    }
    assert_eq!(reader.buffered(), 0);
}

// --- loopback ---------------------------------------------------------

#[test]
fn messages_arrive_at_the_addressed_peer_only() {
    let net = Loopback::with_peers(&[PeerId(0), PeerId(1), PeerId(2)]);
    let mut a = net.endpoint(PeerId(0));
    let mut b = net.endpoint(PeerId(1));
    let mut c = net.endpoint(PeerId(2));

    a.send(PeerId(1), b"for b").unwrap();
    assert_eq!(b.poll(), Some((PeerId(0), b"for b".to_vec())));
    assert_eq!(c.poll(), None);
    assert_eq!(b.poll(), None);
}

#[test]
fn a_peer_lists_everyone_but_itself_in_a_stable_order() {
    let net = Loopback::with_peers(&[PeerId(2), PeerId(0), PeerId(1)]);
    let a = net.endpoint(PeerId(0));
    assert_eq!(a.peers(), vec![PeerId(1), PeerId(2)]);
    assert_eq!(a.peers(), vec![PeerId(1), PeerId(2)]);
}

#[test]
fn broadcast_reaches_everyone_else() {
    let net = Loopback::with_peers(&[PeerId(0), PeerId(1), PeerId(2)]);
    let mut host = net.endpoint(PeerId(0));
    let mut b = net.endpoint(PeerId(1));
    let mut c = net.endpoint(PeerId(2));
    host.broadcast(b"turn").unwrap();
    assert_eq!(b.poll(), Some((PeerId(0), b"turn".to_vec())));
    assert_eq!(c.poll(), Some((PeerId(0), b"turn".to_vec())));
    assert_eq!(host.poll(), None, "a broadcast does not echo to the sender");
}

#[test]
fn sending_to_an_unknown_peer_is_an_error() {
    let net = Loopback::with_peers(&[PeerId(0)]);
    let mut a = net.endpoint(PeerId(0));
    assert_eq!(a.send(PeerId(9), b"x"), Err(TransportError::NoSuchPeer(PeerId(9))));
}

#[test]
fn latency_holds_a_message_for_a_number_of_polls() {
    let net = Loopback::with_peers(&[PeerId(0), PeerId(1)]);
    net.set_latency(PeerId(1), 2);
    let mut a = net.endpoint(PeerId(0));
    let mut b = net.endpoint(PeerId(1));

    a.send(PeerId(1), b"late").unwrap();
    assert_eq!(b.poll(), None);
    assert_eq!(b.poll(), None);
    assert_eq!(b.poll(), Some((PeerId(0), b"late".to_vec())));
}

#[test]
fn reordering_delivers_the_newest_ready_message_first() {
    let net = Loopback::with_peers(&[PeerId(0), PeerId(1)]);
    net.set_reorder(PeerId(1), true);
    let mut a = net.endpoint(PeerId(0));
    let mut b = net.endpoint(PeerId(1));
    a.send(PeerId(1), b"first").unwrap();
    a.send(PeerId(1), b"second").unwrap();
    assert_eq!(b.poll(), Some((PeerId(0), b"second".to_vec())));
    assert_eq!(b.poll(), Some((PeerId(0), b"first".to_vec())));
}

#[test]
fn a_partition_drops_silently_at_the_receiver() {
    let net = Loopback::with_peers(&[PeerId(0), PeerId(1)]);
    let mut a = net.endpoint(PeerId(0));
    let mut b = net.endpoint(PeerId(1));

    net.partition(PeerId(1), true);
    // The sender of a lost packet gets no error on a real network
    // either, which is exactly why §4 needs a packet every tick.
    a.send(PeerId(1), b"lost").unwrap();
    assert_eq!(b.poll(), None);

    net.partition(PeerId(1), false);
    a.send(PeerId(1), b"found").unwrap();
    assert_eq!(b.poll(), Some((PeerId(0), b"found".to_vec())));
}

#[test]
fn pending_counts_what_has_not_been_taken() {
    let net = Loopback::with_peers(&[PeerId(0), PeerId(1)]);
    let mut a = net.endpoint(PeerId(0));
    let mut b = net.endpoint(PeerId(1));
    a.send(PeerId(1), b"one").unwrap();
    a.send(PeerId(1), b"two").unwrap();
    assert_eq!(net.pending(PeerId(1)), 2);
    b.poll();
    assert_eq!(net.pending(PeerId(1)), 1);
}

#[test]
fn an_endpoint_knows_its_own_id() {
    let net = Loopback::new();
    assert_eq!(net.endpoint(PeerId(4)).id(), PeerId(4));
}

/// The loopback is a `Transport`, so anything written against the trait
/// can be tested against it — including code that only holds a `&mut
/// dyn Transport`.
#[test]
fn the_trait_is_object_safe_enough_to_be_swapped() {
    let net = Loopback::with_peers(&[PeerId(0), PeerId(1)]);
    let mut a = net.endpoint(PeerId(0));
    let mut b = net.endpoint(PeerId(1));
    let transport: &mut dyn Transport = &mut a;
    transport.send(PeerId(1), b"through a trait object").unwrap();
    assert_eq!(b.poll().unwrap().1, b"through a trait object".to_vec());
}

#[test]
fn holding_queues_instead_of_dropping_and_releases_in_order() {
    let net = Loopback::with_peers(&[PeerId(0), PeerId(1)]);
    let mut a = net.endpoint(PeerId(0));
    let mut b = net.endpoint(PeerId(1));

    net.hold(PeerId(1), true);
    a.send(PeerId(1), b"one").unwrap();
    a.send(PeerId(1), b"two").unwrap();
    assert_eq!(b.poll(), None, "a stalled link delivers nothing");
    assert_eq!(net.pending(PeerId(1)), 2, "but it has lost nothing either");

    net.hold(PeerId(1), false);
    assert_eq!(b.poll(), Some((PeerId(0), b"one".to_vec())));
    assert_eq!(b.poll(), Some((PeerId(0), b"two".to_vec())));
    assert_eq!(b.poll(), None);
}

/// The distinction the lockstep tests depend on: a held link is a
/// pause, a partitioned one is a loss, and only the first is
/// recoverable. `docs/netcode.md` §7 requires a reliable transport for
/// exactly this reason.
#[test]
fn holding_and_partitioning_are_not_the_same_thing() {
    let net = Loopback::with_peers(&[PeerId(0), PeerId(1)]);
    let mut a = net.endpoint(PeerId(0));
    let mut b = net.endpoint(PeerId(1));

    net.partition(PeerId(1), true);
    a.send(PeerId(1), b"gone").unwrap();
    net.partition(PeerId(1), false);
    assert_eq!(net.pending(PeerId(1)), 0);
    assert_eq!(b.poll(), None, "a dropped message never arrives, however long you wait");
}
