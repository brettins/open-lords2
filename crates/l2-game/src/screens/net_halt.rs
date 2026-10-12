//! A network game stopped (`docs/netcode.md` §8 *A dropped seat*, C240): what
//! happened, then Continue alone or Leave game. `[D]` ours: `L2.eng` has no group
//! for it (C243). At the confirm box's place, (0xA0, 0xA0) (`Ui_OpenConfirm`
//! `0x0040E6F2` from `Menu_Quit`), and it takes every event until answered.

use l2_view::Canvas;

use crate::input::{Event, Key, Rect};
use crate::net::AfterHalt;
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::setup::SetupPage;
use crate::shell::{self, font, Pen};

const X: i32 = 0xA0;
const Y: i32 = 0xA0;
const COLS: i32 = 20;
const INSET: i32 = 16;
const BUTTON_W: i32 = 136;
const BUTTON_H: i32 = 24;
const HEADING: &str = "The game has stopped";
const ALONE_TEXT: &str = "Continue alone and the computer plays their realm.";
pub const ALONE: &str = "Continue alone";
pub const LEAVE: &str = "Leave game";

#[derive(Default)]
pub struct NetHaltScreen {
    selected: usize,
}

impl NetHaltScreen {
    pub fn new() -> NetHaltScreen {
        NetHaltScreen::default()
    }

    /// Continue alone, then Leave game, under the text.
    pub fn buttons(ctx: &Ctx) -> [Rect; 2] {
        let (_, h) = layout(ctx);
        let y = Y + h - INSET - BUTTON_H;
        let w = COLS * 16;
        let gap = (w - 2 * BUTTON_W) / 3;
        [Rect::new(X + gap, y, BUTTON_W, BUTTON_H), Rect::new(X + 2 * gap + BUTTON_W, y, BUTTON_W, BUTTON_H)]
    }

    fn answer(&self, ctx: &mut Ctx, i: usize) -> Transition {
        if i == 0 {
            ctx.game.net.after_halt = Some(AfterHalt::Alone);
            Transition::Pop
        } else {
            // C257's leave: the app closes the session between ticks.
            ctx.game.net.after_halt = Some(AfterHalt::Leave);
            ctx.game.leaving = Some(crate::screens::leave::Leaving::Leave);
            Transition::Goto(ScreenId::Setup(SetupPage::Title))
        }
    }
}

fn pen<'a>(ctx: &'a Ctx) -> Pen<'a> {
    Pen {
        assets: &ctx.assets.shell,
        ink: &ctx.assets.ink,
        chrome: ctx.assets.chrome.as_ref(),
        shadow: Some(font::SHADOW),
        caps: None,
    }
}

fn line(ctx: &Ctx) -> i32 {
    ctx.assets.shell.body.as_ref().map_or(10, |f| f.line)
}

fn text(ctx: &Ctx) -> Vec<String> {
    let why = ctx.game.net.halted.clone().unwrap_or_default();
    let width = COLS * 16 - 2 * INSET;
    let p = pen(ctx);
    let mut lines = p.wrap(&why, width);
    lines.push(String::new());
    lines.extend(p.wrap(ALONE_TEXT, width));
    lines
}

/// The text's lines and the window's height, a whole number of 16-pixel rows.
fn layout(ctx: &Ctx) -> (Vec<String>, i32) {
    let lines = text(ctx);
    let body = INSET + 2 * line(ctx) + lines.len() as i32 * line(ctx) + 12 + BUTTON_H + INSET;
    (lines, (body + 15) / 16 * 16)
}

impl Screen for NetHaltScreen {
    fn id(&self) -> ScreenId {
        ScreenId::NetHalt
    }

    fn title(&self, _ctx: &Ctx) -> String {
        HEADING.into()
    }

    fn is_overlay(&self) -> bool {
        true
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        match event {
            Event::Click { x, y } => match NetHaltScreen::buttons(ctx).iter().position(|r| r.contains(x, y)) {
                Some(i) => self.answer(ctx, i),
                None => Transition::Stay,
            },
            Event::KeyDown(Key::Left) | Event::KeyDown(Key::Right) | Event::KeyDown(Key::Up) | Event::KeyDown(Key::Down) => {
                self.selected ^= 1;
                Transition::Stay
            }
            Event::KeyDown(Key::Enter) => self.answer(ctx, self.selected),
            _ => Transition::Stay,
        }
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let p = pen(ctx);
        let (lines, h) = layout(ctx);
        p.window(canvas, X, Y, COLS, h / 16, 1);
        let w = COLS * 16;
        p.body_centred(canvas, X, Y + INSET, w, HEADING, font::TEXT);
        let mut y = Y + INSET + 2 * line(ctx);
        for l in &lines {
            p.body(canvas, X + INSET, y, l, font::TEXT);
            y += line(ctx);
        }
        for (i, (r, label)) in NetHaltScreen::buttons(ctx).iter().zip([ALONE, LEAVE]).enumerate() {
            shell::button_recess(canvas, r.x, r.y, r.w, r.h);
            let ink = if i == self.selected { font::HIGHLIGHT } else { font::TEXT };
            p.body_centred(canvas, r.x, r.y + 5, r.w, label, ink);
        }
    }
}
