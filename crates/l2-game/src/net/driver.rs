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
use std::io::ErrorKind;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;

/// Ours; any free port serves.
pub const DEFAULT_PORT: u16 = 4712;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
/// The client's name for the host's connection.
const HOST: PeerId = PeerId(0);

pub struct Driver {
    /// `None` while a join's connect runs on its own thread (net-join-blocks).
    transport: Option<TcpTransport>,
    connecting: Option<Receiver<Result<TcpTransport, String>>>,
    lobby: Lobby,
    quirks: Quirks,
    /// Where the others reach this host; empty for a client.
    reach: Vec<SocketAddr>,
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
        let reach = transport.local_addr().map_or_else(Vec::new, reach_of);
        let lobby = Lobby::host(hello, name).map_err(|e| e.to_string())?;
        let lobby = lobby.with_shield(shield).with_unseated_gold(unseated_gold);
        let lobby = lobby.with_setup(setup.encode());
        Ok(Driver { transport: Some(transport), connecting: None, lobby, quirks, reach })
    }

    /// Returns at once; the connect (at most `CONNECT_TIMEOUT`) runs on a thread
    /// and [`Driver::pump`] picks it up.
    pub fn join(
        address: &str,
        hello: Hello,
        name: &str,
        shield: u8,
        quirks: Quirks,
    ) -> Result<Driver, String> {
        let mut lobby = Lobby::join(hello, name).map_err(|e| e.to_string())?.with_shield(shield);
        lobby.greet().map_err(|e| e.to_string())?;
        let (tx, rx) = mpsc::channel();
        let address = address.trim().to_string();
        std::thread::spawn(move || {
            let _ = tx.send(connect(&address));
        });
        Ok(Driver { transport: None, connecting: Some(rx), lobby, quirks, reach: Vec::new() })
    }

    pub fn is_connecting(&self) -> bool {
        self.connecting.is_some()
    }

    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.transport.as_ref()?.local_addr()
    }

    /// The host's addresses to give the others: the LAN's when bound to all.
    pub fn reach(&self) -> &[SocketAddr] {
        &self.reach
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
        if let Some(rx) = &self.connecting {
            match rx.try_recv() {
                Err(TryRecvError::Empty) => return Ok(None),
                Err(TryRecvError::Disconnected) => return Err("the connection attempt failed".into()),
                Ok(result) => {
                    self.connecting = None;
                    self.transport = Some(result?);
                }
            }
        }
        let mut started = None;
        let Some(transport) = self.transport.as_mut() else { return Ok(None) };
        while let Some((peer, bytes)) = transport.poll() {
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
        for peer in transport.take_disconnected() {
            if self.lobby.role() == Role::Client && peer == HOST {
                return Err("The host has left the game.".into());
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

    /// For the lockstep session that follows the lobby; after `Start` the host
    /// takes no one new (net-live-leftovers: a stranger was accepted).
    pub fn into_transport(self) -> Option<TcpTransport> {
        let mut t = self.transport?;
        t.stop_listening();
        Some(t)
    }

    fn flush(&mut self) -> Result<(), String> {
        let Some(transport) = self.transport.as_mut() else { return Ok(()) };
        while let Some(out) = self.lobby.next_outgoing() {
            let bytes = Canonical::bytes_of(&out.message);
            let sent = match out.to {
                Some(peer) => transport.send(peer, &bytes),
                None => transport.broadcast(&bytes),
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

/// The join's thread: resolve, connect, and say why not in the player's terms.
fn connect(address: &str) -> Result<TcpTransport, String> {
    let bad = || format!("\"{address}\" is not an address. Type the host's, as 192.168.1.20:{DEFAULT_PORT}.");
    let to = address.to_socket_addrs().map_err(|_| bad())?.next().ok_or_else(bad)?;
    let stream = TcpStream::connect_timeout(&to, CONNECT_TIMEOUT).map_err(|e| match e.kind() {
        ErrorKind::TimedOut | ErrorKind::WouldBlock => format!(
            "No answer from {to} in {} seconds. Check the address, and that the host's firewall lets the game in.",
            CONNECT_TIMEOUT.as_secs()
        ),
        ErrorKind::ConnectionRefused => {
            format!("{to} refused the connection: no game is hosted there. Check the port.")
        }
        _ => format!("Could not reach {to}: {e}."),
    })?;
    TcpTransport::adopt(stream, HOST).map_err(|e| e.to_string())
}

/// A bound address as the others type it: all interfaces become the LAN's.
fn reach_of(bound: SocketAddr) -> Vec<SocketAddr> {
    if !bound.ip().is_unspecified() {
        return vec![bound];
    }
    // The route to TEST-NET-1 names the LAN interface; UDP connect sends nothing.
    let lan = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
        .and_then(|s| s.connect((Ipv4Addr::new(192, 0, 2, 1), 9)).map(|_| s))
        .and_then(|s| s.local_addr());
    match lan {
        Ok(a) if !a.ip().is_unspecified() => vec![SocketAddr::new(a.ip(), bound.port())],
        _ => vec![SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), bound.port())],
    }
}

fn refusal(e: &LobbyError) -> String {
    match e {
        LobbyError::Incompatible(why) if why.is_empty() => "The host refused the join.".into(),
        e => e.to_string(),
    }
}
