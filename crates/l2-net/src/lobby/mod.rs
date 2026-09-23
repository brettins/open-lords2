
mod lobby;
mod codec;
pub use codec::*;

use crate::command::PlayerSlot;
use crate::packet::{Hello, Mismatch};
use crate::transport::PeerId;

pub const MAX_NAME: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Player {
    pub slot: PlayerSlot,
    pub name: String,
    pub ready: bool,
    pub is_host: bool,
    /// The colour claimed on setup page 4, 1..=5; the host arbitrates it as
    /// `FUN_00432FAB` (`0x00432FAB`) does its claim table. `docs/netcode.md` D-3b.
    pub shield: u8,
}

pub const SHIELDS: u8 = 5;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Roster {
    pub players: Vec<Player>,
}

impl Roster {
    pub fn slots(&self) -> Vec<PlayerSlot> {
        self.players.iter().map(|p| p.slot).collect()
    }

    pub fn get(&self, slot: PlayerSlot) -> Option<&Player> {
        self.players.iter().find(|p| p.slot == slot)
    }

    pub fn len(&self) -> usize {
        self.players.len()
    }

    pub fn is_empty(&self) -> bool {
        self.players.is_empty()
    }

    pub fn all_ready(&self) -> bool {
        self.players.len() >= 2 && self.players.iter().all(|p| p.ready)
    }

    fn insert_sorted(&mut self, player: Player) {
        let at = self
            .players
            .partition_point(|p| p.slot.index() < player.slot.index());
        self.players.insert(at, player);
    }

    /// `FUN_00432FAB` ignores a click on a colour somebody holds; a seat that
    /// asked for a held or invalid one gets the lowest free. `[I]` the fallback.
    fn free_shield(&self, wanted: u8) -> u8 {
        let held = |s: u8| self.players.iter().any(|p| p.shield == s);
        if (1..=SHIELDS).contains(&wanted) && !held(wanted) {
            return wanted;
        }
        (1..=SHIELDS).find(|&s| !held(s)).unwrap_or(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Start {
    pub seed: u64,
    pub roster: Roster,
    /// The host's `g_startingGoldChosen` from its last game: an unseated realm
    /// keeps it (`FUN_0049C995`, `0x0049C995`), and it is process state.
    pub unseated_gold: i32,
    /// The host's map slot and options, opaque here; `l2_game::net::Setup` reads it.
    pub setup: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LobbyError {
    Incompatible(Vec<Mismatch>),
    Full,
    SlotTaken(PlayerSlot),
    BadName(String),
    WrongRole,
    Unknown(PeerId),
    AlreadyStarted,
}

impl core::fmt::Display for LobbyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LobbyError::Incompatible(ms) => {
                write!(f, "cannot play together: ")?;
                for (i, m) in ms.iter().enumerate() {
                    if i > 0 {
                        write!(f, "; ")?;
                    }
                    write!(f, "{m}")?;
                }
                Ok(())
            }
            LobbyError::Full => write!(f, "the game is full"),
            LobbyError::SlotTaken(s) => write!(f, "{s} is already taken"),
            LobbyError::BadName(n) => write!(f, "unusable player name '{n}'"),
            LobbyError::WrongRole => write!(f, "that message is not the sender's to send"),
            LobbyError::Unknown(p) => write!(f, "no player joined from {p:?}"),
            LobbyError::AlreadyStarted => write!(f, "the game has already started"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LobbyEvent {
    Nothing,
    Joined(PlayerSlot),
    Left(PlayerSlot),
    ReadyChanged(PlayerSlot),
    RosterReplaced,
    Started(Start),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Host,
    Client,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outgoing {
    pub to: Option<PeerId>,
    pub message: crate::packet::Message,
}

#[derive(Debug, Clone)]
pub struct Lobby {
    role: Role,
    identity: Hello,
    name: String,
    roster: Roster,
    seats: Vec<(PeerId, PlayerSlot)>,
    outbox: Vec<Outgoing>,
    started: Option<Start>,
    shield: u8,
    unseated_gold: i32,
    setup: Vec<u8>,
}

fn check_name(name: String, roster: &Roster) -> Result<String, LobbyError> {
    if name.is_empty() || name.len() > MAX_NAME {
        return Err(LobbyError::BadName(name));
    }
    if roster.players.iter().any(|p| p.name == name) {
        return Err(LobbyError::BadName(name));
    }
    Ok(name)
}

