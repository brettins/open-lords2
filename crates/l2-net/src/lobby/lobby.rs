#![allow(unused_imports)]
use super::*;
use super::codec::*;
use crate::canonical::{Canonical, CodecError, Decode, Encode, Reader};
use crate::command::{PlayerSlot, MAX_PLAYERS};
use crate::packet::{Hello, Mismatch};
use crate::transport::PeerId;

impl Lobby {
    pub fn host(identity: Hello, name: impl Into<String>) -> Result<Lobby, LobbyError> {
        let name = check_name(name.into(), &Roster::default())?;
        let mut roster = Roster::default();
        roster.insert_sorted(Player {
            slot: identity.slot,
            name: name.clone(),
            ready: true,
            is_host: true,
            shield: 1,
            realm: 0,
        });
        Ok(Lobby {
            role: Role::Host,
            identity,
            name,
            roster,
            seats: Vec::new(),
            outbox: Vec::new(),
            started: None,
            shield: 1,
            unseated_gold: 0,
            setup: Vec::new(),
            install: [0; 16],
            save: Vec::new(),
        })
    }

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
            shield: 0,
            unseated_gold: 0,
            setup: Vec::new(),
            install: [0; 16],
            save: Vec::new(),
        })
    }

    /// The shield this seat asks for; a client's goes out with its `Join`.
    pub fn with_shield(mut self, shield: u8) -> Lobby {
        self.shield = shield;
        if self.role == Role::Host {
            let slot = self.identity.slot;
            self.roster.players.retain(|p| p.slot != slot);
            let shield = self.roster.free_shield(shield);
            let realm = self.roster.get(slot).map_or(0, |p| p.realm);
            let me = Player { slot, name: self.name.clone(), ready: true, is_host: true, shield, realm };
            self.roster.insert_sorted(me);
        }
        self
    }

    /// C260: the install id a client's `Join` carries.
    pub fn with_install(mut self, install: [u8; 16]) -> Lobby {
        self.install = install;
        self.claim(self.identity.slot, install);
        self
    }

    /// What the host's own last game left in `g_startingGoldChosen`.
    pub fn with_unseated_gold(mut self, gold: i32) -> Lobby {
        self.unseated_gold = gold;
        self
    }

    /// The host's game setup, carried to every peer in `Start`.
    pub fn with_setup(mut self, setup: Vec<u8>) -> Lobby {
        self.setup = setup;
        self
    }

    pub fn role(&self) -> Role {
        self.role
    }

    pub fn roster(&self) -> &Roster {
        &self.roster
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn local_slot(&self) -> PlayerSlot {
        self.identity.slot
    }

    pub fn started(&self) -> Option<&Start> {
        self.started.as_ref()
    }

    pub fn greet(&mut self) -> Result<(), LobbyError> {
        if self.role != Role::Client {
            return Err(LobbyError::WrongRole);
        }
        self.outbox.push(Outgoing {
            to: None,
            message: crate::packet::Message::Join(Join {
                hello: self.identity.clone(),
                name: self.name.clone(),
                shield: self.shield,
                install: self.install,
            }),
        });
        Ok(())
    }

    pub fn next_outgoing(&mut self) -> Option<Outgoing> {
        if self.outbox.is_empty() {
            None
        } else {
            Some(self.outbox.remove(0))
        }
    }

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
        if let Some(p) = self.roster.unseated() {
            return Err(LobbyError::Unseated(p.name.clone()));
        }
        let start = Start {
            seed: self.identity.seed,
            roster: self.roster.clone(),
            unseated_gold: self.unseated_gold,
            setup: self.setup.clone(),
            save: self.save.clone(),
        };
        self.started = Some(start.clone());
        self.outbox.push(Outgoing {
            to: None,
            message: crate::packet::Message::Start(start.clone()),
        });
        Ok(start)
    }

    pub fn peer_left(&mut self, peer: PeerId) -> LobbyEvent {
        if let Some(i) = self.seats.iter().position(|(p, _)| *p == peer) {
            let (_, slot) = self.seats.remove(i);
            self.roster.players.retain(|p| p.slot != slot);
            self.broadcast_roster();
            return LobbyEvent::Left(slot);
        }
        LobbyEvent::Nothing
    }

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
            (Role::Client, Message::Refused(reasons)) => Err(LobbyError::Incompatible(reasons)),

            _ => Err(LobbyError::WrongRole),
        }
    }

    fn on_join(&mut self, peer: PeerId, join: Join) -> Result<LobbyEvent, LobbyError> {
        if self.started.is_some() {
            return Err(LobbyError::AlreadyStarted);
        }

        let mismatches: Vec<Mismatch> = self
            .identity
            .check(&join.hello)
            .into_iter()
            // The host seats and seeds: a joiner cannot know either beforehand.
            .filter(|m| !matches!(m, Mismatch::SameSlot(_) | Mismatch::Seed { .. }))
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

        let shield = self.roster.free_shield(join.shield);
        self.roster.insert_sorted(Player { slot, name, ready: false, is_host: false, shield, realm: 0 });
        self.seats.push((peer, slot));
        self.claim(slot, join.install);
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

    pub(super) fn broadcast_roster(&mut self) {
        if self.role == Role::Host {
            self.outbox.push(Outgoing {
                to: None,
                message: crate::packet::Message::Roster(self.roster.clone()),
            });
        }
    }
}

