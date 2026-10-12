//! The lobby screen's mailbox to the app loop's [`Driver`]. Ours. Presentation
//! state: not in the save, not in `encode_state` (`docs/netcode.md` C210).

use super::{Driver, Setup, DEFAULT_PORT};
use crate::game::{Assets, Game};
use l2_kingdom::tables::Tables;
use l2_net::{PlayerSlot, Roster, Start};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// The app binds every interface; a test binds 127.0.0.1 (`net_game` `loopback`).
    Host { addr: std::net::SocketAddr },
    Join { address: String },
    Ready(bool),
    Start,
    Leave,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AfterHalt {
    /// The computer keeps the absent realms; the game goes on single-player (C240).
    Alone,
    Leave,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Idle,
    Lobby,
    /// A join's connect is running off the UI thread, to this address.
    Connecting(String),
    /// Why the host, the socket or the build said no; the lobby screen shows it.
    Failed(String),
    Started,
}

/// The host's port on its router (C252): asked of NAT-PMP, then UPnP, off the loop.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PortMap {
    /// Not hosting, or hosting on loopback.
    #[default]
    Off,
    Asking,
    Open { public: std::net::SocketAddr, via: &'static str, lifetime: u32 },
    /// No router opened it; why, in the router's words.
    Shut(String),
}

/// Who this seat is, set by the setup screen before the lobby opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub name: String,
    pub shield: u8,
    pub setup: Setup,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Link {
    pub requests: Vec<Request>,
    pub identity: Option<Identity>,
    pub phase: Phase,
    pub roster: Roster,
    pub me: Option<PlayerSlot>,
    pub hosting: bool,
    /// Where the others reach this host, for the lobby screen.
    pub reach: Vec<std::net::SocketAddr>,
    pub port: PortMap,
    /// The host's join code (C254), once the service answered.
    pub code: Option<String>,
    /// Why there is no code, in the player's words (C254).
    pub relay: Option<String>,
    /// `--players n` (C253): a host starts at n ready seats, a joiner readies at once.
    pub auto: Option<u8>,
    /// What a replay rebuilds the world from.
    pub origin: Option<Box<super::replay::Origin>>,
    /// A lockstep session runs this world: `Game::issue` queues into `outbox`.
    pub lockstep: bool,
    pub outbox: Vec<crate::command::GameCommand>,
    /// A peer's packet is overdue past `STALL_GRACE`.
    pub stalled: bool,
    /// Why the session stopped, for the dialog.
    pub halted: Option<String>,
    /// `[D]` the halt box's answer; the app ends the session on it.
    pub after_halt: Option<AfterHalt>,
    /// C256: a passing line, such as a resync; counts down in pumps.
    pub notice: Option<(String, u32)>,
    /// The season `net::frame` finished, for the campaign screen to show.
    pub done: Option<Box<crate::turn::TurnOutcome>>,
    /// `FUN_0041A844`'s heartbeat, per seat.
    pub beat: super::status::Beat,
}

impl Link {
    pub fn default_address() -> String {
        format!("127.0.0.1:{DEFAULT_PORT}")
    }
}

impl Request {
    /// What the lobby screen's *Host* binds: every interface, so the LAN reaches it.
    pub fn every_interface(port: u16) -> std::net::SocketAddr {
        (std::net::Ipv4Addr::UNSPECIFIED, port).into()
    }
}

/// The app loop's half, run between ticks. Returns the new world on `Start`,
/// carrying the link across; the caller swaps it in.
pub fn service(
    driver: &mut Option<Driver>,
    game: &mut Game,
    assets: &Assets,
    tables: Tables,
) -> Option<Game> {
    let start = lobby(driver, game, assets.wall_clock, tables)?;
    let built = driver.as_ref()?.build(assets, &start, tables);
    match built {
        Ok(mut world) => {
            let quirks = game.kingdom.options.quirks;
            let origin = Some(Box::new(super::replay::Origin { start, quirks }));
            world.net = Link { phase: Phase::Started, requests: Vec::new(), origin, ..game.net.clone() };
            world.prefs = game.prefs;
            world.tips = game.tips;
            Some(world)
        }
        Err(why) => {
            *driver = None;
            game.net.phase = Phase::Failed(why);
            None
        }
    }
}

/// The lobby half of [`service`]: requests out, roster and refusals in.
/// `Some(start)` once, on the pass the game starts.
pub fn lobby(
    driver: &mut Option<Driver>,
    game: &mut Game,
    wall_clock: Option<i64>,
    tables: Tables,
) -> Option<Start> {
    let link = &mut game.net;
    let quirks = game.kingdom.options.quirks;
    for request in std::mem::take(&mut link.requests) {
        let result = match (request, driver.as_mut(), link.identity.clone()) {
            (Request::Host { addr }, None, Some(id)) => {
                let settings = id.setup.settings(1, quirks);
                // `[I]` the host's seed, drawn as `new_game_seed` draws one.
                let seed = crate::scenario::new_game_seed(wall_clock, None);
                let hello = super::hello(&settings, &tables, seed, PlayerSlot::new(0));
                let gold = game.chosen_gold;
                Driver::host(addr, hello, &id.name, id.shield, &id.setup, gold, quirks)
                    .map(|d| *driver = Some(d.with_relay(super::relay::configured())))
            }
            (Request::Join { address }, None, Some(id)) => {
                let settings = id.setup.settings(1, quirks);
                // `[I]` a joiner asks for slot 0 and the host reseats it (`on_join`).
                let hello = super::hello(&settings, &tables, 0, PlayerSlot::new(0));
                let joined = Driver::join(&address, hello, &id.name, id.shield, quirks);
                joined.map(|d| {
                    *driver = Some(d);
                    link.phase = Phase::Connecting(address);
                })
            }
            (Request::Ready(r), Some(d), _) => d.set_ready(r),
            (Request::Start, Some(d), _) => d.start().map(|_| ()),
            (Request::Leave, _, _) => {
                if let Some(d) = driver.as_mut() {
                    d.release();
                }
                *driver = None;
                link.phase = Phase::Idle;
                Ok(())
            }
            _ => Ok(()),
        };
        if let Err(why) = result {
            *driver = None;
            link.phase = Phase::Failed(why);
        }
    }
    let d = driver.as_mut()?;
    let started = match d.pump() {
        Ok(s) => s,
        Err(why) => {
            *driver = None;
            link.phase = Phase::Failed(why);
            return None;
        }
    };
    if link.phase == Phase::Started || d.is_connecting() {
        return None;
    }
    link.phase = Phase::Lobby;
    link.roster = d.roster().clone();
    link.me = d.me();
    link.hosting = d.lobby().role() == l2_net::Role::Host;
    link.reach = d.reach().to_vec();
    link.port = d.port().clone();
    link.code = d.code().map(String::from);
    link.relay = d.relay_status().map(String::from);
    if let Some(n) = link.auto {
        let me = link.me.and_then(|s| link.roster.get(s));
        let all_ready = link.roster.players.iter().all(|p| p.is_host || p.ready);
        if link.hosting && link.roster.len() >= n as usize && all_ready {
            link.requests.push(Request::Start);
            link.auto = None;
        } else if !link.hosting && me.is_some_and(|p| !p.ready) {
            link.requests.push(Request::Ready(true));
            link.auto = None;
        }
    }
    // The host's own `start` is not echoed back to it.
    started.or_else(|| d.lobby().started().cloned().filter(|_| link.hosting))
}

/// `[D]` the session is over: this world is single-player from here (C240).
pub fn continue_alone(game: &mut Game) {
    let identity = game.net.identity.take();
    game.net = Link { identity, ..Link::default() };
    game.multiplayer = false;
}
