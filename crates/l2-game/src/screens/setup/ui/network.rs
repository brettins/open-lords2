#![allow(unused_imports)]
use super::*;
use crate::net::{Identity, Request, Setup, DEFAULT_PORT};

/// *Multiple players* → the lobby's entry page → *Host a game* → page 4 → page 7
/// → *Start* opens the room, hosting. `[D]` ours: `docs/netcode.md` *The lobby*,
/// C255; the original's pages 8/11 ran DirectPlay, which we do not reproduce.
impl SetupScreen {
    /// `[D]` ours: page 4's custom game without a lobby, for a headless walk.
    pub fn set_network(&mut self, on: bool) {
        self.network = on;
    }

    pub fn network(&self) -> bool {
        self.network
    }

    pub(crate) fn open_lobby(&mut self, ctx: &mut Ctx) -> Transition {
        ctx.game.net.identity = Some(Identity {
            name: self.name.commit(text::PLAYER_NAME_LEN),
            shield: self.shield(),
            setup: Setup { map: self.map as u8, options: self.options },
        });
        ctx.game.net.requests.push(Request::Host { addr: Request::every_interface(DEFAULT_PORT) });
        Transition::Push(ScreenId::Lobby)
    }
}
