//! The help viewer: `L2help.hlp` in the game's own window. [D] CNEW-help-viewer —
//! `Opt_GameHelpContents` (`0x00434942`) calls `WinHelpA(hwnd, "l2help.hlp",
//! HELP_CONTENTS, 1)` and the viewer is Windows'; no function of the binary draws
//! one, so the frame, scroll arrows and OK are the save box's (`g_saveLoadWidgets`)
//! and Back is `L2.eng` 11/9. Scrolling is by the arrows only.

pub mod layout;

use l2_view::Canvas;

use self::layout::Row;
use crate::input::{Event, Rect};
use crate::press::{Kind, Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};

pub const BOX: (i32, i32, i32, i32) = (32, 16, 36, 28);
pub const HEADING: (i32, i32) = (64, 36);
pub const TEXT: (i32, i32, i32) = (64, 72, 448);
pub const ROWS: usize = 22;
pub const ROW_H: i32 = 16;
/// [D] the save box's step is a row of three; ours is three lines.
pub const STEP: usize = 3;
/// System.pl8 frames 35/37, the save box's scroll arrows (`SaveLoad_Scroll` 0x00434346).
pub const UP: (i32, i32, usize) = (528, 72, 35);
pub const DOWN: (i32, i32, usize) = (528, 104, 37);
pub const OK: (i32, i32) = (528, 424);
pub const BACK: Rect = Rect::new(60, 428, 64, 20);
/// `L2.eng` 11/9, the front end's *"Back"*.
pub const BACK_WORDS: (usize, usize) = (11, 9);

fn widgets() -> [Widget; 4] {
    [
        Widget::new(Rect::new(UP.0, UP.1, 24, 24), Kind::Repeat),
        Widget::new(Rect::new(DOWN.0, DOWN.1, 24, 24), Kind::Repeat),
        Widget::new(BACK, Kind::Press),
        Widget::new(Rect::new(OK.0, OK.1, 24, 24), Kind::Press),
    ]
}

pub struct Page {
    pub title: String,
    pub rows: Vec<Row>,
}

pub struct HelpScreen {
    offset: u32,
    back: Vec<u32>,
    top: usize,
    page: Option<Page>,
    press: Press,
}

impl HelpScreen {
    pub fn new() -> HelpScreen {
        HelpScreen { offset: 0, back: Vec::new(), top: 0, page: None, press: Press::default() }
    }

    /// The TOPICOFFSET on screen; `|SYSTEM` record 3 until a link is followed.
    pub fn offset(&self) -> u32 {
        self.offset
    }

    pub fn top(&self) -> usize {
        self.top
    }

    pub fn page(&mut self, ctx: &Ctx) -> Option<&Page> {
        if self.page.is_none() {
            let hlp = ctx.assets.help.as_ref()?;
            if self.back.is_empty() && self.offset == 0 {
                self.offset = hlp.contents;
            }
            let topic = hlp.topic_at_offset(self.offset)?;
            let body = ctx.assets.shell.body.as_ref();
            let measure = |s: &str| body.map_or_else(|| l2_view::text::width(s), |f| f.width(s));
            let rows = layout::layout(&topic, TEXT.2, &measure);
            self.page = Some(Page { title: topic.title, rows });
        }
        self.page.as_ref()
    }

    /// The hotspot hash under (x, y), if any.
    pub fn link_at(&mut self, ctx: &Ctx, x: i32, y: i32) -> Option<u32> {
        let top = self.top;
        let page = self.page(ctx)?;
        page.rows.iter().skip(top).take(ROWS).enumerate().find_map(|(i, row)| {
            let ry = TEXT.1 + i as i32 * ROW_H;
            row.iter()
                .find(|w| Rect::new(TEXT.0 + w.x, ry, w.w, ROW_H).contains(x, y))
                .and_then(|w| w.jump)
        })
    }

    pub fn follow(&mut self, ctx: &Ctx, hash: u32) -> bool {
        let Some(to) = ctx.assets.help.as_ref().and_then(|h| h.jump(hash)) else { return false };
        self.back.push(self.offset);
        self.go(to);
        true
    }

    fn go(&mut self, to: u32) {
        (self.offset, self.top, self.page) = (to, 0, None);
    }

    fn scroll(&mut self, ctx: &Ctx, down: bool) {
        let max = self.page(ctx).map_or(0, |p| p.rows.len().saturating_sub(ROWS));
        self.top = if down { (self.top + STEP).min(max) } else { self.top.saturating_sub(STEP) };
    }

    fn fire(&mut self, ctx: &Ctx, widget: usize) -> Transition {
        match widget {
            0 | 1 => self.scroll(ctx, widget == 1),
            2 => {
                if let Some(to) = self.back.pop() {
                    self.go(to);
                }
            }
            _ => return Transition::Pop,
        }
        Transition::Stay
    }
}

impl Default for HelpScreen {
    fn default() -> HelpScreen {
        HelpScreen::new()
    }
}

impl Screen for HelpScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Help
    }

    fn title(&self, _ctx: &Ctx) -> String {
        match &self.page {
            Some(p) => format!("Help — {}", p.title),
            None => "Help".into(),
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
        if ctx.assets.help.is_none() {
            return Transition::Pop;
        }
        for w in self.press.tick() {
            let t = self.fire(ctx, w);
            if t != Transition::Stay {
                return t;
            }
        }
        Transition::Stay
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        match event {
            Event::RightClick { .. } => Transition::Pop,
            Event::Click { x, y } | Event::DoubleClick { x, y } => {
                if let Some(i) = self.press.event(&widgets(), event) {
                    return self.fire(ctx, i);
                }
                if let Some(hash) = self.link_at(ctx, x, y) {
                    self.follow(ctx, hash);
                }
                Transition::Stay
            }
            Event::Release { .. } | Event::Pointer { .. } | Event::PointerLeft => {
                self.press.event(&widgets(), event);
                Transition::Stay
            }
            _ => Transition::Stay,
        }
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let pen = Pen {
            assets: &ctx.assets.shell,
            ink: &ctx.assets.ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: None,
        };
        pen.window(canvas, BOX.0, BOX.1, BOX.2, BOX.3, 0);
        let top = self.top;
        let can_back = !self.back.is_empty();
        if let Some(page) = self.page(ctx) {
            pen.heading(canvas, HEADING.0, HEADING.1, &page.title, font::TEXT);
            for (i, row) in page.rows.iter().skip(top).take(ROWS).enumerate() {
                for w in row {
                    let colour = if w.jump.is_some() { font::HIGHLIGHT } else { font::TEXT };
                    pen.body(canvas, TEXT.0 + w.x, TEXT.1 + i as i32 * ROW_H, &w.text, colour);
                }
            }
        }
        for (i, (x, y, frame)) in [UP, DOWN].into_iter().enumerate() {
            if !pen.system_frame(canvas, frame + usize::from(self.press.is_pressed(i)), x, y) {
                shell::button_recess(canvas, x, y, 24, 24);
            }
        }
        let colour = if can_back { font::TEXT } else { font::DISABLED };
        pen.eng(canvas, BACK_WORDS.0, BACK_WORDS.1, BACK.x + 4, BACK.y + 2, colour);
        pen.ok_button(canvas, OK.0, OK.1, 0);
    }
}
