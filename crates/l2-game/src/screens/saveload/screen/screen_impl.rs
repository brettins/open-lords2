#![allow(unused_imports)]
use super::*;
use super::methods::*;
use super::*;
use super::helpers::*;
use super::tests::*;
use l2_view::{text, Canvas};
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::saves::{self, Entry};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};

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


