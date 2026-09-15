#![allow(unused_imports)]
use super::*;
use super::connection::*;
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

#[test]
fn a_message_crosses_a_real_socket_intact() {
    let (mut host, addr) = host();
    let mut client = TcpTransport::connect(addr, PeerId(7)).expect("connecting");

    client.send(PeerId(7), b"move 7 to 34,12").expect("sending");
    assert_eq!(wait_for_message(&mut host), (PeerId(0), b"move 7 to 34,12".to_vec()));

    host.send(PeerId(0), b"acknowledged").expect("sending");
    assert_eq!(wait_for_message(&mut client), (PeerId(7), b"acknowledged".to_vec()));
}

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
    host.send(PeerId(0), b"still here").expect("sending");
}

#[test]
fn a_large_message_survives_a_partial_write() {
    let (mut host, addr) = host();
    let mut client = TcpTransport::connect(addr, PeerId(0)).expect("connecting");

    let payload: Vec<u8> = (0..900_000u32).map(|i| (i % 251) as u8).collect();
    client.send(PeerId(0), &payload).expect("sending");

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

