#![allow(unused_imports)]
use super::*;
use super::loopback::*;
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

#[test]
fn a_peer_that_disconnects_mid_session_is_seen_by_the_other() {
    let (mut a, mut b) = connected_pair(0x1234_5678);
    run_to(&mut a, &mut b, Tick(15));
    let agreed = b.hashes.len().min(a.hashes.len());
    assert!(agreed >= 15);

    drop(b);

    let until = deadline();
    while a.departed.is_empty() {
        a.pump(&schedule);
        assert!(Instant::now() < until, "the departure was never noticed");
        breathe();
    }
    assert_eq!(a.departed, vec![PeerId(0)]);
    assert_eq!(a.net.peers(), vec![], "and the list the caller broadcasts to is now empty");

    assert!(!a.session.is_halted());
    match a.session.advance(&mut a.sim) {
        Advance::Waiting { missing, .. } => assert_eq!(missing, vec![PlayerSlot::new(1)]),
        other => panic!("expected to be waiting for the departed peer, got {other:?}"),
    }
    a.session.halt(HaltReason::Left);
    assert!(matches!(a.session.halt_reason(), Some(HaltReason::Left)));
}

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

    let expected = loopback_hashes(seed, Tick(40));
    assert_eq!(a.hashes[..40], expected[0][..40]);
}


