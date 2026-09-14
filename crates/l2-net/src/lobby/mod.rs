//! Host, join, and agree on what game is about to start.
//!
//! Everything before tick 0. [`Session`](crate::Session) takes a seed and a
//! slot list and assumes both are already agreed; this module is how they
//! become agreed.
//!
//! # The host is authoritative, and that is the whole design
//!
//! Clients never invent a slot, never decide who else is present, and never
//! choose the seed. They say what they are ([`Hello`]) and whether they are
//! ready; everything else is the host's to declare and to broadcast. The
//! alternative — peers negotiating a roster among themselves — needs a
//! consensus algorithm to answer questions a single authority answers for free,
//! and every one of those questions is a chance for two machines to start with
//! different slot lists.
//!
//! # Why the roster is sorted, always
//!
//! [`Roster::slots`] returns players **in slot order**, never arrival order.
//! This is not tidiness. `Session::new` takes the slot list, and
//! [`order_commands`](crate::order_commands) breaks ties by slot, so the list's
//! order reaches the simulation. Two machines that ordered the roster by
//! arrival — or, worse, by iterating a map — would disagree about command
//! order on the very first contested tick, and the desync would look like a
//! simulation bug. Sorting here is D-4 applied to the
//! one place it is easiest to forget.
//!
//! # Compatibility is checked before a slot is granted
//!
//! An incompatible peer never appears in a roster at all. [`Hello::check`]
//! compares protocol, engine build, ruleset hash and seed; any mismatch is
//! refused at the join, so the roster only ever holds peers that could
//! play. Refusing late — after other players have seen a name appear — is a
//! worse experience and a larger surface.

mod lobby;
pub use lobby::*;
mod codec;
pub use codec::*;

use crate::canonical::{Canonical, CodecError, Decode, Encode, Reader};
use crate::command::{PlayerSlot, MAX_PLAYERS};
use crate::packet::{Hello, Mismatch};
use crate::transport::PeerId;

/// The longest a player name may be, in bytes.
///
/// Bytes, for the same reason [`Canonical::str`] counts
/// bytes: it is the only definition two implementations cannot disagree about.
pub const MAX_NAME: usize = 32;

/// One seat in the lobby.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Player {
    pub slot: PlayerSlot,
    pub name: String,
    pub ready: bool,
    /// True for exactly one player: the one who opened the game.
    pub is_host: bool,
}

/// Who is here, as the host sees it. The host's view is the only one.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Roster {
    /// Always sorted by slot — see the module docs.
    pub players: Vec<Player>,
}

impl Roster {
    /// The slot list to hand [`Session::new`](crate::Session::new).
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

    /// Everyone present is ready, and there is somebody to be ready with.
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

/// The host's declaration that the game is beginning.
///
/// Carries the seed and the roster together because they must be the same on
/// every machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Start {
    pub seed: u64,
    pub roster: Roster,
}

/// Why a lobby refused something.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LobbyError {
    /// The peer is not compatible. Never partially: all reasons at once, so a
    /// player fixing their setup sees the whole list
    /// reconnect.
    Incompatible(Vec<Mismatch>),
    /// Every slot is taken.
    Full,
    /// The slot this peer claimed is already somebody else's.
    SlotTaken(PlayerSlot),
    /// A name that is empty, over [`MAX_NAME`] bytes, or already in use.
    BadName(String),
    /// A message only a host may send, from a client, or vice versa.
    WrongRole,
    /// A message from a peer that never joined.
    Unknown(PeerId),
    /// The game has already begun.
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

/// What changed, for a caller that wants to redraw the lobby screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LobbyEvent {
    Nothing,
    Joined(PlayerSlot),
    Left(PlayerSlot),
    ReadyChanged(PlayerSlot),
    RosterReplaced,
    Started(Start),
}

/// Host or client. A lobby is one or the other for its whole life.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Host,
    Client,
}

/// A message the caller must send, and to whom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outgoing {
    /// `None` means every connected peer.
    pub to: Option<PeerId>,
    pub message: crate::packet::Message,
}

/// The lobby state machine.
///
/// Transport-free, like the rest of this crate: it consumes messages and
/// produces messages, and the caller owns the socket. That is what lets the
/// tests run a four-player lobby with no network at all, and the same code then
/// run over [`TcpTransport`](crate::TcpTransport).
#[derive(Debug, Clone)]
pub struct Lobby {
    role: Role,
    identity: Hello,
    name: String,
    roster: Roster,
    /// Host only: which peer owns which slot. A `Vec`,
    /// because D-4 bans iterating a map and this is small enough that a scan
    /// is cheaper than the temptation.
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

