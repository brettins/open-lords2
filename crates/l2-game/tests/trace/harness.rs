//! What native/l2-trace's stand-ins do to the original, done to ours so the two traces compare. C246.

use l2_game::audio::Audio;

/// dsound_null.cpp:50 `GetStatus`: `[I]` one-shots end at once, so `Sound_OneShotBusy` 0x00427C9B
/// answers 0 the frame after a take starts. Ours drains nothing headless; end it the same way.
pub fn end_one_shots(audio: &mut Audio) {
    audio.stop_one_shot();
}
