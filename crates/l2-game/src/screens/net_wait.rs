//! C260: a network game waits for a dropped player. Every seat sees who and why; the
//! host's way on is *Let the computer play*, a joiner's is to leave. `[D]` ours:
//! `L2.eng` has no group for it (C243). The leave box's frame (`ExitConfirm_Draw`
//! `0x00414790`, [`super::leave::BOX`]), grown to its text; it takes every event,
//! Escape too, and `Machine::keep_halt_on_top` holds it over anything opened under it.

use l2_view::Canvas;

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::leave::{carry_out, Choice};
use crate::shell::{self, font, Pen};

const X: i32 = 0x80;
const Y: i32 = 0xA0;
const COLS: i32 = 0x12;
const INSET: i32 = 0x10;
const FOOT: i32 = 0x18;
const TEXT_W: i32 = COLS * 16 - 2 * INSET;
const BUTTON_H: i32 = 0x18;
const ROW_STEP: i32 = 0x1C;
pub const COMPUTER: &str = "Let the computer play";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    ComputerPlays,
    Then(Choice),
}

impl Answer {
    pub fn label(self) -> &'static str {
        match self {
            Answer::ComputerPlays => COMPUTER,
            Answer::Then(c) => c.label(),
        }
    }
}

#[derive(Default)]
pub struct NetWaitScreen {
    selected: usize,
}

impl NetWaitScreen {
    pub fn new() -> NetWaitScreen {
        NetWaitScreen::default()
    }

    /// `[D]` the host keeps the game or ends it; a joiner can only go.
    pub fn answers(ctx: &Ctx) -> Vec<Answer> {
        if ctx.game.net.hosting {
            vec![Answer::ComputerPlays, Answer::Then(Choice::SaveAndEnd)]
        } else {
            vec![Answer::Then(Choice::Leave)]
        }
    }

    pub fn heading(ctx: &Ctx) -> String {
        format!("Waiting for {}", names(ctx))
    }

    pub fn text(ctx: &Ctx) -> String {
        let who = names(ctx);
        let rest = if ctx.game.net.hosting {
            format!("The game is paused until they join again with the same code or address.")
        } else {
            let host = ctx.game.net.roster.players.iter().find(|p| p.is_host).map_or("The host".to_string(), |p| p.name.clone());
            format!("The game is paused until they join again, or {host} lets the computer play.")
        };
        format!("{who} lost the connection. {rest}")
    }

    pub fn buttons(ctx: &Ctx) -> Vec<Rect> {
        let (_, h) = layout(ctx);
        let n = NetWaitScreen::answers(ctx).len() as i32;
        let top = Y + h - FOOT - (n - 1) * ROW_STEP - BUTTON_H;
        (0..n).map(|i| Rect::new(X + INSET, top + i * ROW_STEP, TEXT_W, BUTTON_H)).collect()
    }

    fn press(&mut self, ctx: &mut Ctx, i: usize) -> Transition {
        match NetWaitScreen::answers(ctx).get(i) {
            Some(Answer::ComputerPlays) => {
                ctx.game.net.computer_plays = true;
                Transition::Stay
            }
            Some(Answer::Then(c)) => carry_out(*c, ctx),
            None => Transition::Stay,
        }
    }
}

fn names(ctx: &Ctx) -> String {
    ctx.game.net.waiting.join(" and ")
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

/// The body's lines and the window's height, a whole number of 16-pixel rows.
fn layout(ctx: &Ctx) -> (Vec<String>, i32) {
    let lines = pen(ctx).wrap(&NetWaitScreen::text(ctx), TEXT_W);
    let n = NetWaitScreen::answers(ctx).len() as i32;
    let body = INSET + 2 * line(ctx) + lines.len() as i32 * line(ctx) + 12 + (n - 1) * ROW_STEP + BUTTON_H + FOOT;
    (lines, (body + 15) / 16 * 16)
}

impl Screen for NetWaitScreen {
    fn id(&self) -> ScreenId {
        ScreenId::NetWait
    }

    fn title(&self, ctx: &Ctx) -> String {
        NetWaitScreen::heading(ctx)
    }

    fn is_overlay(&self) -> bool {
        true
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        let n = NetWaitScreen::answers(ctx).len();
        match event {
            Event::Click { x, y } => match NetWaitScreen::buttons(ctx).iter().position(|r| r.contains(x, y)) {
                Some(i) => self.press(ctx, i),
                None => Transition::Stay,
            },
            Event::Pointer { x, y } => {
                if let Some(i) = NetWaitScreen::buttons(ctx).iter().position(|r| r.contains(x, y)) {
                    self.selected = i;
                }
                Transition::Stay
            }
            Event::KeyDown(Key::Up) | Event::KeyDown(Key::Down) => {
                self.selected = (self.selected + 1) % n;
                Transition::Stay
            }
            Event::KeyDown(Key::Enter) => self.press(ctx, self.selected.min(n - 1)),
            _ => Transition::Stay,
        }
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let p = pen(ctx);
        let (lines, h) = layout(ctx);
        p.window(canvas, X, Y, COLS, h / 16, 1);
        p.body_centred(canvas, X, Y + INSET, COLS * 16, &NetWaitScreen::heading(ctx), font::TEXT);
        let mut y = Y + INSET + 2 * line(ctx);
        for l in &lines {
            p.body(canvas, X + INSET, y, l, font::TEXT);
            y += line(ctx);
        }
        let flat = p.flat();
        let answers = NetWaitScreen::answers(ctx);
        for (i, (a, r)) in answers.iter().zip(NetWaitScreen::buttons(ctx)).enumerate() {
            shell::button_recess(canvas, r.x, r.y, r.w, r.h);
            let ink = if i == self.selected { font::HIGHLIGHT } else { font::TEXT };
            flat.body_centred(canvas, r.x, r.y + 5, r.w, a.label(), ink);
        }
    }
}
