#![allow(unused_imports)]
use super::*;
use super::page::*;
use super::words::*;
use l2_view::Canvas;
use crate::game::PRESENTATION;
use crate::input::{Event, Key, Rect};
use crate::press::{Kind, Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};


const QUIRK_ROW_H: i32 = 16;
const QUIRK_LIST_X: i32 = 0x30;
const QUIRK_LIST_Y: i32 = 0x70;
const QUIRK_BOX_X: i32 = 0x20;
const QUIRK_PARENT: (i32, i32) = (0x20, 0x54);
const QUIRK_BOX: i32 = 12;

pub struct OptionsScreen {
    page: Page,
    hover: Option<usize>,
    hover_parent: bool,
    /// **`Widget_Test`'s per-record state for the page's table** — the press
    /// timer at `+0x0D` that holds the pressed frame up and fires the handler
    /// when it runs out. See [`Row::kind`].
    press: Press,
}

impl OptionsScreen {
    pub fn new(page: Page) -> OptionsScreen {
        OptionsScreen { page, hover: None, hover_parent: false, press: Press::new() }
    }

    pub fn page(&self) -> Page {
        self.page
    }

    /// `Widget_Draw` adds one to the record's frame while `+0x0D` is non-zero,
    /// which for a kind-5 row is the whole twenty frames between the press and
    /// the toggle — and the timer is each record's own, so two rows pressed a
    /// moment apart are both down.
    pub fn pressed_rows(&self) -> Vec<usize> {
        (0..self.page.rows().len()).filter(|&i| self.press.is_pressed(i)).collect()
    }

    pub(crate) fn fire(&mut self, row: usize, ctx: &mut Ctx) -> Transition {
        let Some(row) = self.page.rows().get(row).copied() else { return Transition::Stay };
        match row.setting {
            // **`Opt_ToggleFullScreen` (`0x00434B10`) on the only desktop this
            // engine runs on.** It leaves the panel before anything else —
            // `g_screenId = 0` on the campaign, `0x29` in a battle, which is
            // whatever the panel was opened over — and then, because
            // `Display_Init` has already forced `g_optFullScreen = 1` on any
            // desktop that is not 8bpp, takes its first branch:
            Setting::FullScreen => {
                let player = ctx.game.player;
                ctx.game.messages.enqueue(
                    crate::message::Record {
                        to: player,
                        from: 0,
                        group: FULL_SCREEN_REFUSAL,
                        variant: 0,
                        category: crate::message::category::NOTICE,
                        county: 0,
                        spare: 0,
                        payload: 0,
                    },
                    player,
                );
                Transition::Pop
            }
            Setting::StartGameHelp => Transition::Stay,
            other => {
                toggle(other, ctx);
                Transition::Stay
            }
        }
    }

    pub fn parent_hit() -> Rect {
        Rect::new(QUIRK_PARENT.0, QUIRK_PARENT.1, QUIRK_BOX, QUIRK_BOX)
    }

    pub fn quirk_hit(index: usize) -> Rect {
        Rect::new(QUIRK_BOX_X, QUIRK_LIST_Y + index as i32 * QUIRK_ROW_H, QUIRK_BOX, QUIRK_BOX)
    }

    pub fn quirk_at(count: usize, x: i32, y: i32) -> Option<usize> {
        (0..count).find(|i| OptionsScreen::quirk_hit(*i).contains(x, y))
    }
}

impl Screen for OptionsScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Options(self.page)
    }

    fn title(&self, ctx: &Ctx) -> String {
        match (self.page.group(), self.page.screen_id()) {
            (Some(g), Some(id)) => format!("{} — screen 0x{id:02X}", ctx.assets.shell.text(g, 0)),
            _ => "The original game's bugs — ours".to_string(),
        }
    }

    fn is_overlay(&self) -> bool {
        true
    }

    fn take_clicks(&mut self) -> u8 {
        self.press.take_clicks()
    }

    fn take_redraw(&mut self) -> bool {
        self.press.take_redraw()
    }

    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        for row in self.press.tick() {
            let t = self.fire(row, ctx);
            if t != Transition::Stay {
                return t;
            }
        }
        Transition::Stay
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        match event {
            // `Screen_FrameInput` (`0x0042FF10`) tests `g_mouseRightReleased`
            // first on all four of these ids, and `L2.eng` group 12 index 0
            // says it in English: *"Click Right to Exit"*.
            //
            // arm: 0x0042FF10/options-right-close right-release
            Event::RightClick { .. } => Transition::Pop,
            // arm: ours/options-keyboard-close key
            Event::KeyDown(Key::Escape) => Transition::Pop,

            Event::Pointer { x, y } => {
                if self.page.is_ours() {
                    let count = quirk_rows().len();
                    self.hover = OptionsScreen::quirk_at(count, x, y);
                    self.hover_parent = OptionsScreen::parent_hit().contains(x, y);
                } else {
                    self.hover = None;
                    self.hover_parent = false;
                    self.press.event(&self.page.widgets(), event);
                }
                Transition::Stay
            }
            Event::PointerLeft => {
                self.press.event(&self.page.widgets(), event);
                Transition::Stay
            }

            // **`Ui_OkButtonClicked` (`0x0040E7E4`), on the RELEASE**, which is
            // the second test of all four arms: `if (g_mouseLeftReleased == 0)
            // return 0;` and then the 24 × 24 box. Ours closed on the press.
            //
            // The quirks page is ours and closes the same way, so that one
            // corner picture
            // arm: 0x0040E7E4/options-ok left-release
            Event::Release { x, y } => {
                self.press.event(&self.page.widgets(), event);
                if self.page.close_hit().contains(x, y) {
                    return Transition::Pop;
                }
                Transition::Stay
            }

            Event::Click { .. } | Event::DoubleClick { .. } if !self.page.is_ours() => {
                let fired = self.press.event(&self.page.widgets(), event);
                debug_assert!(fired.is_none(), "every options row is kind 5");
                Transition::Stay
            }

            // arm: ours/options-quirks-page left-press
            Event::Click { x, y } => {
                if OptionsScreen::parent_hit().contains(x, y) {
                    let reproduced = quirk_group(ctx.game) == l2_net::Group::AllFixed;
                    set_all_quirks(reproduced, ctx.game);
                    return Transition::Stay;
                }
                let rows = quirk_rows();
                if let Some(i) = OptionsScreen::quirk_at(rows.len(), x, y) {
                    let now = quirk_reproduced(rows[i], ctx.game);
                    set_quirk(rows[i], !now, ctx.game);
                }
                Transition::Stay
            }

            _ => Transition::Stay,
        }
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let a = &ctx.assets.shell;
        let pen = Pen {
            assets: a,
            ink: &ctx.assets.ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: None,
        };

        let (bx, by, cols, rows, set) = self.page.window();
        pen.window(canvas, bx, by, cols, rows, set);

        if self.page.is_ours() {
            self.draw_quirks(ctx, canvas, &pen);
        } else {
            self.draw_panel(ctx, canvas, &pen);
        }
        let _ = (bx, by, cols, rows);

        let (cx, cy) = self.page.close_at();
        let drawn = ctx
            .assets
            .chrome
            .as_ref()
            .is_some_and(|c| c.draw_system(canvas, l2_view::chrome::system::OK, cx, cy));
        if !drawn {
            shell::button_recess(canvas, cx, cy, WIDGET, WIDGET);
        }
    }
}

impl OptionsScreen {
    pub(super) fn draw_panel(&self, ctx: &Ctx, canvas: &mut Canvas, pen: &Pen) {
        let a = &ctx.assets.shell;
        let group = self.page.group().expect("one of the original's four");
        let (hx, hy) = self.page.heading_at();
        let heading = a.text(group, 0).to_string();
        pen.heading(canvas, hx, hy, &heading, font::TEXT);

        let mut unsupported = false;
        for (i, row) in self.page.rows().iter().enumerate() {
            let label = a.text(group, row.label).to_string();
            let colour = if row.supported() { font::TEXT } else { font::DISABLED };
            pen.body(canvas, row.label_at.0, row.label_at.1, &label, colour);

            // The state word, out of group 18 or 19, at its own x. *Start game
            // help* is a button and has no state, so it prints none.
            if row.setting != Setting::StartGameHelp {
                let on = value(row.setting, ctx);
                let word = a.text(row.words.group(), row.words.index(on)).to_string();
                pen.body(canvas, row.state_x, row.label_at.1, &word, colour);
            }

            // `Widget_Draw`: the record's frame, plus one while `+0x0D` runs.
            let (wx, wy) = row.widget_at;
            let frame = WIDGET_FRAME + usize::from(self.press.is_pressed(i));
            if !pen.system_frame(canvas, frame, wx, wy) {
                shell::button_recess(canvas, wx, wy, WIDGET, WIDGET);
            }
            unsupported |= !row.supported();
        }

        if self.page == Page::Display {
            let note = a.text(group, DISPLAY_F5_NOTE).to_string();
            pen.body(
                canvas,
                DISPLAY_F5_NOTE_AT.0,
                DISPLAY_F5_NOTE_AT.1,
                &note,
                font::DISABLED,
            );
        }

        if unsupported {
            let (x, y, _, rows, _) = self.page.window();
            if ctx.game.prefs.debug_overlay {
            l2_view::text::draw(canvas, x, y + rows * 16 + 6, "GREYED: THIS ENGINE IS A WINDOW, AND L2HELP.HLP IS WIN3.1", ctx.assets.ink.dim);
            }
        }
    }

    fn draw_quirks(&self, ctx: &Ctx, canvas: &mut Canvas, pen: &Pen) {
        let ink = &ctx.assets.ink;
        let (hx, hy) = self.page.heading_at();
        pen.heading(canvas, hx, hy, "The original game's bugs", font::TEXT);

        let group = quirk_group(ctx.game);
        let (on, total) = quirk_tally(ctx.game);
        check_box(canvas, QUIRK_PARENT.0, QUIRK_PARENT.1, tri(group), ink, self.hover_parent);
        let parent = match group {
            l2_net::Group::AllReproduced => {
                format!("Reproducing all of them  ({on} of {total})")
            }
            l2_net::Group::AllFixed => format!("All turned off  ({on} of {total})"),
            l2_net::Group::Mixed => format!("Some of them  ({on} of {total})"),
        };
        pen.body(canvas, QUIRK_PARENT.0 + 20, QUIRK_PARENT.1 - 2, &parent, font::TEXT);

        for (i, row) in quirk_rows().iter().enumerate() {
            let y = QUIRK_LIST_Y + i as i32 * QUIRK_ROW_H;
            let reproduced = quirk_reproduced(*row, ctx.game);
            check_box(canvas, QUIRK_BOX_X, y, u8::from(reproduced), ink, self.hover == Some(i));
            let colour = if reproduced { font::TEXT } else { font::DISABLED };
            let line = format!("{}  {}", row.entry, summary_of(*row));
            pen.body(canvas, QUIRK_LIST_X, y - 2, &line, colour);
        }

        if ctx.game.prefs.debug_overlay {
        l2_view::text::draw(canvas, QUIRK_PARENT.0, 0x1C4, "OURS: THE ORIGINAL HAS NO SUCH PAGE. SEE DOCS/BUGS.MD", ink.dim);
        }
    }
}

fn check_box(canvas: &mut Canvas, x: i32, y: i32, state: u8, ink: &l2_view::Ink, hover: bool) {
    let edge = if hover { ink.highlight } else { ink.dim };
    canvas.fill_rect(x, y, QUIRK_BOX, QUIRK_BOX, edge);
    canvas.fill_rect(x + 1, y + 1, QUIRK_BOX - 2, QUIRK_BOX - 2, ink.background);
    match state {
        1 => canvas.fill_rect(x + 3, y + 3, QUIRK_BOX - 6, QUIRK_BOX - 6, ink.highlight),
        2 => canvas.fill_rect(x + 3, y + QUIRK_BOX / 2 - 1, QUIRK_BOX - 6, 2, ink.highlight),
        _ => {}
    }
}

