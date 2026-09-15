#![allow(unused_imports)]
use super::*;
use super::types::*;
use super::screen::*;
use machine::*;
use l2_view::Canvas;
use crate::game::{Assets, Game};
use crate::input::Event;

/// The screen stack.
pub struct Machine {
    pub(crate) stack: Vec<Box<dyn Screen>>,
    pub(crate) quit: bool,
    /// Set whenever anything happened that could change what is on screen.
    /// The event loop consults it to decide whether to repaint, which is the
    /// difference between a still menu costing nothing and costing a GPU
    /// submission sixty times a second.
    pub(crate) dirty: bool,
    /// **Every widget click the stack has made since the process started.**
    ///
    /// Monotone on purpose. [`crate::audio::Director`] keeps the previous
    /// tick's value and plays `click3.wav` when this has moved, which is the
    /// same diffing it already does for the screen stack and for where every
    /// unit stood — and it is the only shape that survives the thing that makes
    /// a per-screen counter useless: **the click that opens a screen is
    /// counted by the screen that is then popped.** A sum over the live stack
    /// would lose exactly the presses a player notices most.
    ///
/// Drained out of the screens by [`Machine::handle`]
    /// them,
    /// with it. See [`Screen::take_clicks`].
    pub(crate) clicks: u32,
    /// `(g_mouseX, g_mouseY)` — the last canvas pixel an event put the pointer
    /// on. The original reads `GetCursorPos` every frame; ours hears about it.
    pub(crate) pointer: (i32, i32),
    /// **`g_mouseInputChanged`** — the pointer moved or a button changed since
    /// the last tick. `FUN_004B191E` recomputes it once a frame, so it is taken
    /// once a tick, by [`Machine::run_tooltips`].
    pub(crate) pointer_changed: bool,
    /// **The tool tips** — `FUN_00476E95`'s state. Here and not on
    /// [`Game`], because it counts frames and `Game` is compared whole by the
    /// save round trips. See [`crate::tooltip`].
    pub(crate) tooltips: crate::tooltip::Tooltips,
    /// The screens the last tick painted, to see a repaint — `Screen_Draw`
    /// opens with `FUN_0047703A`.
    pub(crate) tooltip_screens: Vec<ScreenId>,
    /// `g_optToolTips` as the last tick saw it, to see `Opt_ToggleToolTips`.
    pub(crate) tool_tips_seen: Option<bool>,
    /// **A `Save_RotateAndWrite` (`0x0049A453`) is owed**, drained out of the
/// screens. See [`Screen::take_autosave`] and
    /// [`crate::saves::run_pending`].
    ///
    /// A latch and not a counter, because two turns cannot come round between
    /// two pumps: the request is raised in a tick and taken in the same tick's
    /// tail by the application.
    pub(crate) autosave: bool,
    /// The screen the tip host was seated over — what `Tip_Show`
    /// (`0x00476DA9`) saved in `_DAT_004F0350`. See
    /// [`Machine::seat_tip_host`].
    pub(crate) tip_seat: Option<ScreenId>,
}


