//! What native/l2-trace's stand-ins do to the original, done to ours so the two traces compare. C246.

use l2_game::audio::Audio;
use l2_game::message::Record;
use l2_game::{Game, Machine, ScreenId};

/// Message 298 "Non 256 colour display": `Msg_Enqueue(0, local, 298, 0, 0, …)` from 0x004B9A23 in
/// `Battle_Frame` 0x004B99C0, because the fake DirectDraw reports a non-8-bit mode. `[V]` every
/// original new-game trace posts it on the map's first frame (newgame-original f68), and its box
/// holds `Tip_Show`'s 0x27 until its OK runs `FUN_00476E21` (f373 0x27 -> 0), so each tip after
/// it shows over screen 0.
pub fn post_non_256_colour(posted: &mut bool, machine: &Machine, game: &mut Game) {
    if *posted || !machine.ids().contains(&ScreenId::Campaign) {
        return;
    }
    *posted = true;
    let player = game.player;
    let record = Record { to: player, from: 0, group: 298, variant: 0, category: 0, county: 0, spare: 0, payload: 0 };
    game.messages.enqueue(record, player);
}

/// dsound_null.cpp:50 `GetStatus`: `[I]` one-shots end at once, so `Sound_OneShotBusy` 0x00427C9B
/// answers 0 the frame after a take starts. Ours drains nothing headless; end it the same way.
pub fn end_one_shots(audio: &mut Audio) {
    audio.stop_one_shot();
}
