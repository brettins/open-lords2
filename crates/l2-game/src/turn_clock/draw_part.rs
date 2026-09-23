#![allow(unused_imports)]
use super::*;
use super::clock::*;
use screens::*;
use l2_view::Canvas;
use crate::game::Game;
use crate::screen::{Ctx, ScreenId};
use crate::shell::{font, Pen};

pub fn shown(game: &Game) -> Option<i32> {
    let ended = crate::turn::players_turn_ended(game) || game.turn_clock.end_turn_pending();
    game.turn_clock.value(game.kingdom.options.time_limit, ended, game.battle.is_some())
}

/// A network game's clock is `net::frame`'s (the expiry is the world's); the
/// letter `Machine::run_turn_clock` closes on `Tick::Expired` is the seat's,
/// closed when it sees `remaining` fall to [`EXPIRED`]. `[I]` ours,
/// `docs/netcode.md` §5; `Msg_Dismiss` as in `0x0049A010`'s arm.
pub fn seat_saw_expiry(game: &mut Game, last: &mut Option<i32>) -> bool {
    let now = game.turn_clock.remaining();
    let fell = last.is_some_and(|l| l != EXPIRED) && now == EXPIRED;
    *last = Some(now);
    if !fell || !game.messages.is_open() {
        return false;
    }
    crate::message::dismiss(game);
    true
}

/// **`FUN_0041A639`'s two draws.** The caller has already tested the screen.
pub fn draw(ctx: &Ctx, canvas: &mut Canvas) {
    let Some(seconds) = shown(ctx.game) else { return };
    // Nothing in `FUN_0041A639` touches `DAT_005AEA40` or `DAT_0058FE2C`, so
    // this is the ordinary embossed body pen, the menu bar's.
    let pen = Pen {
        assets: &ctx.assets.shell,
        ink: &ctx.assets.ink,
        chrome: ctx.assets.chrome.as_ref(),
        shadow: Some(font::SHADOW),
        caps: None,
    };
    pen.misc_frame(canvas, FRAME, FRAME_X, FRAME_Y);
    pen.number_centred(canvas, NUMBER_X, NUMBER_Y, NUMBER_W, seconds, LEAD, SUFFIX, COLOUR);
}

