#![allow(unused_imports)]
use super::*;
use super::disconnection::*;
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
    assert!(a.sim.unit(1).expect("unit 1").hp < 100, "peer 0's attacks never arrived");
    assert!(a.sim.unit(2).expect("unit 2").hp < 100, "peer 1's attacks never arrived");
}

pub(super) fn loopback_hashes(seed: u64, target: Tick) -> Vec<Vec<(Tick, u64)>> {
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

