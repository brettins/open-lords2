//! `g_screenId` **`0x35` and `0x36`** — loading and saving a conquest.
//!
//! These were two rows of [`crate::screens::shells::SHELLS`] until now: the
//! original's window, the original's heading, and a click that closed them
//! again. They are screens now, and every coordinate below was read out of the
//! painter rather than chosen.
//!
//! # The painter, and what it actually draws
//!
//! `Screen_SaveLoad(saving)` (`0x00414819`) is **one painter with a mode flag**
//! — the two `g_screenId` values differ only in which of `L2.eng` group 40's
//! first two strings is used as the heading, *"Loading a conquest."* or
//! *"Saving a conquest."* Its whole body is:
//!
//! ```c
//! Ui_DrawBox(0x10, 0x90, 0x1C, 0x14);
//! Eng_DrawString(40, saving, 0x20, 0xA0, &g_fontHeading, 0x3F);
//! Ui_DrawInsetRect(0x20, 200,   400,   0x100);   // the whole lower area
//! Ui_DrawInsetRect(0x28, 0xD0,  0xC0,  0x20);    // the name field
//! Ui_DrawInsetRect(0x28, 0xF8,  0x160, 0xA4);    // the file list
//! Ui_DrawInsetRect(0x28, 0x1A4, 0x180, 0x1C);    // the status line
//! SaveLoad_DrawStatus();
//! ```
//!
//! and `SaveLoad_DrawStatus` (`0x004149EC`) fills those rectangles. **[V]**,
//! from the two functions' own bodies. The list is the interesting part:
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
//! the front end's page 3 at `(0x60, 0x0A)` rather than `(0x10, 0x90)`, and one
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
//! clamps at `count - 30`, the number actually on screen. Reproducing an
//! off-by-fifteen in a list of our own files would be superstition, not
//! fidelity.

use l2_view::{text, Canvas};

use crate::input::{Event, Key, Rect};
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

/// `Ui_DrawBox(0x10, 0x90, 0x1C, 0x14)`.
pub const BOX_X: i32 = 0x10;
pub const BOX_Y: i32 = 0x90;
pub const BOX_COLS: i32 = 0x1C;
pub const BOX_ROWS: i32 = 0x14;

/// `Eng_DrawString(40, saving, 0x20, 0xA0, &g_fontHeading, 0x3F)`.
pub const HEADING: (i32, i32) = (0x20, 0xA0);

/// The four `Ui_DrawInsetRect(x, y, w, h)` calls, in the painter's order.
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

/// The four `g_saveLoadWidgets` records, **box-relative** — see this module's
/// header. `(x, y, System.pl8 frame, side)`.
pub const CONFIRM: (i32, i32, usize, i32) = (BOX_X + 304, BOX_Y + 64, 29, 32);
pub const CANCEL: (i32, i32, usize, i32) = (BOX_X + 352, BOX_Y + 64, 31, 32);
pub const SCROLL_UP: (i32, i32, usize, i32) = (BOX_X + 384, BOX_Y + 144, 35, 24);
pub const SCROLL_DOWN: (i32, i32, usize, i32) = (BOX_X + 384, BOX_Y + 176, 37, 24);

fn widget_rect(w: (i32, i32, usize, i32)) -> Rect {
    Rect::new(w.0, w.1, w.3, w.3)
}

/// What the status line is saying.
///
/// **There is no `Done`,** and that is the original's behaviour rather than an
/// omission: group 40's status strings are *"Loading game. Please wait."*,
/// *"Saving game. Please wait."* and *"File error. Operation canceled."* — two
/// progress messages and a failure. Success is not a message, because on
/// success `SaveLoad_Cancel`'s counterpart restores `g_screenIdSaved` and the
/// box is gone before anybody could read one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Nothing has happened yet, and the line is blank — which is also what the
    /// original draws while `DAT_0057D3C4` is clear.
    Idle,
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
    /// The edit buffer the name field shows — `DAT_004EA130`.
    name: String,
    status: Status,
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
            name: String::new(),
            status: Status::Idle,
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
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Where row `i` of the visible page is drawn. The painter fills columns
    /// **across** and then steps down, which is why this is `i % COLS` for the
    /// column and `i / COLS` for the row and not the other way round.
    pub fn row_rect(i: usize) -> Rect {
        let (col, row) = (i % COLS, i / COLS);
        let x = LIST.0 + col as i32 * COL_W;
        // The third column is **narrower than the other two**, and that is the
        // painter's geometry rather than a rounding choice: it steps x by 120
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
        // reachable value rather than one press short of the end.
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
        self.name = entry.name.clone();
    }

    /// The confirm button - `g_saveLoadWidgets` frame 29, a mailed hand with
    /// its thumb up rather than a tick. Everything that can go wrong comes
    /// back as a [`Status`] and
    /// the screen stays open; only success closes it.
    fn confirm(&mut self, ctx: &mut Ctx) -> Transition {
        match self.mode {
            Mode::Load => {
                let Some(i) = self.selected.filter(|&i| i < self.entries.len()) else {
                    self.status = Status::Failed("NO SAVED GAME IS SELECTED".into());
                    return Transition::Stay;
                };
                let tables = ctx.game.kingdom.tables;
                match saves::read_path(&self.entries[i].path, tables) {
                    // **The whole game is replaced or none of it is.** `decode`
                    // builds a complete `Game` before this line runs, so a save
                    // that turns out to be unreadable halfway through cannot
                    // leave the player holding half of one.
                    Ok(game) => {
                        *ctx.game = game;
                        Transition::Pop
                    }
                    Err(e) => {
                        self.status = Status::Failed(e.to_string().to_uppercase());
                        Transition::Stay
                    }
                }
            }
            Mode::Save => {
                let name = self.name.trim().to_string();
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

    /// `MAX_NAME` is counted in **bytes**, because that is what
    /// [`saves::is_valid_name`] checks and what the original's 65-byte list
    /// records hold. A field that let a name past its own validator would be a
    /// field whose confirm always fails.
    fn type_char(&mut self, c: char) {
        if self.mode != Mode::Save || self.name.len() + c.len_utf8() > saves::MAX_NAME {
            return;
        }
        self.name.push(c);
        self.status = Status::Idle;
    }

    fn backspace(&mut self) {
        if self.mode == Mode::Save {
            self.name.pop();
            self.status = Status::Idle;
        }
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
    fn is_overlay(&self) -> bool {
        true
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        match event {
            Event::KeyDown(Key::Escape) => Transition::Pop,
            Event::KeyDown(Key::Enter) => self.confirm(ctx),
            Event::KeyDown(Key::Backspace) => {
                self.backspace();
                Transition::Stay
            }
            // Space is a character in a save name, not a shortcut. Every other
            // screen in this crate treats it as "confirm"; a text field cannot.
            Event::KeyDown(Key::Space) if self.mode == Mode::Save => {
                self.type_char(' ');
                Transition::Stay
            }
            Event::KeyDown(Key::Space) => self.confirm(ctx),
            Event::KeyDown(Key::Char(c)) if self.mode == Mode::Save => {
                self.type_char(c);
                Transition::Stay
            }
            Event::KeyDown(Key::Up) => {
                self.scroll(-(SCROLL_STEP as i32));
                Transition::Stay
            }
            Event::KeyDown(Key::Down) => {
                self.scroll(SCROLL_STEP as i32);
                Transition::Stay
            }
            Event::KeyDown(Key::Left) | Event::KeyDown(Key::Right) => {
                let by = if event == Event::KeyDown(Key::Left) { -1 } else { 1 };
                // `clamp` panics when its bounds cross, which is what an empty
                // list would do here — the reason this is a `checked` walk and
                // not one.
                if let (Some(i), false) = (self.selected, self.entries.is_empty()) {
                    let last = self.entries.len() as i32 - 1;
                    self.select((i as i32 + by).clamp(0, last) as usize);
                }
                Transition::Stay
            }
            Event::Click { x, y } => {
                if widget_rect(CANCEL).contains(x, y) {
                    return Transition::Pop;
                }
                if widget_rect(CONFIRM).contains(x, y) {
                    return self.confirm(ctx);
                }
                if widget_rect(SCROLL_UP).contains(x, y) {
                    self.scroll(-(SCROLL_STEP as i32));
                    return Transition::Stay;
                }
                if widget_rect(SCROLL_DOWN).contains(x, y) {
                    self.scroll(SCROLL_STEP as i32);
                    return Transition::Stay;
                }
                if let Some(i) = self.at(x, y) {
                    self.select(i);
                    self.status = Status::Idle;
                }
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

        for (x, y, w, h) in INSETS {
            inset_rect(canvas, x, y, w, h);
        }

        // The name field. In save mode it is an edit buffer with a caret; in
        // load mode it shows what the highlighted row is called, which is what
        // `DAT_004EA130` holds there too.
        let shown = match self.mode {
            Mode::Save => format!("{}_", self.name),
            Mode::Load => self.name.clone(),
        };
        pen.body(canvas, NAME.0, NAME.1, &shown, font::TEXT);

        // The list: three columns, ten rows, thirty names.
        for i in 0..PAGE {
            let Some(entry) = self.entries.get(self.top + i) else { break };
            let r = Self::row_rect(i);
            if self.selected == Some(self.top + i) {
                // `g_spriteWidth = 6; g_spriteHeight = 0x10; FUN_004B414A(x - 2,
                // y - 1, 0x3F)` — a 6 x 16 mark to the left of the row, and the
                // row itself in colour 0x20 rather than 0x3F.
                canvas.fill_rect(r.x - 2, r.y - 1, 6, ROW_H, ink.highlight);
                pen.body(canvas, r.x, r.y, &entry.name, font::DISABLED);
            } else {
                pen.body(canvas, r.x, r.y, &entry.name, font::TEXT);
            }
        }

        // The status line. Group 40's own words where the game has words for
        // it; ours, in our font, where it does not.
        match &self.status {
            Status::Idle => {}
            Status::Failed(detail) => {
                pen.eng(canvas, GROUP, ERROR_INDEX, STATUS.0, STATUS.1, font::TEXT);
                text::draw(canvas, STATUS.0, STATUS.1 + 18, &ours(detail), ink.bad);
            }
        }

        for w in [CONFIRM, CANCEL, SCROLL_UP, SCROLL_DOWN] {
            let drawn = ctx
                .assets
                .chrome
                .as_ref()
                .is_some_and(|c| c.draw_system(canvas, w.2, w.0, w.1));
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
        text::draw(canvas, BOX_X + 4, BOX_Y + BOX_ROWS * 16 - 12, &ours(&where_), ink.dim);
    }
}

/// Everything we put on this screen that the original does not say is prefixed,
/// so a screenshot cannot be mistaken for the game's own wording.
fn ours(detail: &str) -> String {
    format!("OURS: {detail}")
}

/// `Ui_DrawInsetRect(x, y, w, h)` (`0x00403DEB`): **[V]** colour `0x10` along
/// the top and right edges and `0x1F` along the bottom and left — the opposite
/// lighting to `shell::button_recess`, which is why one reads as recessed and
/// the other as raised.
pub fn inset_rect(canvas: &mut Canvas, x: i32, y: i32, w: i32, h: i32) {
    const DARK: u8 = 0x10;
    const LIGHT: u8 = 0x1F;
    canvas.fill_rect(x, y, w, 1, DARK);
    canvas.fill_rect(x + w - 1, y, 1, h, DARK);
    canvas.fill_rect(x, y + h - 1, w, 1, LIGHT);
    canvas.fill_rect(x, y, 1, h, LIGHT);
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
