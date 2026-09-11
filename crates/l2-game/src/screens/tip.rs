//! **`g_screenId` `0x27` — the screen a tip is shown on.**
//!
//! `Tip_Show` (`0x00476DA9`) saves the screen byte and writes `0x27`, and that
//! is the whole of what this screen is: a value of `g_screenId` under which the
//! message pump runs and no per-screen arm does. The window itself is the
//! message scroll's categories `0x05`…`0x09` — [`crate::screens::message`] —
//! and the ladder that decides which tip is [`crate::tip`].
//!
//! # It draws nothing and answers nothing
//!
//! The decompilation compares `g_screenId` with `0x27` in three functions —
//! `Msg_Pump`, `Tip_Show` and `FUN_00476E21` — and nowhere else; the one
//! `case 0x27` in the image is `App_WndProc`'s VK_RIGHT. So `Screen_FrameInput`
//! has no arm for it and `Screen_Draw` no painter that a comparison would show.
//! `[I]`: a jump-table dispatch would not appear as a comparison and was not
//! looked for. What the player sees is the screen underneath, still drawn, and
//! the scroll over it.
//!
//! **Every event is consumed.** A click the scroll does not want falls through
//! to here, and here it stops, because the screen underneath is not
//! `g_screenId` any more and its arms do not run. Two things the original may
//! still offer on `0x27` are **not** offered: `Screen_HandleInput`'s widget
//! pass, which would find no table for `0x27`, and `Screen_FrameInput`'s
//! epilogue — the minimap press that runs on every id but `0x12` — which would
//! drop the byte to `0` without `FUN_00476E21` and so without the twenty-frame
//! re-arm. Neither has been driven against the original; recorded rather than
//! guessed.

use l2_view::Canvas;

use crate::input::Event;
use crate::screen::{Ctx, Screen, ScreenId, Transition};

/// Screen `0x27`. See the module header.
#[derive(Debug, Default)]
pub struct TipScreen;

impl TipScreen {
    pub fn new() -> TipScreen {
        TipScreen
    }
}

impl Screen for TipScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Tip
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "Tip".into()
    }

    /// The screen underneath is still painted; `0x27` has no painter.
    fn is_overlay(&self) -> bool {
        true
    }

    /// No arm for `0x27`, so nothing reaches the screen underneath.
    fn handle(&mut self, _event: Event, _ctx: &mut Ctx) -> Transition {
        Transition::Stay
    }

    fn draw(&mut self, _ctx: &Ctx, _canvas: &mut Canvas) {}
}
