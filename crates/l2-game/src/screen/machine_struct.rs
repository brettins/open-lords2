#![allow(unused_imports)]
use super::*;
use super::types::*;
use super::screen::*;
use machine::*;
use l2_view::Canvas;
use crate::game::{Assets, Game};
use crate::input::Event;

pub struct Machine {
    pub(crate) stack: Vec<Box<dyn Screen>>,
    pub(crate) quit: bool,
    pub(crate) dirty: bool,
    pub(crate) clicks: u32,
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
    pub(crate) tool_tips_seen: Option<bool>,
    /// **A `Save_RotateAndWrite` (`0x0049A453`) is owed**, drained out of the
/// screens. See [`Screen::take_autosave`] and
    /// [`crate::saves::run_pending`].
    pub(crate) autosave: bool,
    /// The screen the tip host was seated over — what `Tip_Show`
    /// (`0x00476DA9`) saved in `_DAT_004F0350`. See
    /// [`Machine::seat_tip_host`].
    pub(crate) tip_seat: Option<ScreenId>,
}


