
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

struct Table {
    host: Lobby,
    clients: Vec<(PeerId, Lobby)>,
}

impl Table {
    pub(crate) fn new(host_name: &str) -> Table {
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

    fn pump(&mut self) {
        for _ in 0..8 {
            while let Some(out) = self.host.next_outgoing() {
                for (peer, client) in self.clients.iter_mut() {
                    if out.to.is_none() || out.to == Some(*peer) {
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

