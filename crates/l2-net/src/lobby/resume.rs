//! C263: a saved game continued in the room. The host's save rides in `Start` and
//! every peer builds from it; a player is seated on the realm its install played,
//! the host seats the rest, and a realm nobody holds is the computer's. Ours: the
//! original's resume is net command `0x1D`, every peer reading its own copy of the
//! save (`NetCmd_Read_1D` `0x00443143`).

use super::*;
use crate::canonical::{Canonical, CodecError, Decode, Encode, Reader};

/// One realm of the saved game; `owner` is the install that played it, zero for none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedRealm {
    pub realm: u8,
    pub name: String,
    pub shield: u8,
    pub owner: [u8; 16],
}

impl Encode for SavedRealm {
    fn encode(&self, out: &mut Canonical) {
        out.u8(self.realm);
        out.str(&self.name);
        out.u8(self.shield);
        out.raw(&self.owner);
    }
}

impl Decode for SavedRealm {
    fn decode(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(SavedRealm {
            realm: input.u8()?,
            name: input.str()?.to_string(),
            shield: input.u8()?,
            owner: input.raw(16)?.try_into().expect("sixteen bytes"),
        })
    }
}

impl Lobby {
    /// The host continues `save`; the room shows `saved`.
    pub fn with_save(mut self, saved: Vec<SavedRealm>, save: Vec<u8>) -> Lobby {
        self.roster.saved = saved;
        self.save = save;
        self.claim(self.identity.slot, self.install);
        self
    }

    pub fn is_resuming(&self) -> bool {
        !self.roster.saved.is_empty()
    }

    /// The host seats `slot` on saved realm `realm`, 0 for none; whoever held it is unseated.
    pub fn assign(&mut self, slot: PlayerSlot, realm: u8) -> Result<(), LobbyError> {
        if self.role != Role::Host {
            return Err(LobbyError::WrongRole);
        }
        if self.started.is_some() {
            return Err(LobbyError::AlreadyStarted);
        }
        if realm != 0 && !self.roster.saved.iter().any(|r| r.realm == realm) {
            return Err(LobbyError::NoRealm(realm));
        }
        if self.roster.get(slot).is_none() {
            return Err(LobbyError::SlotTaken(slot));
        }
        for p in self.roster.players.iter_mut() {
            if p.slot == slot {
                p.realm = realm;
            } else if realm != 0 && p.realm == realm {
                p.realm = 0;
            }
        }
        self.broadcast_roster();
        Ok(())
    }

    /// Seats `slot` on the saved realm `install` played, if nobody holds it.
    pub(super) fn claim(&mut self, slot: PlayerSlot, install: [u8; 16]) {
        if install == [0; 16] || self.roster.saved.is_empty() {
            return;
        }
        let roster = &self.roster;
        let mine = roster.saved.iter().find(|r| r.owner == install && roster.holder(r.realm).is_none());
        if let Some(realm) = mine.map(|r| r.realm) {
            if let Some(p) = self.roster.players.iter_mut().find(|p| p.slot == slot && p.realm == 0) {
                p.realm = realm;
            }
        }
    }
}

impl Roster {
    /// The first player a resumed game has not seated.
    pub fn unseated(&self) -> Option<&Player> {
        if self.saved.is_empty() {
            return None;
        }
        self.players.iter().find(|p| p.realm == 0)
    }

    /// Who plays saved realm `realm`.
    pub fn holder(&self, realm: u8) -> Option<&Player> {
        self.players.iter().find(|p| p.realm == realm && realm != 0)
    }
}
