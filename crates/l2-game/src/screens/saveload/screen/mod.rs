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
    /// The highlighted row, as an index into [`SaveLoadScreen::entries`].
    selected: Option<usize>,
    /// **The edit buffer the name field shows — `DAT_004EA130`.**
    ///
    /// This was a `String` with a `push` and a `pop` and no caret, and it was
    /// the only text field in the workspace. It is [`crate::text::TextField`]
    /// now, which is the original's own editor, so this screen gained Delete,
    /// Home, End, the left and right arrows, insert mode, a blinking caret and
    /// the character filter in one change. `docs/arms.json`, group `text`.
    name: crate::text::TextField,
    status: Status,
    /// `g_saveLoadWidgets`' press timers and repeat counter.
    press: Press,
    /// `DAT_0057D3C4` — frames left before the load or the save; 0 is none.
    working: u8,
}

/// `Edit_Begin(&DAT_004EA130, 8, 0xA0, 1)` — the save box's own arguments, and
/// **one of the three is deliberately not the original's.**
///
/// * **kind 1**, the file-name kind: `A`–`Z` are lower-cased and `,` `.` `?`
///   `!` are refused outright. Reproduced. A name this field accepts is a name
///   the file layer never has to sanitise, so the original has the
///   kind at all.
/// * **160 pixels**, on a 192-pixel plate. Reproduced: it is what stops a name
///   from drawing out of its recess, and that is as true of our plate as of
///   theirs.
/// * **eight characters — not reproduced.** Eight is a DOS 8.3 file name, and
///   the game appends the extension itself (`SaveLoad_Tick` copies twelve bytes
///   of the buffer and calls `FUN_004AF675` to add `.sav`, `.svb` or `.sva`).
///   Our saves are `.l2sav` files in `%APPDATA%` and [`saves::MAX_NAME`] is 64;
///   holding a person to eight characters on a filesystem that has not had that
/// limit since 1995 would be superstition, which is the
///   line this module's header already draws about the scroll clamp. **In
///   practice the pixel limit bites first** and a name never gets near 64.
pub(super) fn begin_name(seed: &str) -> crate::text::TextField {
    crate::text::TextField::begin(seed, saves::MAX_NAME, 0xA0, crate::text::Kind::Filename)
}

