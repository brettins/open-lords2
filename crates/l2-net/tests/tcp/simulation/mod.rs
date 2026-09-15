#![allow(unused_imports)]

mod loopback;
pub use loopback::*;
mod disconnection;
pub use disconnection::*;

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


struct NetPeer {
    slot: PlayerSlot,
    session: Session,
    sim: ToySim,
    net: TcpTransport,
    outbox: Vec<Vec<u8>>,
    hashes: Vec<(Tick, u64)>,
    errors: Vec<SessionError>,
    departed: Vec<PeerId>,
    issued_through: Option<Tick>,
}

impl NetPeer {
    pub(super) fn new(slot: PlayerSlot, slots: &[PlayerSlot], seed: u64, net: TcpTransport) -> NetPeer {
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
        let at = self.session.execution_tick();
        if self.issued_through.is_none_or(|last| at > last) {
            self.issued_through = Some(at);
            if let Some(order) = schedule(self.slot.index(), at) {
                self.session.issue(order.payload());
            }
        }

        while let Some(packet) = self.session.next_packet() {
            self.outbox.push(Canonical::bytes_of(&Message::Tick(packet)));
        }

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

        while let Advance::Stepped { tick, hash } = self.session.advance(&mut self.sim) {
            self.hashes.push((tick, hash));
        }
    }
}

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

