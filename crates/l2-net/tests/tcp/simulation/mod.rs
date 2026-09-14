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

