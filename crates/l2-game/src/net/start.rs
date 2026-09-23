//! A network game from the lobby's `Start`. `docs/netcode.md` D-3a, D-3b, D-5, D-12.

use super::Setup;
use crate::game::{Assets, Game};
use crate::scenario::{self, Error};
use crate::setup::Settings;
use crate::text::{PlayerName, PLAYER_NAME_LEN};
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::Tables;
use l2_kingdom::Quirks;
use l2_net::{Canonical, Hello, PlayerSlot, Start, PROTOCOL_VERSION};

/// What two peers must hold identically, refused on mismatch (D-12).
/// `[I]` the ruleset is the resolved `Tables`, which is where a mod lands.
pub fn hello(settings: &Settings, tables: &Tables, seed: u64, slot: PlayerSlot) -> Hello {
    Hello {
        protocol: PROTOCOL_VERSION,
        engine: concat!("lords2 ", env!("CARGO_PKG_VERSION")).to_string(),
        ruleset_hash: Canonical::hash_of(tables),
        quirks: settings.kingdom_options().quirks.bits(),
        seed,
        slot,
    }
}

/// `[I]` the roster's k-th seat, in slot order, plays realm k + 1.
pub fn seats(start: &Start) -> Vec<(PlayerSlot, u8)> {
    let players = start.roster.players.iter().take(MAX_REALMS - 1);
    players.enumerate().map(|(k, p)| (p.slot, k as u8 + 1)).collect()
}

/// `SetupScreen::new_game`'s sequence with every input from `start`: the host's
/// map and options, the seed, each seat's shield and name, and the unseated
/// realms' gold. `me` picks only `Game::player`; `quirks` are Hello-checked.
pub fn new_game(
    assets: &Assets,
    start: &Start,
    me: PlayerSlot,
    quirks: Quirks,
    tables: Tables,
) -> Result<Game, Error> {
    let setup = Setup::decode(&start.setup).ok_or_else(|| Error::Missing {
        name: "Start".into(),
        detail: "the host's setup did not decode".into(),
    })?;
    let seats = seats(start);
    let settings = setup.settings(seats.len(), quirks);
    let mut people = [0u8; MAX_REALMS];
    for (p, &(_, realm)) in start.roster.players.iter().zip(&seats) {
        people[realm as usize] = p.shield;
    }
    let local = seats.iter().find(|(s, _)| *s == me).map_or(1, |&(_, r)| r);
    let seed = scenario::new_game_seed(assets.wall_clock, Some(start));
    let map = setup.map as usize;
    let mut game = scenario::new_seated_game(assets, map, &settings, &people, local, seed, tables)?;
    // FUN_0049C995 0x0049C995 :5760-5763, from the host's process, not this one's.
    crate::setup::carry_chosen_gold(&mut game, start.unseated_gold, settings.gold);
    settings.apply_to(&mut game);
    name_the_lords(&mut game, assets, start, &seats);
    game.last_report = Some(game.kingdom.start_new_game());
    game.seats = seats;
    game.multiplayer = true;
    Ok(game)
}

/// `Player_SetHuman` (`0x0049BAE9`) per seat, with its roster name; the rest
/// `Eng_Seek(7, lord)`, sixteen bytes — as `SetupScreen::name_the_lords`.
fn name_the_lords(game: &mut Game, assets: &Assets, start: &Start, seats: &[(PlayerSlot, u8)]) {
    use crate::screens::setup::LORD_TITLE_GROUP;
    for realm in 0..MAX_REALMS {
        let seated = seats.iter().find(|&&(_, r)| r as usize == realm);
        let name: String = match seated.and_then(|&(s, _)| start.roster.get(s)) {
            Some(p) => p.name.chars().take(PLAYER_NAME_LEN).collect(),
            None => {
                let lord = game.kingdom.realms[realm].lord as usize;
                let title = assets.shell.text(LORD_TITLE_GROUP, lord.min(4));
                title.chars().take(0x10).collect()
            }
        };
        game.player_names[realm] = PlayerName::new(&name);
    }
}
