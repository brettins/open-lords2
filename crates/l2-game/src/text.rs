//! **Typing.** The original's text-entry engine, which is one shared buffer and
//! nine keys.
//!
//! # Why this module exists at all
//!
//! A player reported *"I can't type my name in the start menu?"* and the answer
//! was larger than the report: **this workspace had no keyboard text entry of
//! any kind.** `crate::input::Key::Backspace` was manufactured by `main.rs` and
//! read by nothing outside one hand-rolled field on the save screen;
//! `Key::Char` reached only hotkeys. Every field the original lets a person
//! type into was missing, and nobody had counted them.
//!
//! # The original is one buffer, not one widget per field
//!
//! There is exactly **one** edit buffer in `Lords2.exe` — `g_editBuffer`
//! (`0x005CD550`), 2,000 bytes — with a caret (`0x005BB4A8`), a length
//! (`0x0058FE94`), a character limit (`0x005CD4F4`), a *pixel* limit
//! (`0x0058FD68`) and a kind (`0x005C928C`). A screen does not own a field; it
//! **seeds** the buffer on entry and **copies it out** every frame:
//!
//! | function | what it is | here |
//! |---|---|---|
//! | `Edit_Begin` `0x00402009` | clear, seed from a string, set the two limits and the kind, caret to 0 | [`TextField::begin`] |
//! | `Edit_TypeChar` `0x00401A20` | **the character filter**, and the only way a printable character gets in | [`TextField::type_char`] |
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
//! straight to `Edit_TypeChar` with no screen test whatever, and the nine
//! editing keys are arms of `WM_KEYDOWN` (`0x100`) that are equally ungated.
//! What decides whether any of it *lands* is a single flag, `g_editActive`
//! (`0x005AEB78`): `Screen_HandleInput` clears it at the top of every frame and
//! exactly seven of its arms set it again. Those seven arms are the whole
//! inventory of typing in the game — `docs/arms.json`, group `text`.
//!
//! # Two things here that are the original's and look like mistakes
//!
//! **Overwrite is the default.** `g_editInsert` (`0x005C9280`) starts at zero
//! and zero is the *overwrite* branch of `Edit_Insert`; the Insert key
//! (`VK_INSERT`, `Edit_ToggleInsert`) turns insertion on. So arriving on the
//! name page, whose field is seeded with the name you already have, and typing
//! `E`, `d` gives **`Edayer1`** and not `Ed`. That is what the shipped game
//! does. `docs/bugs.md` B79.
//!
//! **A filename field refuses a full stop.** Kind 1 rejects `,` `.` `?` `!`
//! outright and lower-cases `A`–`Z`, which is a DOS 8.3 name with the
//! extension supplied by the caller. It still accepts the eight high-byte runs
//! a text field does.
//!
//! # What is ours, and marked
//!
//! * **The blink phase advances on a tick, not a frame.** The original counts
//!   frames in `Edit_DrawCaret` itself (`0x004E5998`, 0…16, drawn above 8).
//!   `docs/netcode.md` does not let anything below the renderer learn how much
//!   time passed, and a `draw` that mutates is worse than a phase carried on
//!   the field, so [`TextField::tick`] steps it. Presentation only: it can move
//!   pixels and cannot move a number.
//! * **The buffer is a `Vec<char>` of Latin-1 characters**, not 2,000 bytes.
//!   `docs/formats/eng.md` §"String character set" settles that the game's text
//!   is Latin-1, so a `char` under `0x100` *is* the original's byte and every
//!   range test below is the original's byte test unchanged.

use l2_view::Canvas;

use crate::input::{Event, Key};

/// How many characters one `g_playerNames` record holds — `0x1F`, and it is
/// `Edit_Commit`'s own limit at the name page's arm: `Edit_Commit(&g_options,
/// 0x1F)`.
pub const PLAYER_NAME_LEN: usize = 0x1F;

/// **How many a person may actually type**, which is a different number and
/// smaller: `Edit_Begin(&g_options, 0x10, 0xC0, 0)`.
pub const NAME_MAX_TYPED: usize = 0x10;

/// The name field's pixel limit on setup page 4 — 192, which is what stops a
/// wide sixteen-character name from running off the 224-pixel plate it is drawn
/// on (`Panels2.pl8` frame 204).
pub const NAME_MAX_PIXELS: i32 = 0xC0;

/// The persisted default. `g_options` is 0x468 bytes read and written whole,
/// and byte 0 begins a 31-byte name that `Options_SetDefaults` fills with this.
pub const DEFAULT_PLAYER_NAME: &str = "Player1";

/// **One record of `g_playerNames`** (`0x00553D54`) — a lord's name as the
/// interface shows it.
///
/// Fixed width on purpose, and the width is the original's. The array is six
/// 44-byte slots at `0x00553D50` and one save block of its own
/// (`g_saveBlocks[2] = {0x00553D50, 264}`); the name is 31 bytes at `+4`, the
/// banner colour is `+0x25` and `+0x27` is the "a person drives this" byte
/// `Player_SetHuman` sets. A `String` here would put a length prefix and an
/// allocation into something the original writes as a fixed run of bytes.
///
/// **Latin-1 in, Latin-1 out** — `docs/formats/eng.md` — so a byte here is a
/// `char` under `0x100` and the two are the same thing.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PlayerName([u8; PLAYER_NAME_LEN]);

impl PlayerName {
    pub const EMPTY: PlayerName = PlayerName([0; PLAYER_NAME_LEN]);

    /// Truncated at [`PLAYER_NAME_LEN`], which is `Edit_Commit`'s truncation.
    /// Characters above Latin-1 cannot come out of a [`TextField`] — the
    /// filter refuses everything above `0xE1` — but a caller with a `String`
    /// from somewhere else drops them rather than writing half a code point.
    pub fn new(s: &str) -> PlayerName {
        let mut out = [0u8; PLAYER_NAME_LEN];
        for (slot, c) in out.iter_mut().zip(s.chars().filter(|c| (*c as u32) < 0x100)) {
            *slot = c as u8;
        }
        PlayerName(out)
    }

    pub fn bytes(&self) -> &[u8; PLAYER_NAME_LEN] {
        &self.0
    }

    pub fn from_bytes(b: [u8; PLAYER_NAME_LEN]) -> PlayerName {
        PlayerName(b)
    }

    /// The name as text, stopping at the terminator the way every reader in the
    /// binary does.
    pub fn as_str(&self) -> String {
        self.0.iter().take_while(|b| **b != 0).map(|b| *b as char).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.0[0] == 0
    }
}

impl std::fmt::Debug for PlayerName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.as_str())
    }
}

impl Default for PlayerName {
    fn default() -> PlayerName {
        PlayerName::EMPTY
    }
}

/// `g_editKind` (`0x005C928C`) — `Edit_Begin`'s fourth argument.
///
/// Two values are ever passed: `0` for prose and `1` for a file name. The kind
/// changes two things and only two, both in `Edit_TypeChar`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `0` — a name, a chat line, a letter. Punctuation allowed, case kept.
    Text,
    /// `1` — a DOS file name. `,` `.` `?` `!` are refused and `A`–`Z` are
    /// lower-cased by `0x004011B0`.
    Filename,
}

/// `g_editState` (`0x005CD408`) after `Edit_Clamp`.
///
/// Not the kind, despite `Edit_Begin` seeding it from the kind: `Edit_Clamp`
/// overwrites it on the first keystroke and from then on it is a *status*.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// `0` — room to type.
    Ok,
    /// `1` — the caret was past the end of the text and has been pulled back.
    PastEnd,
    /// `2` — **no more characters are accepted**: the caret has reached the
    /// character limit, or the text has reached the pixel limit. Deleting still
    /// works, which is the only way out.
    Full,
}

/// How wide the field's font draws things.
///
/// A trait rather than a borrow of the font, because [`TextField`] is stepped
/// from `Screen::handle`, which has `Ctx` and not a `Pen`, and because a test
/// wants to drive the pixel limit without an install. `docs/agents.md`: the
/// mechanism that needs nothing of the world.
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

/// The body font, when the install has one.
///
/// `Edit_Clamp` measures with `0x005AF8F0` — one fixed font global — so every
/// field in the game is limited by the same font whatever the screen draws in.
pub struct FontMetrics<'a>(pub Option<&'a crate::shell::font::Font>);

impl Metrics for FontMetrics<'_> {
    fn advance(&self, c: char) -> i32 {
        match self.0 {
            // `Font::width` of one character is `FUN_004015B9` exactly: the
            // frame width plus one, or `SPACE_ADVANCE` where there is no glyph.
            Some(f) => f.width(&c.to_string()),
            // No font is not "every character is zero wide" — that would make
            // the pixel limit unreachable and let a name overrun its plate on
            // an install with no artwork. Four is the original's space advance
            // and the fallback renderer's cell.
            None => 4,
        }
    }
}

impl<'a> FontMetrics<'a> {
    /// The metrics a screen has: the shell's body font.
    pub fn of(assets: &'a crate::shell::ShellAssets) -> FontMetrics<'a> {
        FontMetrics(assets.body.as_ref())
    }
}

/// **One text field**, which in the original is the one global buffer with one
/// screen's limits loaded into it.
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

/// The caret is drawn while the counter is above eight, and the counter wraps
/// past sixteen — so eight ticks lit, nine dark. `Edit_DrawCaret`.
const BLINK_PERIOD: u32 = 17;
const BLINK_LIT_FROM: u32 = 9;

impl TextField {
    /// `Edit_Begin` (`0x00402009`).
    ///
    /// `seed` is the text the field opens with — the *current* name on the name
    /// page, an empty string on the chat line. `max_len` and `max_px` are the
    /// two limits, and both are real: the pixel one is what stops a wide name
    /// from running out of its plate.
    pub fn begin(seed: &str, max_len: usize, max_px: i32, kind: Kind) -> TextField {
        TextField {
            buf: seed.chars().collect(),
            caret: 0,
            max_len,
            max_px,
            kind,
            // NOT reset by `Edit_Begin`. The original's is a global that the
            // Insert key toggles and nothing else writes, so it survives from
            // one field to the next; a fresh field here starts at the value a
            // fresh game has.
            insert: false,
            state: State::Ok,
            too_wide: false,
            blink: 0,
        }
    }

    /// What the field holds, which is what `Edit_Commit` would copy out.
    pub fn text(&self) -> String {
        self.buf.iter().collect()
    }

    /// `Edit_Commit` (`0x0040210C`): the buffer, truncated to `max` characters.
    ///
    /// **The commit limit is not the edit limit and is usually larger.** The
    /// save screen edits eight characters and commits sixty-four; the name page
    /// edits sixteen and commits thirty-one. The destination's size is what
    /// `max` describes, so the truncation never fires in a shipped game — it is
    /// the buffer's guarantee to the destination, not a rule a player meets.
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

    /// Whether the field is in insert mode — `g_editInsert`. False is overwrite
    /// and false is where a field starts.
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
    ///
    /// The `length + 1` is the original's and is an off-by-one: it lets the
    /// caret sit one place past the terminator of a 2,000-byte buffer, which is
    /// harmless there and would be an out-of-bounds index here, so the clamp is
    /// to the length. Nothing can reach the extra slot — `Edit_Right` stops at
    /// the length and `Edit_End` sets it — so no behaviour rides on the
    /// difference.
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
    ///
    /// The chain, in the binary's order, with `kind` written as the original's
    /// `g_editKind == 0` test:
    ///
    /// | byte | accepted |
    /// |---|---|
    /// | `0x20` space | always |
    /// | `0x2C , ` `0x2E .` `0x3F ?` `0x21 !` | **[`Kind::Text`] only** |
    /// | `0x2D -` | always |
    /// | `0x30`–`0x39` digits | always |
    /// | `0x61`–`0x7A` `a`–`z` | always |
    /// | `0x41`–`0x5A` `A`–`Z` | always, **lower-cased** unless [`Kind::Text`] |
    /// | `0x80`–`0x9A`, `0xA0`–`0xA7`, `0xE1` | always |
    ///
    /// **Everything else is dropped in silence** — no beep, no message, and in
    /// particular the apostrophe, the ampersand, every bracket and the whole of
    /// `0xA8`–`0xE0`. A person typing `O'Neill` gets `ONeill`.
    ///
    /// Returns whether the character was accepted, which the original does not
    /// — it is here so a screen can tell "the field ate this key" from "the
    /// field is not interested", and a screen that used the answer to make a
    /// noise the original does not make would be an invention.
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
        // `Edit_TypeChar` ends with `Edit_Clamp`, whatever happened.
        self.clamp(m);
        self.buf.len() != before || self.caret > 0
    }

    /// `Edit_Insert` (`0x00401D26`): one accepted character at the caret.
    ///
    /// ```c
    /// if (!g_editActive) return;
    /// Edit_Clamp();
    /// if (g_editState == 2) return;                 /* full: silently dropped */
    /// if (g_editInsert == 0) { buf[caret++] = ch; } /* OVERWRITE, the default */
    /// else if (g_editLength < g_editMaxLen) { shift right; buf[caret++] = ch; }
    /// ```
    ///
    /// The two branches have **different limits**, and that is the original's:
    /// overwrite is bounded only by the caret reaching `max_len`, insert also
    /// needs the *length* to be under it. On a full field they agree.
    ///
    // arm: 0x00401D26/overwrite-default
    fn put(&mut self, c: char, m: &dyn Metrics) {
        self.clamp(m);
        if self.state == State::Full {
            return;
        }
        if !self.insert {
            if self.caret < self.buf.len() {
                self.buf[self.caret] = c;
            } else {
                // Past the text, into what is NUL in the original's buffer:
                // the write extends the string by one.
                self.buf.push(c);
            }
            self.caret += 1;
        } else if self.buf.len() < self.max_len {
            self.buf.insert(self.caret, c);
            self.caret += 1;
        }
    }

    /// `Edit_Backspace` (`0x00401C70`): caret back one, **then** delete.
    ///
    /// Not gated on the full state — deleting is how a person gets out of a
    /// field that has hit either limit, and the original leaves that road open.
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

    /// **One event, routed the way the window procedure routes it.**
    ///
    /// Returns whether the field consumed it. The original's split is two
    /// Windows messages and this is the same split: [`Event::Text`] is
    /// `WM_CHAR` (`0x102`), whose whole arm is `Edit_TypeChar`, and the six
    /// [`Event::KeyDown`] arms below are `WM_KEYDOWN`'s (`0x100`) editing
    /// cases. Every other key belongs to the screen and is handed back.
    ///
    /// **Two keys the original dispatches here are deliberately absent**,
    /// because neither of them edits: `VK_RETURN` runs `Chat_Toggle`
    /// (`0x0043600C`) and `VK_ESCAPE` runs `Menu_Quit` or arms the back-out
    /// flag. Both are the screen's business, and a field that swallowed Return
    /// would take the only key that opens multiplayer chat.
    ///
    /// **And one thing the original does that no field here should copy:** all
    /// seven of these arms are *ungated by screen* in the binary. The caret
    /// keys move a caret on the campaign map, in a battle and on the title
    /// page, because the window procedure never asks what is on screen; only
    /// the insert and the delete consult `g_editActive`. Reproducing that would
    /// mean a hidden caret walking about while a person plays, so a field here
    /// is stepped by the screen that owns it and by nothing else.
    pub fn event(&mut self, event: Event, m: &dyn Metrics) -> bool {
        match event {
            // arm: 0x004B29BE/wm-char
            Event::Text(c) => self.type_char(c, m),
            Event::KeyDown(key) => match key {
                // arm: 0x00401C70/backspace
                Key::Backspace => {
                    self.backspace(m);
                    true
                }
                // arm: 0x00401DC8/delete
                Key::Delete => {
                    self.delete(m);
                    true
                }
                // arm: 0x00401CBC/caret-left
                Key::Left => {
                    self.left();
                    true
                }
                // arm: 0x00401CDA/caret-right
                Key::Right => {
                    self.right();
                    true
                }
                // arm: 0x00401CFC/caret-home
                Key::Home => {
                    self.home();
                    true
                }
                // arm: 0x00401D11/caret-end
                Key::End => {
                    self.end();
                    true
                }
                // arm: 0x00401CA3/toggle-insert
                Key::Insert => {
                    self.toggle_insert();
                    true
                }
                // **Swallowed, and this is not fussiness.** `main.rs` delivers
                // a printable key as *both* messages, exactly as Windows does:
                // `KeyDown(Key::Char('I'))` and then `Text('I')`. The character
                // arrives above; if the `KeyDown` half were handed back, a
                // screen whose `I` opens something would open it *and* type an
                // I, and a screen whose Space confirms would confirm *and* type
                // a space. Both of those were live bugs for the length of one
                // compile.
                //
                // In the original the question does not arise, because
                // `VK_SPACE` and the letter keys have **no `WM_KEYDOWN` arm at
                // all** — the window procedure's `0x100` switch handles
                // backspace, return, control, escape, home, end, the arrows,
                // insert, delete, the nine digits and the function keys, and
                // nothing else. A field being live is exactly when a letter is
                // a letter.
                Key::Char(_) | Key::Space => true,
                _ => false,
            },
            _ => false,
        }
    }

    /// The blink phase. See the module header for why it is a tick and not a
    /// frame.
    pub fn tick(&mut self) {
        self.blink = (self.blink + 1) % BLINK_PERIOD;
    }

    /// Whether the caret is lit this tick — `if (8 < g_caretBlink)`.
    pub fn caret_lit(&self) -> bool {
        self.blink >= BLINK_LIT_FROM
    }

    /// Where the caret sits relative to the field's text origin.
    ///
    /// `Ui_DrawText` captures it while it draws: `if (index == g_editCaret &&
    /// !g_caretPlaced) { g_caretX = g_penAdvance; … }`. The pen advance after
    /// `caret` characters *is* the width of the first `caret` characters, so
    /// this needs no cooperation from the drawing code — which is what lets the
    /// caret be drawn by the field rather than by the font.
    pub fn caret_x(&self, m: &dyn Metrics) -> i32 {
        m.width(&self.buf[..self.caret.min(self.buf.len())])
    }

    /// `Edit_DrawCaret` (`0x0040ACCE`), at the text origin `(x, y)` the field's
    /// painter used. The original's caller adds two to the text's `y` before
    /// calling, and that two is folded in here so a screen passes the same `y`
    /// it passed to the text.
    ///
    /// **Two shapes, and they are the opposite way round to the convention.**
    /// Overwrite — the default — is an **underline** two pixels tall under the
    /// character; insert is an **I-beam** before it. The width of the underline
    /// is the advance of the character *under* the caret, so at the end of the
    /// text, where that character is the terminator and `FUN_004015B9` returns
    /// zero, the underline is a two-pixel stub.
    ///
    // arm: 0x0040ACCE/caret
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Every character four pixels wide, so the pixel limit is a character
    /// count and a test can say what it means.
    struct Four;
    impl Metrics for Four {
        fn advance(&self, _c: char) -> i32 {
            4
        }
    }

    fn text(seed: &str, max: usize) -> TextField {
        TextField::begin(seed, max, 10_000, Kind::Text)
    }

    fn typed(f: &mut TextField, s: &str) {
        for c in s.chars() {
            f.type_char(c, &Four);
        }
    }

    /// **The default is overwrite, and this is the reported bug's real shape.**
    #[test]
    fn a_seeded_field_overwrites_because_that_is_where_the_original_starts() {
        let mut f = text("Player1", 16);
        assert!(!f.inserting(), "g_editInsert is zero in a fresh game");
        typed(&mut f, "Ed");
        assert_eq!(f.text(), "Edayer1", "overwrite, not insert");
        // And the Insert key is the way out of it.
        let mut f = text("Player1", 16);
        f.toggle_insert();
        typed(&mut f, "Ed");
        assert_eq!(f.text(), "EdPlayer1");
    }

    /// Typing past the end of a seeded field extends it, because the original
    /// is writing into cleared buffer rather than off the end of a string.
    #[test]
    fn overwriting_past_the_end_extends_the_text() {
        let mut f = text("ab", 16);
        f.end();
        typed(&mut f, "cd");
        assert_eq!(f.text(), "abcd");
    }

    /// **The character filter, both kinds.** Every row is a byte range out of
    /// `Edit_TypeChar`.
    #[test]
    fn the_filter_is_the_originals_seven_ranges() {
        let mut f = text("", 64);
        typed(&mut f, "Ab9 -,.?!");
        assert_eq!(f.text(), "Ab9 -,.?!", "text kind keeps its four punctuation marks");

        let mut f = text("", 64);
        typed(&mut f, "O'Neill (the 3rd) & co_");
        assert_eq!(
            f.text(),
            "ONeill the 3rd  co",
            "apostrophe, brackets, ampersand and underscore are dropped in silence"
        );

        let mut f = TextField::begin("", 64, 10_000, Kind::Filename);
        typed(&mut f, "MySave.sav");
        assert_eq!(f.text(), "mysavesav", "a filename lower-cases and refuses the dot");

        // The three high runs, which are the accented letters and are accepted
        // by both kinds.
        let mut f = TextField::begin("", 64, 10_000, Kind::Filename);
        typed(&mut f, "\u{80}\u{9a}\u{a0}\u{a7}\u{e1}\u{9b}\u{a8}\u{e0}");
        assert_eq!(
            f.text(),
            "\u{80}\u{9a}\u{a0}\u{a7}\u{e1}",
            "0x9B, 0xA8 and 0xE0 are between the runs and are refused"
        );
    }

    /// The two limits, and the fact that deleting is the only way back.
    #[test]
    fn typing_stops_at_the_character_limit_and_at_the_pixel_limit() {
        let mut f = text("", 4);
        typed(&mut f, "abcdefg");
        assert_eq!(f.text(), "abcd", "max_len 4");
        assert_eq!(f.state(), State::Full);
        f.backspace(&Four);
        assert_eq!(f.text(), "abc");
        f.type_char('z', &Four);
        assert_eq!(f.text(), "abcz", "a delete reopens the field");

        // Sixteen characters allowed, but only twenty pixels of them: at four
        // pixels each the fifth is refused, because the test is `>=`.
        let mut f = TextField::begin("", 16, 20, Kind::Text);
        typed(&mut f, "abcdefgh");
        assert_eq!(f.text(), "abcde", "width 20 >= max 20 stops the sixth");
        assert_eq!(f.state(), State::Full);
    }

    /// Backspace deletes to the left, Delete to the right, and neither moves
    /// the other's character.
    #[test]
    fn the_caret_keys_are_the_window_procedures_six() {
        let mut f = text("abcd", 16);
        f.end();
        assert_eq!(f.caret(), 4);
        f.right();
        assert_eq!(f.caret(), 4, "Edit_Right stops at the length");
        f.left();
        f.left();
        assert_eq!(f.caret(), 2);
        f.backspace(&Four);
        assert_eq!((f.text().as_str(), f.caret()), ("acd", 1));
        f.delete(&Four);
        assert_eq!((f.text().as_str(), f.caret()), ("ad", 1));
        f.home();
        assert_eq!(f.caret(), 0);
        f.left();
        assert_eq!(f.caret(), 0, "and Edit_Left stops at zero");
    }

    /// The caret x is the width of the text before it, which is what
    /// `Ui_DrawText` captures while it draws.
    #[test]
    fn the_caret_sits_after_the_characters_before_it() {
        let mut f = text("abcd", 16);
        assert_eq!(f.caret_x(&Four), 0);
        f.right();
        f.right();
        assert_eq!(f.caret_x(&Four), 8);
        f.end();
        assert_eq!(f.caret_x(&Four), 16);
    }

    /// Eight ticks lit in seventeen.
    #[test]
    fn the_caret_blinks_on_the_originals_period() {
        let mut f = text("", 16);
        let lit = (0..BLINK_PERIOD)
            .filter(|_| {
                f.tick();
                f.caret_lit()
            })
            .count();
        assert_eq!(lit, 8, "counter 9..=16 of 0..=16");
    }

    /// `Edit_Commit`'s truncation is the destination's size, and it is larger
    /// than the edit limit everywhere the game uses it.
    #[test]
    fn the_commit_limit_is_not_the_edit_limit() {
        let mut f = text("", 16);
        typed(&mut f, "Aethelred The Un");
        assert_eq!(f.text().len(), 16);
        assert_eq!(f.commit(31), f.text(), "31 is g_options' name field");
        assert_eq!(f.commit(4), "Aeth");
    }
}
