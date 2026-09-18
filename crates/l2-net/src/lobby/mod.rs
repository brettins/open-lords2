
mod lobby;
pub use lobby::*;
mod codec;
pub use codec::*;

use crate::canonical::{Canonical, CodecError, Decode, Encode, Reader};
use crate::command::{PlayerSlot, MAX_PLAYERS};
use crate::packet::{Hello, Mismatch};
use crate::transport::PeerId;

pub const MAX_NAME: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Player {
    pub slot: PlayerSlot,
    pub name: String,
    pub ready: bool,
    pub is_host: bool,
}

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Start {
    pub seed: u64,
    pub roster: Roster,
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

