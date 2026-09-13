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

fn widget_rect(w: (i32, i32, usize, i32)) -> Rect {
    Rect::new(w.0, w.1, w.3, w.3)
}

/// **`g_saveLoadWidgets` as a table, and all four records are kind 4** —
/// `node tools/oracle/kinds.js` files `FUN_004342F3`, `SaveLoad_Cancel` and
/// `SaveLoad_Scroll` under `widget 4`, read out of `+0x0F` of `0x004DDD78` …
/// `0x004DDDC0`. `[V]`
///
/// So the thumb up **acts on the press**, clicks, and shows `base + 1` — and a
/// player who says it *"is still on mousedown instead of mouseup"* is feeling
/// something real that is not the gesture: the handler only arms a latch, and
/// the save happens 150 frames later ([`WORK_FRAMES`]). This screen used to
/// answer a raw click, so it did neither the click nor the wait.
///
/// Each `arm!` is the marker and the kind. The two scroll arrows are one
/// handler, `SaveLoad_Scroll`, told apart by the hotspot id, so they share one.
fn widgets() -> [Widget; 4] {
    const SCROLL: crate::press::Kind = crate::arm!("0x00434346/saveload-scroll", Repeat);
    [
        Widget::new(widget_rect(CONFIRM), crate::arm!("0x004342F3/saveload-confirm", Repeat)),
        Widget::new(widget_rect(CANCEL), crate::arm!("0x00434308/saveload-cancel", Repeat)),
        Widget::new(widget_rect(SCROLL_UP), SCROLL),
        Widget::new(widget_rect(SCROLL_DOWN), SCROLL),
    ]
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

pub struct SaveLoadScreen {
    mode: Mode,
    entries: Vec<Entry>,
    /// `g_fileListTop` (`0x004EA1A0`) — the index the visible page starts at.
    top: usize,
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
fn begin_name(seed: &str) -> crate::text::TextField {
    crate::text::TextField::begin(seed, saves::MAX_NAME, 0xA0, crate::text::Kind::Filename)
}

impl SaveLoadScreen {
    /// Reads the save directory once, on open. A screen that re-listed on every
    /// frame would be a screen that hits the disk sixty times a second.
    pub fn new(mode: Mode) -> SaveLoadScreen {
        let entries = saves::list();
        let mut screen = SaveLoadScreen {
            mode,
            entries,
            top: 0,
            selected: None,
            name: begin_name(""),
            status: Status::Idle,
            press: Press::new(),
            working: 0,
        };
        // Loading opens on the first file, because loading *is* choosing one.
        // Saving opens on none, because saving is naming one, and a preselected
        // row would mean the confirm button overwrote a game the player never
        // pointed at.
        if mode == Mode::Load {
            screen.select(0);
        }
        screen
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn status(&self) -> &Status {
        &self.status
    }

    /// What the name field holds — the typed name in save mode, the selected
    /// file's in load mode.
    pub fn name(&self) -> String {
        self.name.text()
    }

    /// The field itself, for a test that wants to look at the caret.
    pub fn name_field(&self) -> &crate::text::TextField {
        &self.name
    }

    /// Where row `i` of the visible page is drawn. The painter fills columns
/// **across** and then steps down, so this is `i % COLS` for the
    /// column and `i / COLS` for the row and not the other way round.
    pub fn row_rect(i: usize) -> Rect {
        let (col, row) = (i % COLS, i / COLS);
        let x = LIST.0 + col as i32 * COL_W;
        // The third column is **narrower than the other two**, and that is the
// painter's geometry: it steps x by 120
        // three times inside an interior that is only 336 wide, so the columns
        // start at 48, 168 and 288 and the box ends at 382. A hit box of a
        // uniform 120 would put the third column's right-hand 18 pixels
        // outside the list it belongs to.
        let right = (INTERIOR.0 + INTERIOR.2).min(x + COL_W - 8);
        Rect::new(x, LIST.1 + row as i32 * ROW_H, right - x, ROW_H)
    }

    /// The highest `top` that still shows a full page, in steps of three.
    fn max_top(&self) -> usize {
        let over = self.entries.len().saturating_sub(PAGE);
        // Round up to a whole scroll step so that the last press lands on a
// reachable value.
        over.div_ceil(SCROLL_STEP) * SCROLL_STEP
    }

    fn scroll(&mut self, by: i32) {
        let max = self.max_top() as i32;
        self.top = (self.top as i32 + by).clamp(0, max) as usize;
    }

    /// Which entry a click landed on, if any.
    fn at(&self, x: i32, y: i32) -> Option<usize> {
        (0..PAGE)
            .find(|&i| Self::row_rect(i).contains(x, y))
            .map(|i| self.top + i)
            .filter(|&i| i < self.entries.len())
    }

    /// Highlight a row and put its name in the field. In save mode that is how
    /// an existing save is overwritten — you pick it, and the name it had is
    /// what the confirm button will write to.
    fn select(&mut self, i: usize) {
        let Some(entry) = self.entries.get(i) else { return };
        self.selected = Some(i);
        self.name = begin_name(&entry.name);
    }

    /// **`DAT_005CD41C = 100`**, which is the whole of the thumb up's handler
    /// and of Enter's: arm the latch, and let [`WORK_FRAMES`] run.
    ///
/// **And the line the box speaks**, which is `SaveLoad_Tick`'s.
    /// the handler's: the tick that takes the latch up plays `S040_02.wav` on
    /// `g_screenId == '6'` — the save box — and `S040_01.wav` on anything else,
    /// which here is the load box. `[V]`, two `if`s and not an `if`/`else`.
    /// Ours collapses the arm and the take-up into this one call,
    /// is a frame earlier than the original's and on the same occasion.
    ///
    /// A screen cannot reach the audio layer (`docs/netcode.md` D-3), so the
    /// decision is made here and reported on [`crate::game::Game::spoken`].
    // sfx: FUN_004ad9f0#1,FUN_004ad9f0#2
    fn begin(&mut self, ctx: &mut Ctx) {
        self.working = WORK_FRAMES;
        self.status = Status::Working;
        let line = match self.mode {
            Mode::Save => crate::audio::names::speech::SAVE_GAME,
            Mode::Load => crate::audio::names::speech::LOAD_GAME,
        };
        ctx.game.spoken = (ctx.game.spoken.0.wrapping_add(1), line);
    }

    /// Frames left before the load or the save runs; 0 when nothing is armed.
    pub fn working(&self) -> u8 {
        self.working
    }

    /// One `g_saveLoadWidgets` record's handler. The index is [`widgets`]'.
    fn fire(&mut self, ctx: &mut Ctx, widget: usize) -> Transition {
        match widget {
            // `FUN_004342F3`.
            0 => {
                self.begin(ctx);
                Transition::Stay
            }
            // `SaveLoad_Cancel` (`0x00434308`): `g_screenId = g_screenIdSaved`.
            1 => Transition::Pop,
            // `SaveLoad_Scroll`, hotspot id −3 and +3.
            2 => {
                self.scroll(-(SCROLL_STEP as i32));
                Transition::Stay
            }
            _ => {
                self.scroll(SCROLL_STEP as i32);
                Transition::Stay
            }
        }
    }

    /// The load or the save itself — what `SaveLoad_Tick` does when
    /// `DAT_0057D3C4` reaches zero. Everything that can go wrong comes back as
    /// a [`Status`] and the screen stays open; only success closes it.
    fn confirm(&mut self, ctx: &mut Ctx) -> Transition {
        match self.mode {
            Mode::Load => {
                // **The path comes from the edit buffer, not from the
                // highlighted row.** `SaveLoad_Tick` (`0x004AD9F0`) is
                // `Str_Copy(0x4EA130, 0x4EAD60, 0xC); Path_AddExtension(…)` —
                // twelve bytes of the *typed* name — and clicking a row is what
                // puts a name into the buffer. So one road, and the mouse joins
                // it upstream. Ours read `selected` and ignored what was typed.
                let name = self.name.text().trim().to_string();
                let Some(i) = self.entries.iter().position(|e| e.name == name) else {
                    self.status = Status::Failed(if name.is_empty() {
                        "NO SAVED GAME IS SELECTED".into()
                    } else {
                        format!("{name:?} IS NOT A SAVED GAME")
                    });
                    return Transition::Stay;
                };
                let tables = ctx.game.kingdom.tables;
                match saves::read_path(&self.entries[i].path, tables) {
                    // **The whole game is replaced or none of it is.** `decode`
                    // builds a complete `Game` before this line runs,
                    // that turns out to be unreadable halfway through cannot
                    // leave the player holding half of one.
                    Ok(game) => {
                        // Nor does a load clear `g_tipShown`. `crate::tip`.
                        let tips = ctx.game.tips;
                        *ctx.game = game;
                        ctx.game.tips = tips;
                        Transition::Pop
                    }
                    Err(e) => {
                        self.status = Status::Failed(e.to_string().to_uppercase());
                        Transition::Stay
                    }
                }
            }
            Mode::Save => {
                // **OURS, and it is a refusal the original does not make.**
                //
                // `Menu_SaveGame` (`0x00433F49`) is reachable from the
                // battlefield — `Screen_FrameInput`'s `0x29` arm opens with
                // `Menu_OpenDropdown` and neither the opener nor the handler
                // tests `g_battlePhase` — and the original's `.sav` is a memory
                // dump, so the battle goes into it. Ours is a versioned format
                // that does not encode `crate::battlefield::LiveBattle`, and
                // `crate::save::decode` puts `battle: None` back.
                //
                // So a mid-battle save would write a file that quietly lost the
                // battle the player was fighting. It is refused instead, through
                // `Status::Failed`, whose first line is the game's **own**
                // sentence — `Eng_DrawString(40, ERROR_INDEX)` —
// sees a refusal.
                //
                // **Measured, and that is why it is still a refusal.** The
                // state a mid-battle save would have to carry is
                // `LiveBattle` -> `BattleRunner` -> `Battle`, `Battlefield`,
                // `Vec<Fighter>`, `Units`, `Ai`, `AiField`, `Missiles`,
                // `SiegeState`: **196 fields over 16 structs, 21 of them
                // private to `l2-sim`**. The kingdom encoder next door spends
                // 1975 lines on 242 stored fields, so this is four figures of
                // encoder, not the ~150 lines the question was worth.
                //
                // arm: ours/save-refuses-mid-battle left-press
                if ctx.game.battle.is_some() {
                    self.status = Status::Failed(BATTLE_REFUSAL.into());
                    return Transition::Stay;
                }
                let name = self.name.text().trim().to_string();
                if !saves::is_valid_name(&name) {
                    self.status = Status::Failed(format!("{name:?} IS NOT A SAVE NAME"));
                    return Transition::Stay;
                }
                match saves::write(&name, ctx.game) {
                    // The box closes, the way `Menu_SaveGame`'s does: it saved
                    // `g_screenId` into `g_screenIdSaved` on the way in and the
                    // screen puts it back on the way out, which is what a `Pop`
                    // over whatever pushed this is.
                    Ok(_) => Transition::Pop,
                    Err(e) => {
                        self.status = Status::Failed(e.to_string().to_uppercase());
                        Transition::Stay
                    }
                }
            }
        }
    }

    /// **One event into the name field**, and it is live on **both** screens.
    ///
    /// `Screen_HandleInput`'s arm is `else if (g_screenId == '5' || g_screenId
    /// == '6')` — one arm for `0x35` and `0x36` together — so the original lets
    /// a person **type the name of the game they want to load**, and
    /// `SaveLoad_Tick` builds the path out of the edit buffer either way rather
    /// than out of the highlighted row. Ours refused every keystroke unless
    /// `mode == Save`, which was a restriction we invented; clicking a row
    /// still fills the field, so the mouse route is unchanged.
    fn edit(&mut self, event: Event, ctx: &Ctx) -> bool {
        // arm: 0x004BA9C8/saveload-name key
        let m = crate::text::FontMetrics::of(&ctx.assets.shell);
        if !self.name.event(event, &m) {
            return false;
        }
        self.status = Status::Idle;
        true
    }
}

impl Screen for SaveLoadScreen {
    fn id(&self) -> ScreenId {
        ScreenId::SaveLoad(self.mode)
    }

    fn title(&self, _ctx: &Ctx) -> String {
        let what = match self.mode {
            Mode::Load => "Load a conquest",
            Mode::Save => "Save a conquest",
        };
        format!("{what} — screen 0x{:02X}", self.mode.screen_id())
    }

    /// A `Ui_DrawBox` window over whatever opened it. Same as the four county
    /// panels: `Screen_SaveLoad` clears nothing.
    /// The caret's blink, and nothing else. `Edit_DrawCaret` counts frames of
    /// its own; `docs/netcode.md` does not let anything below the renderer read
    /// a clock, so it is a tick here. See [`crate::text`].
    ///
    /// **And `Widget_Test`'s countdown and `SaveLoad_Tick`'s**, in that order:
    /// the scroll arrows repeat while held, and the latch the thumb up armed
    /// runs down to the load or the save.
    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        self.name.tick();
        for widget in self.press.tick() {
            let t = self.fire(ctx, widget);
            if t != Transition::Stay {
                return t;
            }
        }
        if self.working > 0 {
            self.working -= 1;
            if self.working == 0 {
                return self.confirm(ctx);
            }
        }
        Transition::Stay
    }

    /// `Widget_Test`'s `Sound_RestartSlot(1)`, carried up to the audio layer.
    fn take_clicks(&mut self) -> u8 {
        self.press.take_clicks()
    }

    /// A held arrow scrolled the list: `SaveLoad_Scroll` sets
    /// `g_redrawRequest = 2`. See [`Press::take_redraw`].
    fn take_redraw(&mut self) -> bool {
        self.press.take_redraw()
    }

    fn is_overlay(&self) -> bool {
        true
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        // **The field first, on both screens.** See [`SaveLoadScreen::edit`].
        // It takes `WM_CHAR` and the six editing keys and nothing else,
        // Escape, Enter and the four navigation arrows below still arrive —
        // except Left and Right, which the original spends on the caret here
        // and which this screen was spending on the file list. The list keeps
        // Up and Down, which the original spends on nothing at all.
        if self.edit(event, ctx) {
            return Transition::Stay;
        }
        match event {
            // Ours. `SaveLoad_Cancel` (`0x00434308`) is the cross widget and
            // nothing reaches it from the keyboard; the window procedure's
            // Escape arm is `Menu_Quit` or `g_backOut = 1`, neither of which
            // knows this box exists.
            //
            // arm: ours/saveload-key-escape key
            Event::KeyDown(Key::Escape) => Transition::Pop,
            // **Enter is the confirm button, and that is the original's.**
            // `VK_RETURN` runs `Edit_Confirm` (`0x00401C5B`), whose whole body
            // is `g_saveLoadConfirm = 100` — the identical assignment the
            // confirm widget's handler `FUN_004342F3` makes. `SaveLoad_Tick`
            // reads that latch and does the load or the save. This was a
            // convenience of ours until the keyboard path was read; it turns
            // out to be an arm.
            //
            // arm: 0x00401C5B/enter-confirms key
            Event::KeyDown(Key::Enter) => {
                self.begin(ctx);
                Transition::Stay
            }
            // **Space no longer confirms, and could not**: the field takes it
            // above as a character, on both screens, which is what the original
            // does — a space is a legal character in a name and `VK_SPACE` has
            // no `WM_KEYDOWN` arm at all. It used to confirm here, and with the
            // field live it would have done both.
            //
            // Ours. The original scrolls the list from the two arrow *widgets*
            // and from nothing else.
            //
            // arm: ours/saveload-key-scroll key
            Event::KeyDown(Key::Up) => {
                self.scroll(-(SCROLL_STEP as i32));
                Transition::Stay
            }
            Event::KeyDown(Key::Down) => {
                self.scroll(SCROLL_STEP as i32);
                Transition::Stay
            }
            // **Left and Right walked the file list here and no longer do.**
            // They are `Edit_Left` and `Edit_Right` in the original — caret
            // keys, taken by the field above — and a screen that spent them on
            // a list would leave a person unable to move the caret in the one
            // place the game has a caret. Clicking a row still selects it.
            // **The four widgets, through the hit test that plays the click.**
            // Kind 4: each acts on the press, and a double click is a press.
            // The arms are declared on [`widgets`].
            Event::Click { x, y } | Event::DoubleClick { x, y } => {
                if let Some(i) = self.press.event(&widgets(), event) {
                    return self.fire(ctx, i);
                }
                if let (Event::Click { .. }, Some(i)) = (event, self.at(x, y)) {
                    self.select(i);
                    self.status = Status::Idle;
                }
                Transition::Stay
            }
            Event::Release { .. } | Event::Pointer { .. } | Event::PointerLeft => {
                let fired = self.press.event(&widgets(), event);
                debug_assert!(fired.is_none(), "no save/load widget is a release widget");
                Transition::Stay
            }
            _ => Transition::Stay,
        }
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let a = &ctx.assets.shell;
        let ink = &ctx.assets.ink;
        let pen = Pen {
            assets: a,
            ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: None,
        };

        pen.window(canvas, BOX_X, BOX_Y, BOX_COLS, BOX_ROWS, 0);
        let heading = a.text(GROUP, self.mode.heading_index()).to_string();
        pen.heading(canvas, HEADING.0, HEADING.1, &heading, font::TEXT);

        // `FUN_00403CF4(x, y, w, h, 0x3F)`, four times — flat outlines in one
        // colour, not the two-tone `Ui_DrawInsetRect` this module used to draw.
        for (x, y, w, h) in INSETS {
            rect_outline(canvas, x, y, w, h, OUTLINE);
        }

// **The name field, with the original's caret
        // underscore.**
        //
        // The underscore was a stand-in and it was wrong twice over: it was
        // drawn in save mode only, when the original's edit arm covers both
        // screens; and `Ui_DrawText` maps `0x5F` to a space
        // (`if (ch == 0x5F) ch = 0x20;`),
        // it drew **nothing at all** — a blank where the caret should be. The
        // caret is `Edit_DrawCaret` (`0x0040ACCE`) now: it blinks, it sits at
// the caret, and it changes shape with insert
        // mode.
        pen.body(canvas, NAME.0, NAME.1, &self.name.text(), font::TEXT);
        let caret_colour = if a.body.is_some() { font::TEXT } else { pen.ink.text };
        self.name.draw_caret(canvas, NAME.0, NAME.1, caret_colour, &crate::text::FontMetrics::of(a));

        // The list: three columns, ten rows, thirty names.
        for i in 0..PAGE {
            let Some(entry) = self.entries.get(self.top + i) else { break };
            let r = Self::row_rect(i);
            if self.selected == Some(self.top + i) {
                // `g_spriteWidth = 6; g_spriteHeight = 0x10; FUN_004B414A(x - 2,
                // y - 1, 0x3F)`, then the row's own text in colour 0x20.
                //
                // **`g_spriteWidth` is in units of sixteen pixels, not pixels.**
                // `FUN_004B414A` writes `g_spriteWidth` iterations of four
                // dwords per row — sixteen bytes each — so 6 is a **96-pixel**
                // bar, not a six-pixel one, and it runs *behind* the name
// This module drew a
                // six-pixel tick until the primitive was read. See
                // [`HIGHLIGHT_CELL`].
                canvas.fill_rect(r.x - 2, r.y - 1, HIGHLIGHT_W, ROW_H, ink.highlight);
                pen.body(canvas, r.x, r.y, &entry.name, font::DISABLED);
            } else {
                pen.body(canvas, r.x, r.y, &entry.name, font::TEXT);
            }
        }

        // The status line. Group 40's own words where the game has words for
        // it; ours, in our font, where it does not.
        match &self.status {
            Status::Idle => {}
            // `SaveLoad_DrawStatus`: `Eng_DrawString(40, 2 | 3, …)` while
            // `DAT_0057D3C4` runs.
            Status::Working => {
                pen.eng(canvas, GROUP, self.mode.working_index(), STATUS.0, STATUS.1, font::TEXT);
            }
            Status::Failed(detail) => {
                pen.eng(canvas, GROUP, ERROR_INDEX, STATUS.0, STATUS.1, font::TEXT);
                // Our detail under the original's sentence: debug overlay only.
                if ctx.game.prefs.debug_overlay {
                    text::draw(canvas, STATUS.0, STATUS.1 + 18, &ours(detail), ink.bad);
                }
            }
        }

        // `Widget_Draw(0x10, 0x90, &g_saveLoadWidgets, 4)`, with `base + 1`
        // while a record's press timer runs.
        for (i, w) in [CONFIRM, CANCEL, SCROLL_UP, SCROLL_DOWN].into_iter().enumerate() {
            let frame = w.2 + usize::from(self.press.is_pressed(i));
            let drawn = ctx
                .assets
                .chrome
                .as_ref()
                .is_some_and(|c| c.draw_system(canvas, frame, w.0, w.1));
            if !drawn {
                shell::button_recess(canvas, w.0, w.1, w.3, w.3);
            }
        }

        // **Ours, and it says so.** The directory these files live in is not
        // something the original has an opinion about, so it is drawn in our
        // own 5 x 7 font in the dim colour — never in `Fntl2_14.pl8`.
        let where_ = match saves::dir() {
            Some(d) => d.display().to_string().to_uppercase(),
            None => "NO SAVE DIRECTORY ON THIS MACHINE".into(),
        };
        if ctx.game.prefs.debug_overlay {
            text::draw(canvas, DIR_LINE.0, DIR_LINE.1, &directory_line(&where_), ink.dim);
        }
    }
}

/// Where the save directory is written: inside the box's bottom edge.
const DIR_LINE: (i32, i32) = (BOX_X + 4, BOX_Y + BOX_ROWS * 16 - 12);

/// Everything we put on this screen that the original does not say is prefixed,
///
fn ours(detail: &str) -> String {
    format!("OURS: {detail}")
}

/// The directory line, **cut from the left** when it is wider than the box.
///
/// Ours, like the line itself — the original has no directory to show. It was
/// drawn whole, and a directory longer than about 67 characters ran straight
/// out of the side of the box: a `LORDS2_SAVES` on a deep path, or the default
/// under a profile whose user name is longer than 24. Nobody saw it because
/// the only test of the box's edges ran with a short temporary directory, and
/// it went red the day that directory got longer. The left is what goes
/// because the right is the part that says which directory this is.
fn directory_line(dir: &str) -> String {
    let room = BOX_X + BOX_COLS * 16 - 4 - DIR_LINE.0;
    let whole = ours(dir);
    if text::width(&whole) <= room {
        return whole;
    }
    let chars: Vec<char> = dir.chars().collect();
    let fixed = ours("...").chars().count() as i32;
    let keep = ((room + 1) / text::ADVANCE - fixed).max(0) as usize;
    let tail: String = chars[chars.len().saturating_sub(keep)..].iter().collect();
    ours(&format!("...{tail}"))
}

/// **`FUN_00403CF4(x, y, w, h, colour)`** — four `FUN_00403A8F` lines, **all in
/// the caller's one colour**,
/// body:
///
/// ```c
/// FUN_00403A8F(x,         y,         x + w - 1, y,         c);   /* top    */
/// FUN_00403A8F(x,         y + h - 1, x + w - 1, y + h - 1, c);   /* bottom */
/// FUN_00403A8F(x,         y,         x,         y + h - 1, c);   /* left   */
/// FUN_00403A8F(x + w - 1, y,         x + w - 1, y + h - 1, c);   /* right  */
/// ```
///
/// It is **not** `Ui_DrawInsetRect` (`0x00403DEB`), which takes four arguments
/// and lights `0x10` / `0x1F` on opposite corners; `shell::inset_rect` is that
/// one and three other screens use it. This module drew the save box's four
/// rectangles through it until the painter's call was read
/// name.
pub fn rect_outline(canvas: &mut Canvas, x: i32, y: i32, w: i32, h: i32, colour: u8) {
    canvas.fill_rect(x, y, w, 1, colour);
    canvas.fill_rect(x, y + h - 1, w, 1, colour);
    canvas.fill_rect(x, y, 1, h, colour);
    canvas.fill_rect(x + w - 1, y, 1, h, colour);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_screens_are_one_painter_with_a_mode_flag() {
        assert_eq!(Mode::Load.screen_id(), 0x35);
        assert_eq!(Mode::Save.screen_id(), 0x36);
        // The painter's argument *is* the string index: `Eng_DrawString(40,
        // saving, ...)`.
        assert_eq!(Mode::Load.heading_index(), 0);
        assert_eq!(Mode::Save.heading_index(), 1);
        assert_eq!(Mode::Load.working_index(), 2);
        assert_eq!(Mode::Save.working_index(), 3);
    }

    #[test]
    fn the_thirty_rows_fill_the_rectangle_the_painter_reserves_for_them() {
        // `Ui_DrawBoxInterior(box.x + 0x1E, box.y + 0x6A, 0x15, 10)` — 21 × 10
        // cells at (46, 250), so 336 × 160 covering x 46 … 381, y 250 … 409.
        let interior = Rect::new(INTERIOR.0, INTERIOR.1, INTERIOR.2, INTERIOR.3);
        let first = SaveLoadScreen::row_rect(0);
        let last = SaveLoadScreen::row_rect(PAGE - 1);
        assert_eq!(first.x - interior.x, 2, "the first name is two pixels into the interior");
        assert_eq!(first.y - interior.y, 2);
        assert_eq!(
            (last.y - first.y) / ROW_H,
            ROWS as i32 - 1,
            "ten rows, and the last one is the tenth"
        );
        // The tenth row's *text* sits inside; its 16-pixel step overhangs the
        // interior's last two pixels, which is the painter's own arithmetic —
        // the box starts at 250 and the first baseline at 252 — and not a slip
        // here. The body font is 14 pixels tall.
        assert!(last.y + 14 <= interior.y + interior.h, "the tenth name at {} spills", last.y);
        for i in 0..PAGE {
            let r = SaveLoadScreen::row_rect(i);
            assert!(r.x >= interior.x, "row {i} starts left of the list: {}", r.x);
            assert!(r.x + r.w <= interior.x + interior.w, "row {i} runs past the list");
        }
        assert_eq!(PAGE, 30, "the loop breaks after thirty names");
    }

    #[test]
    fn the_rows_run_across_before_they_run_down() {
        // The painter steps x by 0x78 twice and only then resets and steps y.
        assert_eq!(SaveLoadScreen::row_rect(0).y, SaveLoadScreen::row_rect(2).y);
        assert_eq!(SaveLoadScreen::row_rect(1).x - SaveLoadScreen::row_rect(0).x, COL_W);
        assert_eq!(SaveLoadScreen::row_rect(3).x, SaveLoadScreen::row_rect(0).x);
        assert_eq!(SaveLoadScreen::row_rect(3).y - SaveLoadScreen::row_rect(0).y, ROW_H);
    }

    #[test]
    fn every_rectangle_this_screen_draws_is_inside_the_box() {
        let box_r = Rect::new(BOX_X, BOX_Y, BOX_COLS * 16, BOX_ROWS * 16);
        for (x, y, w, h) in INSETS {
            assert!(x >= box_r.x && y >= box_r.y, "inset ({x}, {y}) is outside the window");
            assert!(x + w <= box_r.x + box_r.w, "inset ({x}, {y}) is {} wide", x + w);
            assert!(y + h <= box_r.y + box_r.h, "inset ({x}, {y}) is {} tall", y + h);
        }
        // The widgets, on the reading this module argues for. Absolutely they
        // would be at y = 64 and y = 144, above this window entirely, which is
        // the argument.
        for w in [CONFIRM, CANCEL, SCROLL_UP, SCROLL_DOWN] {
            let r = widget_rect(w);
            assert!(
                r.x >= box_r.x
                    && r.y >= box_r.y
                    && r.x + r.w <= box_r.x + box_r.w
                    && r.y + r.h <= box_r.y + box_r.h,
                "widget at ({}, {}) is outside the box it belongs to",
                r.x,
                r.y
            );
        }
    }

    #[test]
    fn the_confirm_and_cancel_buttons_do_not_overlap_anything_clickable() {
        for w in [CONFIRM, CANCEL, SCROLL_UP, SCROLL_DOWN] {
            let r = widget_rect(w);
            for i in 0..PAGE {
                let row = SaveLoadScreen::row_rect(i);
                let overlaps = r.x < row.x + row.w
                    && row.x < r.x + r.w
                    && r.y < row.y + row.h
                    && row.y < r.y + r.h;
                assert!(!overlaps, "widget ({}, {}) sits on list row {i}", r.x, r.y);
            }
        }
    }

    #[test]
    fn scrolling_stops_at_both_ends() {
        let mut s = SaveLoadScreen::new(Mode::Load);
        s.entries = (0..40)
            .map(|i| Entry {
                name: format!("save{i:02}"),
                path: std::path::PathBuf::from(format!("save{i:02}.l2sav")),
                bytes: 0,
            })
            .collect();
        s.scroll(-9);
        assert_eq!(s.top, 0, "a list cannot scroll above its first row");
        for _ in 0..20 {
            s.scroll(SCROLL_STEP as i32);
        }
        assert_eq!(s.top, s.max_top());
        assert!(s.top + PAGE >= s.entries.len(), "the last name must be reachable");
        assert_eq!(s.max_top() % SCROLL_STEP, 0, "the top is always a whole row of three");

        // A list that fits on the page does not scroll at all.
        s.entries.truncate(PAGE);
        s.top = 0;
        s.scroll(SCROLL_STEP as i32);
        assert_eq!(s.top, 0);
    }
}
