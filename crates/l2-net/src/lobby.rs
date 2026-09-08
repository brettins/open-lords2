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
//! simulation bug rather than a lobby bug. Sorting here is D-4 applied to the
//! one place it is easiest to forget.
//!
//! # Compatibility is checked before a slot is granted
//!
//! An incompatible peer never appears in a roster at all. [`Hello::check`]
//! compares protocol, engine build, ruleset hash and seed; any mismatch is
//! refused at the join, so the roster only ever holds peers that could actually
//! play. Refusing late — after other players have seen a name appear — is a
//! worse experience and a larger surface.

use crate::canonical::{Canonical, CodecError, Decode, Encode, Reader};
use crate::command::{PlayerSlot, MAX_PLAYERS};
use crate::packet::{Hello, Mismatch};
use crate::transport::PeerId;

/// The longest a player name may be, in bytes.
///
/// Bytes rather than characters, for the same reason [`Canonical::str`] counts
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
/// every machine and there is no reason to let them arrive separately.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Start {
    pub seed: u64,
    pub roster: Roster,
}

/// Why a lobby refused something.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LobbyError {
    /// The peer is not compatible. Never partially: all reasons at once, so a
    /// player fixing their setup sees the whole list rather than one item per
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
    /// Host only: which peer owns which slot. A `Vec` rather than a map,
    /// because D-4 bans iterating a map and this is small enough that a scan
    /// is cheaper than the temptation.
    seats: Vec<(PeerId, PlayerSlot)>,
    outbox: Vec<Outgoing>,
    started: Option<Start>,
}

impl Lobby {
    /// Open a game. The host always takes the slot in its own [`Hello`].
    pub fn host(identity: Hello, name: impl Into<String>) -> Result<Lobby, LobbyError> {
        let name = check_name(name.into(), &Roster::default())?;
        let mut roster = Roster::default();
        roster.insert_sorted(Player {
            slot: identity.slot,
            name: name.clone(),
            ready: true,
            is_host: true,
        });
        Ok(Lobby {
            role: Role::Host,
            identity,
            name,
            roster,
            seats: Vec::new(),
            outbox: Vec::new(),
            started: None,
        })
    }

    /// Prepare to join one. Nothing is sent until [`Lobby::greet`].
    pub fn join(identity: Hello, name: impl Into<String>) -> Result<Lobby, LobbyError> {
        let name = check_name(name.into(), &Roster::default())?;
        Ok(Lobby {
            role: Role::Client,
            identity,
            name,
            roster: Roster::default(),
            seats: Vec::new(),
            outbox: Vec::new(),
            started: None,
        })
    }

    pub fn role(&self) -> Role {
        self.role
    }

    pub fn roster(&self) -> &Roster {
        &self.roster
    }

    pub fn local_slot(&self) -> PlayerSlot {
        self.identity.slot
    }

    pub fn started(&self) -> Option<&Start> {
        self.started.as_ref()
    }

    /// Queue our introduction. Clients only — a host introduces itself by
    /// existing.
    pub fn greet(&mut self) -> Result<(), LobbyError> {
        if self.role != Role::Client {
            return Err(LobbyError::WrongRole);
        }
        self.outbox.push(Outgoing {
            to: None,
            message: crate::packet::Message::Join(Join {
                hello: self.identity.clone(),
                name: self.name.clone(),
            }),
        });
        Ok(())
    }

    /// Take the next message to send, or `None`.
    pub fn next_outgoing(&mut self) -> Option<Outgoing> {
        if self.outbox.is_empty() {
            None
        } else {
            Some(self.outbox.remove(0))
        }
    }

    /// Say whether we are ready. The host applies it locally; a client asks.
    pub fn set_ready(&mut self, ready: bool) -> Result<(), LobbyError> {
        if self.started.is_some() {
            return Err(LobbyError::AlreadyStarted);
        }
        match self.role {
            Role::Host => {
                if let Some(p) = self
                    .roster
                    .players
                    .iter_mut()
                    .find(|p| p.slot == self.identity.slot)
                {
                    p.ready = ready;
                }
                self.broadcast_roster();
            }
            Role::Client => self.outbox.push(Outgoing {
                to: None,
                message: crate::packet::Message::Ready(ready),
            }),
        }
        Ok(())
    }

    /// Host only. Begin, if everyone is ready.
    pub fn start(&mut self) -> Result<Start, LobbyError> {
        if self.role != Role::Host {
            return Err(LobbyError::WrongRole);
        }
        if self.started.is_some() {
            return Err(LobbyError::AlreadyStarted);
        }
        if !self.roster.all_ready() {
            return Err(LobbyError::WrongRole);
        }
        let start = Start { seed: self.identity.seed, roster: self.roster.clone() };
        self.started = Some(start.clone());
        self.outbox.push(Outgoing {
            to: None,
            message: crate::packet::Message::Start(start.clone()),
        });
        Ok(start)
    }

    /// A peer's connection dropped.
    pub fn peer_left(&mut self, peer: PeerId) -> LobbyEvent {
        if let Some(i) = self.seats.iter().position(|(p, _)| *p == peer) {
            let (_, slot) = self.seats.remove(i);
            self.roster.players.retain(|p| p.slot != slot);
            self.broadcast_roster();
            return LobbyEvent::Left(slot);
        }
        LobbyEvent::Nothing
    }

    /// Feed in a message that arrived from `peer`.
    pub fn receive(
        &mut self,
        peer: PeerId,
        message: crate::packet::Message,
    ) -> Result<LobbyEvent, LobbyError> {
        use crate::packet::Message;
        match (self.role, message) {
            (Role::Host, Message::Join(join)) => self.on_join(peer, join),
            (Role::Host, Message::Ready(ready)) => self.on_ready(peer, ready),

            (Role::Client, Message::Roster(roster)) => {
                self.roster = roster;
                Ok(LobbyEvent::RosterReplaced)
            }
            (Role::Client, Message::Start(start)) => {
                self.roster = start.roster.clone();
                self.started = Some(start.clone());
                Ok(LobbyEvent::Started(start))
            }
            // A refusal is a message too: the host tells us why rather than
            // dropping the connection and leaving us to guess.
            (Role::Client, Message::Refused(reasons)) => Err(LobbyError::Incompatible(reasons)),

            _ => Err(LobbyError::WrongRole),
        }
    }

    fn on_join(&mut self, peer: PeerId, join: Join) -> Result<LobbyEvent, LobbyError> {
        if self.started.is_some() {
            return Err(LobbyError::AlreadyStarted);
        }

        // Compatibility first, before any state changes, so a refused peer
        // leaves no trace in the roster.
        //
        // `Mismatch::SameSlot` is deliberately dropped here, and it is the one
        // thing in this function worth arguing about. `Hello::check` is written
        // for a direct two-peer handshake, where nobody has the authority to
        // move anyone, so a shared slot claim is fatal. A lobby *does* have that
        // authority — it reseats the joiner below. Refusing instead would also
        // be inconsistent: two clients both claiming slot 2 are already reseated
        // without complaint, so refusing only the one that happened to collide
        // with the host would be an accident of who opened the game.
        let mismatches: Vec<Mismatch> = self
            .identity
            .check(&join.hello)
            .into_iter()
            .filter(|m| !matches!(m, Mismatch::SameSlot(_)))
            .collect();
        if !mismatches.is_empty() {
            self.outbox.push(Outgoing {
                to: Some(peer),
                message: crate::packet::Message::Refused(mismatches.clone()),
            });
            return Err(LobbyError::Incompatible(mismatches));
        }

        let name = match check_name(join.name, &self.roster) {
            Ok(n) => n,
            Err(e) => {
                self.outbox.push(Outgoing {
                    to: Some(peer),
                    message: crate::packet::Message::Refused(Vec::new()),
                });
                return Err(e);
            }
        };

        // The claimed slot is honoured when free, otherwise the lowest free one
        // is assigned. Honouring a claim keeps "player 2 reconnects into slot 2"
        // working; falling back keeps two simultaneous joiners from colliding.
        let slot = if self.roster.get(join.hello.slot).is_none() {
            join.hello.slot
        } else {
            match (0..MAX_PLAYERS as u8)
                .map(PlayerSlot::new)
                .find(|s| self.roster.get(*s).is_none())
            {
                Some(s) => s,
                None => {
                    self.outbox.push(Outgoing {
                        to: Some(peer),
                        message: crate::packet::Message::Refused(Vec::new()),
                    });
                    return Err(LobbyError::Full);
                }
            }
        };

        self.roster.insert_sorted(Player { slot, name, ready: false, is_host: false });
        self.seats.push((peer, slot));
        self.broadcast_roster();
        Ok(LobbyEvent::Joined(slot))
    }

    fn on_ready(&mut self, peer: PeerId, ready: bool) -> Result<LobbyEvent, LobbyError> {
        let slot = self
            .seats
            .iter()
            .find(|(p, _)| *p == peer)
            .map(|(_, s)| *s)
            .ok_or(LobbyError::Unknown(peer))?;
        if let Some(p) = self.roster.players.iter_mut().find(|p| p.slot == slot) {
            p.ready = ready;
        }
        self.broadcast_roster();
        Ok(LobbyEvent::ReadyChanged(slot))
    }

    fn broadcast_roster(&mut self) {
        if self.role == Role::Host {
            self.outbox.push(Outgoing {
                to: None,
                message: crate::packet::Message::Roster(self.roster.clone()),
            });
        }
    }
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

/// A client's request to sit down.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Join {
    pub hello: Hello,
    pub name: String,
}

impl Encode for Join {
    fn encode(&self, out: &mut Canonical) {
        self.hello.encode(out);
        out.str(&self.name);
    }
}

impl Decode for Join {
    fn decode(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Join { hello: Hello::decode(input)?, name: input.str()?.to_string() })
    }
}

impl Encode for Player {
    fn encode(&self, out: &mut Canonical) {
        out.u8(self.slot.index());
        out.str(&self.name);
        out.bool(self.ready);
        out.bool(self.is_host);
    }
}

impl Decode for Player {
    fn decode(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        let at = input.position();
        let slot = PlayerSlot::from_wire(input.u8()?)
            .ok_or(CodecError::BadTag { tag: 0, expected: "player slot", at })?;
        Ok(Player {
            slot,
            name: input.str()?.to_string(),
            ready: input.bool()?,
            is_host: input.bool()?,
        })
    }
}

impl Encode for Roster {
    fn encode(&self, out: &mut Canonical) {
        out.seq(&self.players, |c, p| p.encode(c));
    }
}

impl Decode for Roster {
    fn decode(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        let players = input.seq(Player::decode)?;
        // The sort order is part of the contract, so a peer that claims
        // otherwise is refused rather than trusted. A roster out of order would
        // hand Session::new a different slot list on one machine.
        if players.windows(2).any(|w| w[0].slot.index() >= w[1].slot.index()) {
            return Err(CodecError::BadTag {
                tag: 0,
                expected: "roster sorted by slot, without duplicates",
                at: 0,
            });
        }
        Ok(Roster { players })
    }
}

impl Encode for Start {
    fn encode(&self, out: &mut Canonical) {
        out.u64(self.seed);
        self.roster.encode(out);
    }
}

impl Decode for Start {
    fn decode(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Start { seed: input.u64()?, roster: Roster::decode(input)? })
    }
}
