//! A network game from the lobby's `Start`. `docs/netcode.md` D-3a, D-3b, D-5, D-12.

use crate::game::{Assets, Game};
use crate::scenario::{self, Error};
use crate::setup::Settings;
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::Tables;
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

/// `SetupScreen::new_game`'s sequence with every input from `start`: the seed,
/// each seat's shield, and the unseated realms' gold. The caller's `settings`
/// are the host's; `me` picks only `Game::player`.
pub fn new_game(
    assets: &Assets,
    map: usize,
    settings: &Settings,
    start: &Start,
    me: PlayerSlot,
    tables: Tables,
) -> Result<Game, Error> {
    let seats = seats(start);
    let mut people = [0u8; MAX_REALMS];
    for (p, &(_, realm)) in start.roster.players.iter().zip(&seats) {
        people[realm as usize] = p.shield;
    }
    let local = seats.iter().find(|(s, _)| *s == me).map_or(1, |&(_, r)| r);
    let seed = scenario::new_game_seed(assets.wall_clock, Some(start));
    let mut game = scenario::new_seated_game(assets, map, settings, &people, local, seed, tables)?;
    // FUN_0049C995 0x0049C995 :5760-5763, from the host's process, not this one's.
    crate::setup::carry_chosen_gold(&mut game, start.unseated_gold, settings.gold);
    settings.apply_to(&mut game);
    game.last_report = Some(game.kingdom.start_new_game());
    game.seats = seats;
    game.multiplayer = true;
    Ok(game)
}
