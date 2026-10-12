//! C257: between ticks, the app carries out what this seat asked of its network
//! game: the season's save on the host, and a leave. `FUN_0043441C`'s network arm
//! is `Net_LeaveGameChecked()` then the front end; ours says goodbye and closes the
//! connections through `Live::leave` (C260): a joiner's realm goes to the computer,
//! a host's leave ends the game for everyone.

use super::Live;
use crate::game::Game;

/// C257: the save records each seat's realm and install; C263 seats a resumed game by
/// them. A joiner knows only its own install (C260's `Join`s reach the host).
pub fn own_seat(game: &mut Game) {
    let install = |slot| game.net.installs.iter().find(|&&(s, _)| s == slot).map(|&(_, i)| i);
    let mut owners: Vec<(u8, super::InstallId)> =
        game.seats.iter().filter_map(|&(slot, realm)| install(slot).map(|i| (realm, i))).collect();
    if !owners.iter().any(|&(r, _)| r == game.player) {
        owners.push((game.player, game.prefs.install));
    }
    owners.sort();
    game.owners = owners;
}

pub fn end_session(live: &mut Option<Live>, game: &mut Game) -> bool {
    if let Some(Err(e)) = crate::saves::write_season(game) {
        eprintln!("network autosave: {e}");
    }
    if game.leaving.take().is_none() {
        return false;
    }
    if let Some(mut l) = live.take() {
        l.leave();
    }
    game.net = super::Link::default();
    true
}
