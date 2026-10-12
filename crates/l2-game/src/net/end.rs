//! C257: between ticks, the app carries out what this seat asked of its network
//! game: the season's save on the host, and a leave. `FUN_0043441C`'s network arm
//! is `Net_LeaveGameChecked()` then the front end; ours says goodbye and closes the
//! connections through `Live::leave` (C260): a joiner's realm goes to the computer,
//! a host's leave ends the game for everyone.

use super::Live;
use crate::game::Game;

/// C257: the save records this seat's realm and install; the others' are not on
/// the wire yet, so a save names the host alone.
pub fn own_seat(game: &mut Game) {
    game.owners = vec![(game.player, game.prefs.install)];
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
