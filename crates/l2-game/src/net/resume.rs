//! C263: a saved game continued over the network. The host's picker reads the
//! save; the room shows its realms; every peer builds the world from the same
//! bytes in `Start`. `docs/netcode.md` §8.

use crate::game::Game;
use crate::scenario::Error;
use l2_kingdom::tables::Tables;
use l2_net::{PlayerSlot, SavedRealm, Start};

/// What the host's picker hands the lobby.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resume {
    pub saved: Vec<SavedRealm>,
    pub save: Vec<u8>,
}

impl Resume {
    /// `[D]` every realm still in play is offered; `Game::owners` names who played it.
    pub fn of(game: &Game, save: Vec<u8>) -> Resume {
        let realms = game.kingdom.realms.iter().enumerate().skip(1);
        let saved = realms
            .filter(|(_, r)| r.in_play)
            .map(|(i, r)| {
                let owner = game.owners.iter().find(|(o, _)| *o as usize == i).map_or([0; 16], |(_, id)| id.0);
                SavedRealm { realm: i as u8, name: game.player_names[i].as_str().to_string(), shield: r.shield_index, owner }
            })
            .collect();
        Resume { saved, save }
    }

    /// The realm this install played, for the host's own seat.
    pub fn mine(&self, install: super::InstallId) -> Option<&SavedRealm> {
        self.saved.iter().find(|r| !install.is_nil() && r.owner == install.0)
    }
}

/// The seats a resumed `Start` names: every player the host seated.
pub fn seats(start: &Start) -> Vec<(PlayerSlot, u8)> {
    start.roster.players.iter().filter(|p| p.realm != 0).map(|p| (p.slot, p.realm)).collect()
}

/// `saves::read_path`'s load on every peer, then the seats: a seated realm is a
/// person's, every other the computer's (C240's rule for an empty seat).
pub fn build(start: &Start, me: PlayerSlot, tables: Tables) -> Result<Game, Error> {
    let missing = |detail: String| Error::Missing { name: "the host's save".into(), detail };
    let mut game = crate::save::decode(&start.save, tables).map_err(|e| missing(e.to_string()))?;
    crate::save::resume(&mut game);
    let seats = seats(start);
    let seated = |r: u8| seats.iter().any(|&(_, s)| s == r);
    for (r, realm) in game.kingdom.realms.iter_mut().enumerate() {
        realm.is_human = seated(r as u8);
    }
    for (_, u) in game.kingdom.campaign.units.iter_mut() {
        u.owner_is_human = seated(u.owner);
    }
    game.player = seats.iter().find(|(s, _)| *s == me).map(|&(_, r)| r).ok_or_else(|| missing("this seat has no realm".into()))?;
    // [D] g_moveCost 0x004F4080 is in no save block (B108): a single-player load keeps
    // the last game's. Built here, it is the one Map_BeginMoveSelection rebuilds on one seat.
    game.kingdom.campaign.rebuild_move_cost();
    game.seats = seats;
    game.multiplayer = true;
    Ok(game)
}
