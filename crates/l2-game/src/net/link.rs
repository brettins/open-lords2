//! The lobby screen's mailbox to the app loop's [`Driver`]. Ours. Presentation
//! state: not in the save, not in `encode_state` (`docs/netcode.md` C210).

use super::{Driver, Setup, DEFAULT_PORT};
use crate::game::{Assets, Game};
use l2_kingdom::tables::Tables;
use l2_net::{PlayerSlot, Roster, Start};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    Host { port: u16 },
    Join { address: String },
    Ready(bool),
    Start,
    Leave,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Idle,
    Lobby,
    /// Why the host, the socket or the build said no; the lobby screen shows it.
    Failed(String),
    Started,
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
}

impl Link {
    pub fn default_address() -> String {
        format!("127.0.0.1:{DEFAULT_PORT}")
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
            world.net = Link { phase: Phase::Started, requests: Vec::new(), ..game.net.clone() };
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
            (Request::Host { port }, None, Some(id)) => {
                let settings = id.setup.settings(1, quirks);
                // `[I]` the host's seed, drawn as `new_game_seed` draws one.
                let seed = crate::scenario::new_game_seed(wall_clock, None);
                let hello = super::hello(&settings, &tables, seed, PlayerSlot::new(0));
                let gold = game.chosen_gold;
                Driver::host(("0.0.0.0", port), hello, &id.name, id.shield, &id.setup, gold, quirks)
                    .map(|d| *driver = Some(d))
            }
            (Request::Join { address }, None, Some(id)) => {
                let settings = id.setup.settings(1, quirks);
                // `[I]` a joiner asks for slot 0 and the host reseats it (`on_join`).
                let hello = super::hello(&settings, &tables, 0, PlayerSlot::new(0));
                Driver::join(&address, hello, &id.name, id.shield, quirks).map(|d| *driver = Some(d))
            }
            (Request::Ready(r), Some(d), _) => d.set_ready(r),
            (Request::Start, Some(d), _) => d.start().map(|_| ()),
            (Request::Leave, _, _) => {
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
    if link.phase == Phase::Started {
        return None;
    }
    link.phase = Phase::Lobby;
    link.roster = d.roster().clone();
    link.me = d.me();
    link.hosting = d.lobby().role() == l2_net::Role::Host;
    // The host's own `start` is not echoed back to it.
    started.or_else(|| d.lobby().started().cloned().filter(|_| link.hosting))
}
