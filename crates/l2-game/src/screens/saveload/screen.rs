#![allow(unused_imports)]
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

