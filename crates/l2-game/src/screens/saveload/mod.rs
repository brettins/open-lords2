//! `g_screenId` **`0x35` and `0x36`** — loading and saving a conquest.
//!
//! These were two rows of [`crate::screens::shells::SHELLS`] until now: the
//! original's window, the original's heading, and a click that closed them
//! again. They are screens now, and every coordinate below was read out of the
//! painter.
//!
//! # The painter, address by address
//!
//! `Screen_SaveLoad(saving)` (`0x00414819`) is **one painter with a mode flag**
//! — the two `g_screenId` values differ only in which of `L2.eng` group 40's
//! first two strings is used as the heading, *"Loading a conquest."* or
//! *"Saving a conquest."* `Screen_Draw` calls `Screen_SaveLoad(0)` for `0x35`
//! and `Screen_SaveLoad(1)` for `0x36`, so **the argument is the string index**.
//!
//! ```text
//! Screen_SaveLoad(saving):                                      0x00414819
//!   FUN_004B1DE0()                                     the clip reset, not a draw
//!   Ui_DrawBox(0x10, 0x90, 0x1C, 0x14)      the window at (16, 144), 448 x 320
//!   Gfx_MarkAllDirty()
//!   Eng_DrawString(40, saving, 0x20, 0xA0, heading, 0x3F)          (32, 160)
//!   FUN_00403CF4(0x20, 200,   400,   0x100, 0x3F)   (32, 200)  400 x 256
//!   FUN_00403CF4(0x28, 0xD0,  0xC0,  0x20,  0x3F)   (40, 208)  192 x 32
//!   FUN_00403CF4(0x28, 0xF8,  0x160, 0xA4,  0x3F)   (40, 248)  352 x 164
//!   FUN_00403CF4(0x28, 0x1A4, 0x180, 0x1C,  0x3F)   (40, 420)  384 x 28
//!   DAT_004E65DC = 999
//!   SaveLoad_DrawStatus()                    twelve of the fourteen draws
//!
//! SaveLoad_DrawStatus(selected, frontEnd):                      0x004149EC
//!   origin = frontEnd ? (0x60, 0x0A) : (0x10, 0x90)
//!   two blanks at (org + 0x1C, org + 0x42) and (+ 0x4C), 10 x 1 cells
//!   Ui_DrawText(g_editBuffer, org + 0x20, org + 0x48, body, 0x3F)
//!   FUN_0040ACCE(0x5AF8F0, 0x3F)                     Edit_DrawCaret
//!   one blank at (org + 0x1E, org + 0x6A), 21 x 10 cells       the list well
//!   for i in top .. count:  Ui_DrawText(name[i], x, y, body, i == sel ? 0x20 : 0x3F)
//!   one blank at (org + 0x20, org + 0x118), 21 x 1 cells       the status well
//!   if (DAT_0057D3C4)
//!     error   -> Eng_DrawString(40, 4, org + 0x20, org + 0x11A, body, 0x3F)
//!     0x35    -> Eng_DrawString(40, 2, …)      "Loading game. Please wait."
//!     0x1F    -> Eng_DrawString(40, 2, …)      the front end also only loads
//!     else    -> Eng_DrawString(40, 3, …)      "Saving game. Please wait."
//! ```
//!
//! **[V]** from the two functions' own bodies.
//!
//! ## `FUN_00403CF4` is not `Ui_DrawInsetRect`, and this module used to say it was
//!
//! The four rectangles were transcribed here as `Ui_DrawInsetRect(x, y, w, h)`
//! (`0x00403DEB`) — colour `0x10` on the top and right and `0x1F` on the bottom
//! and left, a *lit* recess. The painter calls **`FUN_00403CF4(x, y, w, h,
//! colour)`** instead, which is four `FUN_00403A8F` lines **all in the one
//! colour the caller passes**, and every one of the four call sites passes
//! `0x3F`. So all four are **flat single-colour outlines**, not recesses, and
//! [`rect_outline`] is that. The two functions are 249 and 247 bytes and sit
//! seventeen bytes apart; the mistake was reading the name and not the body.
//!
//! The two-tone one *is* in this screen's family — the front end's twin
//! `FUN_004148E4` opens with `FUN_00403EE4(0x70, 0x42, 400, 0x100)`, whose
//! colours are `0x35` top and right and `0x28` bottom and left, a **third**
//! bevel that is neither of the other two.
//!
//! ## The status flag reaches every place it should — checked
//!
//! `0x35` and `0x36` are one painter and one flag, so *"a load screen that says
//! Save somewhere"* is the defect to look for. There are exactly two places the
//! mode is read and **they read two different variables**:
//!
//! * the heading reads the painter's `saving` **argument**;
//! * the status line reads **`g_screenId`** directly, and its ladder is
//!   `0x35 → 2`, `0x1F → 2`, *anything else* `→ 3`.
//!
//! Both are right, because `Screen_Draw` is the only thing that sets either and
//! it sets them together. They are not the same
//! source: the `else` arm means every screen id that is not `0x35` or `0x1F`
//! gets *"Saving game."*, which is correct today only because `0x36` is the
//! sole remaining caller. `SaveLoad_DrawStatus` is reached from three places —
//! `Screen_SaveLoad`, `FUN_004148E4` (the front end's page 3) and
//! `Screen_DrawWidgets` — and never from a fourth.
//!
//! The list is the interesting part:
//!
//! ```c
//! x = box.x + 0x20; y = box.y + 0x6C;              // (48, 252)
//! for (i = g_fileListTop; i < g_fileListCount; i++) {
//!     if (i == selected) { sprite 6 x 16 at (x - 2, y - 1); text in 0x20; }
//!     else                                          text in 0x3F;
//!     if (column == 2) { x = box.x + 0x20; y += 0x10; column = -1; }
//!     else               x += 0x78;
//!     if (drawn > 0x1C) break;
//!     column++; drawn++;
//! }
//! ```
//!
//! — **three columns 120 apart, ten rows 16 apart, thirty names visible**, and
//! the interior it fills (`Ui_DrawBoxInterior(box.x + 0x1E, box.y + 0x6A, 0x15,
//! 10)`) is exactly 21 × 10 cells, which is those ten rows. The names come from
//! a table of **65-byte records** at `0x004E8790`, which is where
//! [`crate::saves::MAX_NAME`]'s 64 comes from.
//!
//! # The four widgets, and the one thing here that is inferred
//!
//! `g_saveLoadWidgets` (`0x004DDD78`) holds four 24-byte records —
//! `node tools/oracle/widgets.js widgets 4ddd78 4`:
//!
//! | # | x | y | frame | size | handler |
//! |---|---|---|---|---|---|
//! | 0 | 304 | 64 | 29 | 32 | `0x004342F3` — confirm |
//! | 1 | 352 | 64 | 31 | 32 | `SaveLoad_Cancel` `0x00434308` |
//! | 2 | 384 | 144 | 35 | 24 | `SaveLoad_Scroll` −3, list 1 |
//! | 3 | 384 | 176 | 37 | 24 | `SaveLoad_Scroll` +3, list 1 |
//!
//! **[I] — those coordinates are treated here as relative to the box origin,
//! not absolute.** Read absolutely, all four sit above or on the top edge of a
//! window that runs from y = 144 to y = 464, which would put the two hands
//! outside the panel they belong to. Read relative to `Ui_DrawBox(0x10, 0x90)`
//! they land at (320, 208) and (368, 208) — level with the name field, whose
//! own rectangle ends at x = 232 — and the arrows at (400, 288) and (400, 320),
//! immediately right of the list, whose rectangle ends at x = 392. The
//! deciding argument is that **the same table serves two screens whose boxes
//! are at different origins**: `FUN_004148E4` draws the identical furniture for
//! the front end's page 3 at `(0x60, 0x0A)`, and one
//! absolute table cannot serve both. It is still an inference, and it is marked
//! as one.
//!
//! # What is ours, and it says so on the screen
//!
//! The *files* are ours: our own format (`crate::save`), in our own directory
//! (`crate::saves`), with names the player types. `docs/decisions.md` C21 —
//! anything of ours is marked as ours — so the directory path and every error
//! that is not one of group 40's three status strings are drawn in
//! `l2_view::text`, **our** 5 × 7 font, never in `Fntl2_14.pl8`. A player
//! looking at this screen can tell at a glance which words are the game's.
//!
//! The scroll clamp is **not** the original's, and that is deliberate.
//! `SaveLoad_Scroll` clamps the top row at `count - 15` while thirty names are
//! visible, so the original can scroll a short list into empty space. Ours
//! clamps at `count - 30`, the number on screen. Reproducing an
//! off-by-fifteen in a list of our own files would be superstition, not
//! fidelity.

mod helpers;
pub use helpers::*;
mod screen;
pub use screen::*;
mod tests;
pub use tests::*;

use l2_view::{text, Canvas};

use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::saves::{self, Entry};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};

/// Which of the two screens this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// `g_screenId` `0x35`.
    Load,
    /// `g_screenId` `0x36`.
    Save,
}

impl Mode {
    /// The `g_screenId` byte.
    pub fn screen_id(self) -> u8 {
        match self {
            Mode::Load => 0x35,
            Mode::Save => 0x36,
        }
    }

    /// The `L2.eng` group 40 index the painter uses as its heading — the
    /// painter's `saving` argument, used directly as a string index.
    pub fn heading_index(self) -> usize {
        match self {
            Mode::Load => 0,
            Mode::Save => 1,
        }
    }

    /// Group 40 index 2 *"Loading game. Please wait."* or 3 *"Saving game.
    /// Please wait."* — what `SaveLoad_DrawStatus` puts on the status line
    /// while the operation runs.
    pub fn working_index(self) -> usize {
        match self {
            Mode::Load => 2,
            Mode::Save => 3,
        }
    }
}

/// `L2.eng` group 40.
pub const GROUP: usize = 40;
/// Group 40 index 4 — *"File error. Operation canceled."*
pub const ERROR_INDEX: usize = 4;

/// The detail under group 40 index 4 when a save is asked for mid-battle. It
/// says the battle is not saved, because that is the thing a player loses.
/// Group 40 has three strings and none of them is about a battle, so this one
/// is ours. See the refusal in [`SaveLoadScreen::handle`].
pub const BATTLE_REFUSAL: &str = "THE BATTLE IS NOT SAVED. FINISH IT, THEN SAVE.";

/// `Ui_DrawBox(0x10, 0x90, 0x1C, 0x14)`.
pub const BOX_X: i32 = 0x10;
pub const BOX_Y: i32 = 0x90;
pub const BOX_COLS: i32 = 0x1C;
pub const BOX_ROWS: i32 = 0x14;

/// `Eng_DrawString(40, saving, 0x20, 0xA0, &g_fontHeading, 0x3F)`.
pub const HEADING: (i32, i32) = (0x20, 0xA0);

/// The colour every one of the painter's four rectangles is drawn in —
/// `FUN_00403CF4(…, 0x3F)`, four times, with no second colour anywhere.
pub const OUTLINE: u8 = 0x3F;

/// The four `FUN_00403CF4(x, y, w, h, 0x3F)` calls, in the painter's order.
pub const INSETS: [(i32, i32, i32, i32); 4] = [
    (0x20, 200, 400, 0x100),
    (0x28, 0xD0, 0xC0, 0x20),
    (0x28, 0xF8, 0x160, 0xA4),
    (0x28, 0x1A4, 0x180, 0x1C),
];

/// The name being typed or shown: `Ui_DrawText(edit, box.x + 0x20, box.y + 0x48)`.
pub const NAME: (i32, i32) = (BOX_X + 0x20, BOX_Y + 0x48);

/// The list's textured interior — `Ui_DrawBoxInterior(box.x + 0x1E,
/// box.y + 0x6A, 0x15, 10)`, so 336 × 160 at (46, 250). `(x, y, w, h)`.
pub const INTERIOR: (i32, i32, i32, i32) = (BOX_X + 0x1E, BOX_Y + 0x6A, 0x15 * 16, 10 * 16);

/// The first list row, `(box.x + 0x20, box.y + 0x6C)`.
pub const LIST: (i32, i32) = (BOX_X + 0x20, BOX_Y + 0x6C);
/// The column step, `x += 0x78`.
pub const COL_W: i32 = 0x78;
/// The row step — the loop's `y += 0x10`, and the interior's cell height.
pub const ROW_H: i32 = 0x10;
pub const COLS: usize = 3;
/// Ten rows: `Ui_DrawBoxInterior(…, 0x15, 10)`, and the loop's own
/// `if (0x1C < drawn) break` after thirty names.
pub const ROWS: usize = 10;
/// Thirty names on screen, and the scroll step is one row of three.
pub const PAGE: usize = COLS * ROWS;
pub const SCROLL_STEP: usize = COLS;

/// `Ui_DrawBoxInterior(box.x + 0x20, box.y + 0x118, 0x15, 1)` and the text two
/// pixels into it.
pub const STATUS: (i32, i32) = (BOX_X + 0x20, BOX_Y + 0x11A);

/// **`FUN_004B414A`'s width unit is sixteen pixels.** It writes
/// `g_spriteWidth` iterations of four dwords per row, and four dwords is
/// sixteen bytes.
/// body at `0x004B414A`. Getting this wrong is worth a factor of sixteen and it
/// was got wrong here once.
pub const HIGHLIGHT_CELL: i32 = 16;
/// `g_spriteWidth = 6` for the selected file's bar — 96 pixels, inside a
/// 120-pixel column.
pub const HIGHLIGHT_W: i32 = 6 * HIGHLIGHT_CELL;

/// The four `g_saveLoadWidgets` records, **box-relative** — see this module's
/// header. `(x, y, System.pl8 frame, side)`.
pub const CONFIRM: (i32, i32, usize, i32) = (BOX_X + 304, BOX_Y + 64, 29, 32);
pub const CANCEL: (i32, i32, usize, i32) = (BOX_X + 352, BOX_Y + 64, 31, 32);
pub const SCROLL_UP: (i32, i32, usize, i32) = (BOX_X + 384, BOX_Y + 144, 35, 24);
pub const SCROLL_DOWN: (i32, i32, usize, i32) = (BOX_X + 384, BOX_Y + 176, 37, 24);

pub(super) fn widget_rect(w: (i32, i32, usize, i32)) -> Rect {
    Rect::new(w.0, w.1, w.3, w.3)
}

/// **`SaveLoad_Tick`'s wait, in frames: `DAT_0057D3C4 = 0x96`.**
///
/// The thumb up's handler `FUN_004342F3` is one statement,
/// `DAT_005CD41C = 100`, and so is `Edit_Confirm`, Enter's. `SaveLoad_Tick`
/// (`0x004AD9F0`) sees the latch on its next call, clears it, sets
/// `DAT_0057D3C4 = 0x96`, copies the name, and only when that count has run
/// down to zero restores `g_screenIdSaved` and calls `Save_Write` or the
/// loader. While it runs, `SaveLoad_DrawStatus` prints group 40's *"Saving
/// game. Please wait."* `[V]`
///
/// **A press during the wait starts it again** — the latch is re-armed and the
/// tick resets the count to `0x96` — and the cross during it closes the box,
/// the count ends on a screen that is no longer `0x35` or `0x36` and nothing is
/// written. Both fall out of the same two lines here.
///
/// Frames, not milliseconds, and ours are 16 ms ticks: `crate::press` says why.
pub const WORK_FRAMES: u8 = 0x96;

/// What the status line is saying.
///
/// and that is the original's behaviour.
/// omission: group 40's status strings are *"Loading game. Please wait."*,
/// *"Saving game. Please wait."* and *"File error. Operation canceled."* — two
/// progress messages and a failure.
/// success `SaveLoad_Cancel`'s counterpart restores `g_screenIdSaved` and the
/// box is gone before anybody could read one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Nothing has happened yet, and the line is blank —
    /// original draws while `DAT_0057D3C4` is clear.
    Idle,
    /// The thumb up (or Enter) armed the latch and [`WORK_FRAMES`] are running.
    /// Group 40 index 2 or 3, *"Loading game. Please wait."* or *"Saving game.
    /// Please wait."*
    Working,
    /// It did not work. Group 40 index 4 in the game's font, and the detail —
    /// which is ours, since the original has no vocabulary for "this save was
    /// written by a newer build" — in ours.
    Failed(String),
}

