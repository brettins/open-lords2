//! The lobby over TCP, pumped by the app loop between ticks, never inside one
//! (`docs/netcode.md` §7, D-5). Ours: the original's DirectPlay lobby is absent.

use super::{new_game, Setup};
use crate::game::{Assets, Game};
use l2_kingdom::tables::Tables;
use l2_kingdom::Quirks;
use l2_net::{
    decode_all, Canonical, Hello, Lobby, LobbyError, LobbyEvent, Message, PeerId, PlayerSlot,
    Role, Roster, Start, TcpTransport, Transport,
};
use std::net::{SocketAddr, ToSocketAddrs};
use std::time::Duration;

/// Ours; any free port serves.
pub const DEFAULT_PORT: u16 = 4712;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
/// The client's name for the host's connection.
const HOST: PeerId = PeerId(0);

pub struct Driver {
    transport: TcpTransport,
    lobby: Lobby,
    quirks: Quirks,
}

impl Driver {
    /// Listens on `addr`; the host's own seat is ready from the start.
    pub fn host(
        addr: impl ToSocketAddrs,
        hello: Hello,
        name: &str,
        shield: u8,
        setup: &Setup,
        unseated_gold: i32,
        quirks: Quirks,
    ) -> Result<Driver, String> {
        let transport = TcpTransport::listen(addr).map_err(|e| e.to_string())?;
        let lobby = Lobby::host(hello, name).map_err(|e| e.to_string())?;
        let lobby = lobby.with_shield(shield).with_unseated_gold(unseated_gold);
        Ok(Driver { transport, lobby: lobby.with_setup(setup.encode()), quirks })
    }

    /// Connects and sends `Join`. Blocks for at most `CONNECT_TIMEOUT`.
    pub fn join(
        address: &str,
        hello: Hello,
        name: &str,
        shield: u8,
        quirks: Quirks,
    ) -> Result<Driver, String> {
        let to: SocketAddr = address
            .to_socket_addrs()
            .map_err(|e| format!("{address}: {e}"))?
            .next()
            .ok_or_else(|| format!("{address}: no such address"))?;
        let transport =
            TcpTransport::connect_timeout(to, HOST, CONNECT_TIMEOUT).map_err(|e| e.to_string())?;
        let mut lobby = Lobby::join(hello, name).map_err(|e| e.to_string())?.with_shield(shield);
        lobby.greet().map_err(|e| e.to_string())?;
        let mut driver = Driver { transport, lobby, quirks };
        driver.flush()?;
        Ok(driver)
    }

    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.transport.local_addr()
    }

    pub fn lobby(&self) -> &Lobby {
        &self.lobby
    }

    pub fn roster(&self) -> &Roster {
        self.lobby.roster()
    }

    /// This peer's seat: the host may have reseated a client, which finds
    /// itself by its name (names are unique in a roster).
    pub fn me(&self) -> Option<PlayerSlot> {
        match self.lobby.role() {
            Role::Host => Some(self.lobby.local_slot()),
            Role::Client => {
                let name = self.lobby.name();
                self.roster().players.iter().find(|p| p.name == name).map(|p| p.slot)
            }
        }
    }

    pub fn set_ready(&mut self, ready: bool) -> Result<(), String> {
        self.lobby.set_ready(ready).map_err(|e| e.to_string())?;
        self.flush()
    }

    pub fn start(&mut self) -> Result<Start, String> {
        let start = self.lobby.start().map_err(|e| e.to_string())?;
        self.flush()?;
        Ok(start)
    }

    /// One non-blocking pass: read everything waiting, answer it, send.
    /// `Ok(Some(start))` once, on the pass the game starts.
    pub fn pump(&mut self) -> Result<Option<Start>, String> {
        let mut started = None;
        while let Some((peer, bytes)) = self.transport.poll() {
            let Ok(message) = decode_all::<Message>(&bytes) else { continue };
            match self.lobby.receive(peer, message) {
                Ok(LobbyEvent::Started(s)) => started = Some(s),
                Ok(_) => {}
                Err(LobbyError::Incompatible(why)) if self.lobby.role() == Role::Client => {
                    return Err(refusal(&LobbyError::Incompatible(why)));
                }
                // A host refuses a joiner and carries on.
                Err(_) => {}
            }
        }
        for peer in self.transport.take_disconnected() {
            if self.lobby.role() == Role::Client && peer == HOST {
                return Err("the host has left".into());
            }
            self.lobby.peer_left(peer);
        }
        self.flush()?;
        Ok(started)
    }

    /// The world this peer plays, from `Start`, on every peer alike.
    pub fn build(&self, assets: &Assets, start: &Start, tables: Tables) -> Result<Game, String> {
        let me = self.me().ok_or("this peer has no seat")?;
        new_game(assets, start, me, self.quirks, tables).map_err(|e| e.to_string())
    }

    /// For the lockstep session that follows the lobby.
    pub fn into_transport(self) -> TcpTransport {
        self.transport
    }

    fn flush(&mut self) -> Result<(), String> {
        while let Some(out) = self.lobby.next_outgoing() {
            let bytes = Canonical::bytes_of(&out.message);
            let sent = match out.to {
                Some(peer) => self.transport.send(peer, &bytes),
                None => self.transport.broadcast(&bytes),
            };
            if let Err(e) = sent {
                if self.lobby.role() == Role::Client {
                    return Err(e.to_string());
                }
            }
        }
        Ok(())
    }
}

fn refusal(e: &LobbyError) -> String {
    match e {
        LobbyError::Incompatible(why) if why.is_empty() => "the host refused the join".into(),
        e => e.to_string(),
    }
}
