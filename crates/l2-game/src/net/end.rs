//! C257: between ticks, the app carries out what this seat asked of its network
//! game: the season's save on the host, and a leave. `FUN_0043441C`'s network arm
//! is `Net_LeaveGameChecked()` then the front end; ours closes the connections
//! through `Live::leave`, and the others see the seat go (C240's halt).

use super::Live;
use crate::game::Game;

/// The save records this seat's own realm and install; the others' are not on
/// the wire yet, so a save names the host alone.
pub fn own_seat(game: &mut Game) {
    game.owners = vec![(game.player, game.prefs.install)];
}

/// `true` when the session ended: its connections are closed and the link reset.
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
