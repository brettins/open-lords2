//! There is exactly **one** edit buffer in `Lords2.exe` — `g_editBuffer`
//! (`0x005CD550`), 2,000 bytes — with a caret (`0x005BB4A8`), a length
//! (`0x0058FE94`), a character limit (`0x005CD4F4`), a *pixel* limit
//! (`0x0058FD68`) and a kind (`0x005C928C`). A screen does not own a field; it
//! **seeds** the buffer on entry and **copies it out** every frame:
//!
//! | function | what it is | here |
//! |---|---|---|
//! | `Edit_Begin` `0x00402009` | clear, seed from a string, set the two limits and the kind, caret to 0 | [`TextField::begin`]
//! | `Edit_TypeChar` `0x00401A20` | **the character filter** | [`TextField::type_char`]
//! | `Edit_Insert` `0x00401D26` | put one accepted character at the caret, insert or overwrite | [`TextField::put`] |
//! | `Edit_Clamp` `0x00401984` | recompute the pixel width, clamp the caret, decide whether the field is full | [`TextField::clamp`] |
//! | `Edit_Backspace` `0x00401C70` | caret back one, then delete | [`TextField::backspace`] |
//! | `Edit_Delete` `0x00401DC8` | shift the tail left over the caret | [`TextField::delete`] |
//! | `Edit_Home` `0x00401CFC` / `Edit_End` `0x00401D11` | caret to 0 / to the length | [`TextField::home`], [`TextField::end`] |
//! | `Edit_Left` `0x00401CBC` / `Edit_Right` `0x00401CDA` | caret by one | [`TextField::left`], [`TextField::right`] |
//! | `Edit_ToggleInsert` `0x00401CA3` | `insert ^= 1` | [`TextField::toggle_insert`] |
//! | `Edit_Commit` `0x0040210C` | copy the buffer out to a destination, truncated | [`TextField::commit`] |
//! | `Edit_DrawCaret` `0x0040ACCE` | the blinking caret, two shapes | [`TextField::draw_caret`] |
//!
//! **The keyboard dispatch is the window procedure** (`0x004B29BE`), not
//! `Screen_FrameInput` and not `Screen_HandleInput`. `WM_CHAR` (`0x102`) goes
//! straight to `Edit_TypeChar` with no screen test whatever
//! editing keys are arms of `WM_KEYDOWN` (`0x100`) that are equally ungated.
//!
//! What decides whether any of it *lands* is a single flag, `g_editActive`
//! (`0x005AEB78`): `Screen_HandleInput` clears it at the top of every frame and
//! exactly seven of its arms set it again. Those seven arms are the whole
//! inventory of typing in the game — `docs/arms.json`, group `text`.
//!
//! **Overwrite is the default.** `g_editInsert` (`0x005C9280`) starts at zero
//! and zero is the *overwrite* branch of `Edit_Insert`; the Insert key
//! (`VK_INSERT`, `Edit_ToggleInsert`) turns insertion on. So arriving on the
//! name page, whose field is seeded with the name you already have, and typing
//! `E`, `d` gives **`Edayer1`** and not `Ed`. That is what the shipped game
//! does. `docs/bugs.md` B79.
//!
//! * **The blink phase advances on a tick, not a frame.** The original counts
//!   frames in `Edit_DrawCaret` itself (`0x004E5998`, 0…16, drawn above 8).

mod player_name;
pub use player_name::*;
mod tests_part;
pub use tests_part::*;

use l2_view::Canvas;

use crate::input::{Event, Key};

pub const PLAYER_NAME_LEN: usize = 0x1F;

pub const NAME_MAX_TYPED: usize = 0x10;

pub const NAME_MAX_PIXELS: i32 = 0xC0;

pub const DEFAULT_PLAYER_NAME: &str = "Player1";

/// `g_editKind` (`0x005C928C`) — `Edit_Begin`'s fourth argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Text,
    /// `1` — a DOS file name. `,` `.` `?` `!` are refused and `A`–`Z` are
    /// lower-cased by `0x004011B0`.
    Filename,
}

/// `g_editState` (`0x005CD408`) after `Edit_Clamp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Ok,
    PastEnd,
    Full,
}

pub trait Metrics {
    /// `FUN_004015B9(ch, font)`: **0 for NUL, 4 for a space, 0 for a character
    /// the font has no glyph for, otherwise the glyph's frame width plus one.**
    fn advance(&self, c: char) -> i32;

    /// `FUN_00401F8F`: the sum of the advances, which is what the pixel limit
    /// is compared against.
    fn width(&self, s: &[char]) -> i32 {
        s.iter().map(|c| self.advance(*c)).sum()
    }
}

/// `Edit_Clamp` measures with `0x005AF8F0` — one fixed font global — so every
/// field in the game is limited by the same font whatever the screen draws in.
pub struct FontMetrics<'a>(pub Option<&'a crate::shell::font::Font>);

impl Metrics for FontMetrics<'_> {
    fn advance(&self, c: char) -> i32 {
        match self.0 {
            // `Font::width` of one character is `FUN_004015B9` exactly: the
            // frame width plus one
            Some(f) => f.width(&c.to_string()),
            None => 4,
        }
    }
}

impl<'a> FontMetrics<'a> {
    pub fn of(assets: &'a crate::shell::ShellAssets) -> FontMetrics<'a> {
        FontMetrics(assets.body.as_ref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextField {
    /// `g_editBuffer` (`0x005CD550`). Latin-1 characters, no NUL.
    buf: Vec<char>,
    /// `g_editCaret` (`0x005BB4A8`).
    caret: usize,
    /// `g_editMaxLen` (`0x005CD4F4`) — `Edit_Begin`'s second argument.
    max_len: usize,
    /// `g_editMaxPixels` (`0x0058FD68`) — `Edit_Begin`'s third.
    max_px: i32,
    kind: Kind,
    /// `g_editInsert` (`0x005C9280`). **False is overwrite and false is the
    /// default**; see the module header.
    insert: bool,
    /// `g_editState` (`0x005CD408`) as `Edit_Clamp` last left it.
    state: State,
    /// `g_editTooWide` (`0x0057D3B0`).
    too_wide: bool,
    /// `g_caretBlink` (`0x004E5998`), 0..=16. Ours; see the module header.
    blink: u32,
}

const BLINK_PERIOD: u32 = 17;
const BLINK_LIT_FROM: u32 = 9;

impl TextField {
    /// `Edit_Begin` (`0x00402009`).
    pub fn begin(seed: &str, max_len: usize, max_px: i32, kind: Kind) -> TextField {
        TextField {
            buf: seed.chars().collect(),
            caret: 0,
            max_len,
            max_px,
            kind,
            insert: false,
            state: State::Ok,
            too_wide: false,
            blink: 0,
        }
    }

    pub fn text(&self) -> String {
        self.buf.iter().collect()
    }

    /// `Edit_Commit` (`0x0040210C`): the buffer, truncated to `max` characters.
    pub fn commit(&self, max: usize) -> String {
        self.buf.iter().take(max).collect()
    }

    pub fn caret(&self) -> usize {
        self.caret
    }

    pub fn state(&self) -> State {
        self.state
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    pub fn inserting(&self) -> bool {
        self.insert
    }

    /// `Edit_Clamp` (`0x00401984`), in its own order, because the order is what
    /// decides which of the two limits gets to name the state.
    ///
    /// ```c
    /// g_editState = 0;
    /// Edit_MeasureWidth(0x5af8f0);                  /* sets g_editTooWide */
    /// if (g_editLength + 1 <= g_editCaret) { g_editCaret = g_editLength + 1; g_editState = 1; }
    /// if (g_editMaxLen   <= g_editCaret)   { g_editCaret = g_editMaxLen;     g_editState = 2; }
    /// if (g_editTooWide)                     g_editState = 2;
    /// if (g_editCaret < 0)                   g_editCaret = 0;
    /// ```
    pub fn clamp(&mut self, m: &dyn Metrics) {
        self.state = State::Ok;
        self.too_wide = m.width(&self.buf) >= self.max_px;
        if self.caret > self.buf.len() {
            self.caret = self.buf.len();
            self.state = State::PastEnd;
        }
        if self.caret >= self.max_len {
            self.caret = self.max_len;
            self.state = State::Full;
        }
        if self.too_wide {
            self.state = State::Full;
        }
    }

    /// `Edit_TypeChar` (`0x00401A20`) — **the character set, exactly.**
    pub fn type_char(&mut self, c: char, m: &dyn Metrics) -> bool {
        let b = c as u32;
        let accepted = match b {
            0x20 => Some(c),
            0x2C | 0x2E | 0x3F | 0x21 if self.kind == Kind::Text => Some(c),
            0x2D => Some(c),
            0x30..=0x39 => Some(c),
            0x61..=0x7A => Some(c),
            // `0x004011B0`: `if ('A' <= ch && ch <= 'Z') ch += 0x20`.
            0x41..=0x5A => Some(if self.kind == Kind::Text { c } else { c.to_ascii_lowercase() }),
            0x80..=0x9A | 0xA0..=0xA7 | 0xE1 => Some(c),
            _ => None,
        };
        let Some(c) = accepted else { return false };
        let before = self.buf.len();
        self.put(c, m);
        self.clamp(m);
        self.buf.len() != before || self.caret > 0
    }

    /// `Edit_Insert` (`0x00401D26`): one accepted character at the caret.
    ///
    // arm: 0x00401D26/overwrite-default key
    fn put(&mut self, c: char, m: &dyn Metrics) {
        self.clamp(m);
        if self.state == State::Full {
            return;
        }
        if !self.insert {
            if self.caret < self.buf.len() {
                self.buf[self.caret] = c;
            } else {
                self.buf.push(c);
            }
            self.caret += 1;
        } else if self.buf.len() < self.max_len {
            self.buf.insert(self.caret, c);
            self.caret += 1;
        }
    }

    /// `Edit_Backspace` (`0x00401C70`): caret back one, **then** delete.
    pub fn backspace(&mut self, m: &dyn Metrics) {
        if self.caret > 0 {
            self.caret -= 1;
            self.delete(m);
        }
    }

    /// `Edit_Delete` (`0x00401C93` → `0x00401DC8`): the tail shifts left over
    /// the caret, which leaves the caret where it was.
    pub fn delete(&mut self, m: &dyn Metrics) {
        if self.caret < self.buf.len() {
            self.buf.remove(self.caret);
        }
        self.clamp(m);
    }

    /// `Edit_Left` (`0x00401CBC`).
    pub fn left(&mut self) {
        self.caret = self.caret.saturating_sub(1);
    }

    /// `Edit_Right` (`0x00401CDA`) — stops at the length, not at `max_len`.
    pub fn right(&mut self) {
        if self.caret < self.buf.len() {
            self.caret += 1;
        }
    }

    /// `Edit_Home` (`0x00401CFC`).
    pub fn home(&mut self) {
        self.caret = 0;
    }

    /// `Edit_End` (`0x00401D11`).
    pub fn end(&mut self) {
        self.caret = self.buf.len();
    }

    /// `Edit_ToggleInsert` (`0x00401CA3`): `g_editInsert ^= 1`.
    pub fn toggle_insert(&mut self) {
        self.insert = !self.insert;
    }

    /// **Two keys the original dispatches here are deliberately absent**,
    /// because neither of them edits: `VK_RETURN` runs `Chat_Toggle`
    /// (`0x0043600C`) and `VK_ESCAPE` runs `Menu_Quit` or arms the back-out
    /// flag. Both are the screen's business, and a field that swallowed Return
    /// would take the only key that opens multiplayer chat.
    pub fn event(&mut self, event: Event, m: &dyn Metrics) -> bool {
        match event {
            // arm: 0x004B29BE/wm-char key
            Event::Text(c) => self.type_char(c, m),
            Event::KeyDown(key) => match key {
                // arm: 0x00401C70/backspace key
                Key::Backspace => {
                    self.backspace(m);
                    true
                }
                // arm: 0x00401DC8/delete key
                Key::Delete => {
                    self.delete(m);
                    true
                }
                // arm: 0x00401CBC/caret-left key
                Key::Left => {
                    self.left();
                    true
                }
                // arm: 0x00401CDA/caret-right key
                Key::Right => {
                    self.right();
                    true
                }
                // arm: 0x00401CFC/caret-home key
                Key::Home => {
                    self.home();
                    true
                }
                // arm: 0x00401D11/caret-end key
                Key::End => {
                    self.end();
                    true
                }
                // arm: 0x00401CA3/toggle-insert key
                Key::Insert => {
                    self.toggle_insert();
                    true
                }
                Key::Char(_) | Key::Space => true,
                _ => false,
            },
            _ => false,
        }
    }

    pub fn tick(&mut self) {
        self.blink = (self.blink + 1) % BLINK_PERIOD;
    }

    pub fn caret_lit(&self) -> bool {
        self.blink >= BLINK_LIT_FROM
    }

    pub fn caret_x(&self, m: &dyn Metrics) -> i32 {
        m.width(&self.buf[..self.caret.min(self.buf.len())])
    }

    /// `Edit_DrawCaret` (`0x0040ACCE`), at the text origin `(x, y)` the field's
    /// painter used. The original's caller adds two to the text's `y` before
    /// calling
    /// it passed to the text.
    ///
    /// Overwrite — the default — is an **underline** two pixels tall under the
    /// character; insert is an **I-beam** before it. The width of the underline
    /// is the advance of the character *under* the caret, so at the end of the
    /// text, where that character is the terminator and `FUN_004015B9` returns
    /// zero, the underline is a two-pixel stub.
    ///
    // arm: 0x0040ACCE/caret timer
    pub fn draw_caret(&self, canvas: &mut Canvas, x: i32, y: i32, colour: u8, m: &dyn Metrics) {
        if !self.caret_lit() {
            return;
        }
        let x = x + self.caret_x(m);
        let y = y + 2;
        let w = match self.buf.get(self.caret) {
            Some(c) => m.advance(*c),
            // The NUL past the end: `FUN_004015B9` returns 0 for it.
            None => 0,
        };
        if !self.insert {
            canvas.fill_rect(x - 1, y + 0xE, w + 2, 2, colour);
        } else {
            canvas.fill_rect(x - 2, y - 2, 1, 0xF, colour);
            canvas.fill_rect(x - 4, y - 3, 2, 1, colour);
            canvas.fill_rect(x - 4, y + 0xE, 2, 1, colour);
            canvas.fill_rect(x - 1, y - 3, 2, 1, colour);
            canvas.fill_rect(x - 1, y + 0xE, 2, 1, colour);
        }
    }
}

