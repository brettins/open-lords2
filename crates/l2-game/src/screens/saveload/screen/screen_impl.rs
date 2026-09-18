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

    fn take_clicks(&mut self) -> u8 {
        self.press.take_clicks()
    }

    fn take_redraw(&mut self) -> bool {
        self.press.take_redraw()
    }

    fn is_overlay(&self) -> bool {
        true
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
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
            // arm: ours/saveload-key-scroll key
            Event::KeyDown(Key::Up) => {
                self.scroll(-(SCROLL_STEP as i32));
                Transition::Stay
            }
            Event::KeyDown(Key::Down) => {
                self.scroll(SCROLL_STEP as i32);
                Transition::Stay
            }
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

        for i in 0..PAGE {
            let Some(entry) = self.entries.get(self.top + i) else { break };
            let r = Self::row_rect(i);
            if self.selected == Some(self.top + i) {
                // `g_spriteWidth = 6; g_spriteHeight = 0x10; FUN_004B414A(x - 2,
                // y - 1, 0x3F)`, then the row's own text in colour 0x20.
                //
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

        match &self.status {
            Status::Idle => {}
            // `SaveLoad_DrawStatus`: `Eng_DrawString(40, 2 | 3, …)` while
            // `DAT_0057D3C4` runs.
            Status::Working => {
                pen.eng(canvas, GROUP, self.mode.working_index(), STATUS.0, STATUS.1, font::TEXT);
            }
            Status::Failed(detail) => {
                pen.eng(canvas, GROUP, ERROR_INDEX, STATUS.0, STATUS.1, font::TEXT);
                if ctx.game.prefs.debug_overlay {
                    text::draw(canvas, STATUS.0, STATUS.1 + 18, &ours(detail), ink.bad);
                }
            }
        }

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

        let where_ = match saves::dir() {
            Some(d) => d.display().to_string().to_uppercase(),
            None => "NO SAVE DIRECTORY ON THIS MACHINE".into(),
        };
        if ctx.game.prefs.debug_overlay {
            text::draw(canvas, DIR_LINE.0, DIR_LINE.1, &directory_line(&where_), ink.dim);
        }
    }
}


