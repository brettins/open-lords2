//! The lobby: host, join, agree, start.
//!
//! Two things are being checked here, and only one of them is the obvious one.
//!
//! The obvious one is that a lobby does lobby things — a host opens a game, a
//! client joins, names and slots and readiness behave.
//!
//! The one that matters is that **the lobby hands `Session::new` the same
//! arguments on every machine**. The seed and the slot list are load-bearing:
//! the seed feeds every `Pcg32` in the simulation, and the slot list's *order*
//! reaches `order_commands` and decides how contested commands are sequenced. A
//! lobby that produced a roster in arrival order would look completely correct
//! on screen and desync on the first tick where two players acted at once. The
//! last test in this file closes that loop by
//! the lobby's output and running it over a real socket.

mod lobby_tests;
pub use lobby_tests::*;

#[path = "../common/mod.rs"]
mod common;

use common::{Order, ToySim};
use l2_net::{
    decode_all, frame, Advance, Canonical, Config, FrameReader, Hello, Lobby, LobbyError,
    LobbyEvent, Message, Mismatch, PeerId, PlayerSlot, Role, Roster, Session, TcpTransport, Tick,
    Transport, PROTOCOL_VERSION,
};

const SEED: u64 = 0x10_2517_9600;

fn hello(slot: u8) -> Hello {
    Hello {
        protocol: PROTOCOL_VERSION,
        quirks: 0,
        engine: "lords2 0.1.0-test".to_string(),
        ruleset_hash: 0xABCD_EF01_2345_6789,
        seed: SEED,
        slot: PlayerSlot::new(slot),
    }
}

/// Drive a host and a set of clients with no transport at all.
///
/// The lobby is transport-free by design, so most of its behaviour can be
/// tested without a socket, a clock or a thread. The socket appears only in the
/// last test, where it is genuinely the thing under test.
struct Table {
    host: Lobby,
    clients: Vec<(PeerId, Lobby)>,
}

impl Table {
    fn new(host_name: &str) -> Table {
        Table { host: Lobby::host(hello(0), host_name).unwrap(), clients: Vec::new() }
    }

    fn join(&mut self, peer: u32, h: Hello, name: &str) -> Result<LobbyEvent, LobbyError> {
        let mut client = Lobby::join(h, name).unwrap();
        client.greet().unwrap();
        let out = client.next_outgoing().expect("a greeting");
        let event = self.host.receive(PeerId(peer), out.message);
        self.clients.push((PeerId(peer), client));
        self.pump();
        event
    }

    /// Deliver everything the host has queued to every client, and back.
    fn pump(&mut self) {
        for _ in 0..8 {
            while let Some(out) = self.host.next_outgoing() {
                for (peer, client) in self.clients.iter_mut() {
                    if out.to.is_none() || out.to == Some(*peer) {
                        // A refusal is an error at the client, which is the
                        // point of it; the tests that care assert on it
                        // directly.
                        let _ = client.receive(PeerId(0), out.message.clone());
                    }
                }
            }
            let mut relayed = Vec::new();
            for (peer, client) in self.clients.iter_mut() {
                while let Some(out) = client.next_outgoing() {
                    relayed.push((*peer, out.message));
                }
            }
            if relayed.is_empty() {
                return;
            }
            for (peer, message) in relayed {
                let _ = self.host.receive(peer, message);
            }
        }
    }

    fn client(&self, i: usize) -> &Lobby {
        &self.clients[i].1
    }
}

