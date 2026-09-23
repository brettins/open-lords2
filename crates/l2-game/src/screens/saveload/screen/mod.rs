#![allow(unused_imports)]

mod methods;
pub use methods::*;
mod screen_impl;
pub use screen_impl::*;

use super::*;
use super::helpers::*;
use super::tests::*;
use l2_view::{text, Canvas};
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::saves::{self, Entry};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};

pub struct SaveLoadScreen {
    mode: Mode,
    pub(super) entries: Vec<Entry>,
    /// `g_fileListTop` (`0x004EA1A0`) — the index the visible page starts at.
    pub(super) top: usize,
    selected: Option<usize>,
    /// **The edit buffer the name field shows — `DAT_004EA130`.**
    name: crate::text::TextField,
    status: Status,
    press: Press,
    /// `DAT_0057D3C4` — frames left before the load or the save; 0 is none.
    working: u8,
    /// `DAT_00591548`, the failure latch: set, the count ends by clearing it instead of working.
    pub(super) latched: bool,
    /// `g_redrawRequest = 2` — `SaveLoad_Tick` (`0x004AD9F0`) sets it after the
    /// three `g_screenId` arms, so the frame the count reaches zero is painted
    /// whatever the outcome was.
    redraw: bool,
    /// Box origin less `Screen_SaveLoad`'s `(0x10, 0x90)`: `SaveLoad_DrawStatus` 0x004149EC
    /// takes `(0x60, 10)` on the front end's page 3, `Widget_Draw(0x60,10,..)` 004b0000.c:7067. [V]
    pub(super) shift: (i32, i32),
}

/// `Edit_Begin(&DAT_004EA130, 8, 0xA0, 1)` — the save box's own arguments, and
/// **one of the three is deliberately not the original's.**
///
/// * **eight characters — not reproduced.** Eight is a DOS 8.3 file name, and
///   the game appends the extension itself (`SaveLoad_Tick` copies twelve bytes
///   of the buffer and calls `FUN_004AF675` to add `.sav`, `.svb` or `.sva`).
pub(super) fn begin_name(seed: &str) -> crate::text::TextField {
    crate::text::TextField::begin(seed, saves::MAX_NAME, 0xA0, crate::text::Kind::Filename)
}

