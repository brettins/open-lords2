#![allow(unused_imports)]
use super::*;
use crate::net::{Identity, Setup};

/// *Multiple players* → page 4 → page 7 → *Start* opens our lobby instead of a
/// world. `[D]` ours: `docs/netcode.md` *The lobby*; the original's pages 8/11
/// ran DirectPlay, which we do not reproduce.
impl SetupScreen {
    pub(crate) fn open_lobby(&mut self, ctx: &mut Ctx) -> Transition {
        ctx.game.net.identity = Some(Identity {
            name: self.name.commit(text::PLAYER_NAME_LEN),
            shield: self.shield(),
            setup: Setup { map: self.map as u8, options: self.options },
        });
        Transition::Push(ScreenId::Lobby)
    }
}
