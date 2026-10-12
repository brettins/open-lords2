//! The lobby over TCP, pumped by the app loop between ticks, never inside one
//! (`docs/netcode.md` §7, D-5). Ours: the original's DirectPlay lobby is absent.

use super::{new_game, Setup};
use crate::game::{Assets, Game};
use l2_kingdom::tables::Tables;
use l2_kingdom::Quirks;
use super::link::PortMap;
use super::relay;
use l2_net::{
    decode_all, Canonical, Hello, Lobby, LobbyError, LobbyEvent, Mapping, Message, NatError,
    NoSeat, PeerId, PlayerSlot, Rejoin, Role, Roster, Start, TcpTransport, Transport,
};
use super::InstallId;
use std::io::ErrorKind;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs};
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
    /// The router's answer, on its way (C252); a loopback host never asks.
    mapping: Option<Receiver<Result<Mapping, NatError>>>,
    mapped: Option<Mapping>,
    port: PortMap,
    /// The join-code service's control connection (C254); a client has none.
    relay: Option<relay::Host>,
    /// C260: each seat's install, as its `Join` said; the host's own first.
    installs: Vec<(PlayerSlot, InstallId)>,
    /// C260: the host took this client back into a game under way.
    rejoined: Option<Box<Rejoin>>,
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
        let bound = transport.local_addr();
        let reach = bound.map_or_else(Vec::new, reach_of);
        let lobby = Lobby::host(hello, name).map_err(|e| e.to_string())?;
        let lobby = lobby.with_shield(shield).with_unseated_gold(unseated_gold);
        let lobby = lobby.with_setup(setup.encode());
        let mut d = Driver {
            transport: Some(transport),
            connecting: None,
            lobby,
            quirks,
            reach,
            mapping: None,
            mapped: None,
            port: PortMap::Off,
            relay: None,
            installs: Vec::new(),
            rejoined: None,
        };
        if let Some(bound) = bound.filter(|b| b.ip().is_unspecified()) {
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let _ = tx.send(l2_net::open_port(bound.port()));
            });
            d.mapping = Some(rx);
            d.port = PortMap::Asking;
        }
        Ok(d)
    }

    /// A host asks the join-code service for a code (C254); off the loop.
    pub fn with_relay(mut self, relay: Option<String>) -> Driver {
        if let Some(at) = relay.filter(|_| self.lobby.role() == Role::Host) {
            self.relay = Some(relay::Host::open(at));
        }
        self
    }

    /// Returns at once; the connect (at most `CONNECT_TIMEOUT`) runs on a thread
    /// and [`Driver::pump`] picks it up. A code goes through `relay::configured()`.
    pub fn join(
        address: &str,
        hello: Hello,
        name: &str,
        shield: u8,
        quirks: Quirks,
    ) -> Result<Driver, String> {
        Driver::join_via(address, relay::configured().as_deref(), hello, name, shield, quirks)
    }

    /// `address` as six symbols of the code alphabet is a join code (C254):
    /// `JOIN 1 <code>` at `relay`, else the address path.
    pub fn join_via(
        address: &str,
        relay: Option<&str>,
        hello: Hello,
        name: &str,
        shield: u8,
        quirks: Quirks,
    ) -> Result<Driver, String> {
        let code = l2_relay::as_code(address);
        let relay = match (&code, relay) {
            (Some(_), None) => return Err(relay::NOT_CONFIGURED.into()),
            (_, relay) => relay.map(String::from),
        };
        let lobby = Lobby::join(hello, name).map_err(|e| e.to_string())?.with_shield(shield);
        let (tx, rx) = mpsc::channel();
        let address = address.trim().to_string();
        std::thread::spawn(move || {
            let _ = tx.send(match (code, relay) {
                (Some(code), Some(at)) => relay::join(&at, &code)
                    .and_then(|s| TcpTransport::adopt(s, HOST).map_err(|e| e.to_string())),
                _ => connect(&address),
            });
        });
        Ok(Driver {
            transport: None,
            connecting: Some(rx),
            lobby,
            quirks,
            reach: Vec::new(),
            mapping: None,
            mapped: None,
            port: PortMap::Off,
            relay: None,
            installs: Vec::new(),
            rejoined: None,
        })
    }

    /// C260: this install's id; a client's goes out in its `Join` once connected.
    pub fn with_install(mut self, install: InstallId) -> Driver {
        self.lobby = self.lobby.with_install(install.0);
        if self.lobby.role() == Role::Host {
            self.installs = vec![(self.lobby.local_slot(), install)];
        }
        self
    }

    pub fn installs(&self) -> &[(PlayerSlot, InstallId)] {
        &self.installs
    }

    /// C260: the host's `Rejoin`, once.
    pub fn take_rejoin(&mut self) -> Option<Box<Rejoin>> {
        self.rejoined.take()
    }

    /// The host's join code, once the service answered (C254).
    pub fn code(&self) -> Option<&str> {
        self.relay.as_ref()?.code()
    }

    /// Why there is no code, in the player's words (C254).
    pub fn relay_status(&self) -> Option<&str> {
        self.relay.as_ref()?.status()
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

    /// The router's port mapping, for the lobby screen.
    pub fn port(&self) -> &PortMap {
        &self.port
    }

    /// Closes the router's port; after `Start` the lease is left to lapse
    /// (`l2_net::nat::LEASE`), the session taking no one new.
    pub fn release(&mut self) {
        self.mapping = None;
        if let Some(mapping) = self.mapped.take() {
            std::thread::spawn(move || {
                let _ = l2_net::close_port(&mapping);
            });
        }
        self.port = PortMap::Off;
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
                    self.lobby.greet().map_err(|e| e.to_string())?;
                }
            }
        }
        if let Some(rx) = &self.mapping {
            match rx.try_recv() {
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    self.mapping = None;
                    self.port = PortMap::Shut("the port mapping attempt failed".into());
                }
                Ok(Err(why)) => {
                    self.mapping = None;
                    self.port = PortMap::Shut(why.to_string());
                }
                Ok(Ok(mapping)) => {
                    self.mapping = None;
                    self.port = PortMap::Open {
                        public: SocketAddr::V4(mapping.external),
                        via: mapping.via.name(),
                        lifetime: mapping.lifetime,
                    };
                    self.mapped = Some(mapping);
                }
            }
        }
        let mut started = None;
        let Some(transport) = self.transport.as_mut() else { return Ok(None) };
        if let Some(relay) = self.relay.as_mut() {
            for stream in relay.pump() {
                // A stream the relay paired: `add` numbers it like an accepted one (C254).
                let _ = transport.add(stream);
            }
        }
        while let Some((peer, bytes)) = transport.poll() {
            let Ok(message) = decode_all::<Message>(&bytes) else { continue };
            let install = match &message {
                Message::Join(j) => InstallId(j.install),
                // C260: the tick packets after it are the session's; they stay queued.
                Message::Rejoin(r) if self.lobby.role() == Role::Client => {
                    self.rejoined = Some(r.clone());
                    break;
                }
                Message::NoSeat(why) if self.lobby.role() == Role::Client => return Err(no_seat(*why)),
                _ => InstallId::NIL,
            };
            match self.lobby.receive(peer, message) {
                Ok(LobbyEvent::Started(s)) => started = Some(s),
                Ok(LobbyEvent::Joined(slot)) => self.installs.push((slot, install)),
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
            if let LobbyEvent::Left(slot) = self.lobby.peer_left(peer) {
                self.installs.retain(|&(s, _)| s != slot);
            }
        }
        self.flush()?;
        Ok(started)
    }

    /// The world this peer plays, from `Start`, on every peer alike.
    pub fn build(&self, assets: &Assets, start: &Start, tables: Tables) -> Result<Game, String> {
        let me = self.me().ok_or("this peer has no seat")?;
        new_game(assets, start, me, self.quirks, tables).map_err(|e| e.to_string())
    }

    /// For the lockstep session that follows the lobby. C260: the host keeps its
    /// port and code, and `Live` takes back a dropped player or refuses the join.
    pub fn into_session(self) -> Option<(TcpTransport, Option<relay::Host>)> {
        Some((self.transport?, self.relay))
    }

    pub fn into_transport(self) -> Option<TcpTransport> {
        self.into_session().map(|(t, _)| t)
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
    match l2_net::nat::lan_ip() {
        Some(ip) => vec![SocketAddr::new(IpAddr::V4(ip), bound.port())],
        None => vec![SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), bound.port())],
    }
}

/// C260: `[D]` ours, in the joining player's words.
fn no_seat(why: NoSeat) -> String {
    match why {
        NoSeat::Retired => "That game went on without you: the computer plays your realm now.".into(),
        NoSeat::Started => "That game has already started without you.".into(),
    }
}

fn refusal(e: &LobbyError) -> String {
    match e {
        LobbyError::Incompatible(why) if why.is_empty() => "The host refused the join.".into(),
        e => e.to_string(),
    }
}
