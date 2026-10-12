//! C257: leaving a network game. A joiner's *Leave game* hands its realm to the
//! computer; the host's *End game for everyone* offers a save first. The box is
//! `ExitConfirm_Draw`'s (`0x00414790`): `0x12 × 9` cells at (0x80, 0xA0), lines at
//! x 0x90, the three-line shape of `L2.eng` group 48 (*"Exit the game?"* / *"Save the
//! game and exit ?"* / *"Return to the game ?"*), which no caller reaches. `[D]` the
//! words are ours: group 48 says "exit", and this ends a game for others too.
//! `FUN_0043441C`'s network arm, `Net_LeaveGameChecked` then `g_screenId = 0x1F;
//! g_setupPage = 1`, is where the yes lands: the title page.

use l2_view::Canvas;

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::setup::SetupPage;
use crate::shell::{self, font, Pen};

pub const BOX: (i32, i32, i32, i32) = (0x80, 0xA0, 0x12, 9);
const TEXT_X: i32 = 0x90;
const TEXT_W: i32 = 0x12 * 16 - 0x20;
const HEAD_Y: i32 = 0xB8;
const BUTTON_H: i32 = 0x18;
const ROWS_Y: i32 = 0xD8;
const ROW_STEP: i32 = 0x1C;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leaving {
    Leave,
    EndForEveryone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    SaveAndEnd,
    EndWithoutSaving,
    Leave,
    Back,
}

impl Choice {
    pub fn label(self) -> &'static str {
        match self {
            Choice::SaveAndEnd => "Save, then end the game",
            Choice::EndWithoutSaving => "End without saving",
            Choice::Leave => "Leave the game",
            Choice::Back => "Back to the game",
        }
    }
}

pub struct LeaveScreen {
    selected: usize,
    redraw: bool,
}

impl LeaveScreen {
    pub fn new() -> LeaveScreen {
        LeaveScreen { selected: 0, redraw: true }
    }

    pub fn choices(ctx: &Ctx) -> Vec<Choice> {
        if ctx.game.net.hosting {
            vec![Choice::SaveAndEnd, Choice::EndWithoutSaving, Choice::Back]
        } else {
            vec![Choice::Leave, Choice::Back]
        }
    }

    pub fn lines(ctx: &Ctx) -> Vec<&'static str> {
        if ctx.game.net.hosting {
            vec!["End the game for everyone?"]
        } else {
            vec!["Leave the game?", "The computer will play your realm."]
        }
    }

    pub fn rects(ctx: &Ctx) -> Vec<Rect> {
        let n = LeaveScreen::choices(ctx).len() as i32;
        let top = ROWS_Y + (3 - n) * ROW_STEP;
        (0..n).map(|i| Rect::new(TEXT_X, top + i * ROW_STEP, TEXT_W, BUTTON_H)).collect()
    }

    fn press(&mut self, choice: Choice, ctx: &mut Ctx) -> Transition {
        carry_out(choice, ctx)
    }
}

/// C257's answers; C260's waiting box gives the same ones.
pub fn carry_out(choice: Choice, ctx: &mut Ctx) -> Transition {
    let leaving = match choice {
        Choice::Back => return Transition::Pop,
        Choice::Leave => Leaving::Leave,
        Choice::EndWithoutSaving => Leaving::EndForEveryone,
        Choice::SaveAndEnd => {
            let name = save_name(ctx.game);
            if let Err(e) = crate::saves::write(&name, ctx.game) {
                eprintln!("save before ending: {e}");
            }
            Leaving::EndForEveryone
        }
    };
    ctx.game.leaving = Some(leaving);
    Transition::Goto(ScreenId::Setup(SetupPage::Title))
}

impl Default for LeaveScreen {
    fn default() -> LeaveScreen {
        LeaveScreen::new()
    }
}

/// `[D]` C262: "net", the season and the year, narrow enough for one column of the
/// load list (`SaveLoad_DrawStatus` `0x004149EC`, columns `0x78` apart).
pub fn save_name(game: &crate::Game) -> String {
    let season: String = crate::screens::map::season_name(game.kingdom.season).chars().take(3).collect();
    let name = format!("net {season} {}", game.kingdom.year);
    if crate::saves::is_valid_name(&name) { name } else { "net game".into() }
}

impl Screen for LeaveScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Leave
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "Leave the game".into()
    }

    fn is_overlay(&self) -> bool {
        true
    }

    fn take_redraw(&mut self) -> bool {
        core::mem::take(&mut self.redraw)
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        let choices = LeaveScreen::choices(ctx);
        let n = choices.len();
        self.redraw = true;
        match event {
            Event::KeyDown(Key::Escape) | Event::RightClick { .. } => Transition::Pop,
            Event::KeyDown(Key::Up) => {
                self.selected = (self.selected + n - 1) % n;
                Transition::Stay
            }
            Event::KeyDown(Key::Down) => {
                self.selected = (self.selected + 1) % n;
                Transition::Stay
            }
            Event::KeyDown(Key::Enter) => self.press(choices[self.selected.min(n - 1)], ctx),
            Event::Pointer { x, y } => {
                if let Some(i) = LeaveScreen::rects(ctx).iter().position(|r| r.contains(x, y)) {
                    self.selected = i;
                }
                Transition::Stay
            }
            Event::Click { x, y } => match LeaveScreen::rects(ctx).iter().position(|r| r.contains(x, y)) {
                Some(i) => self.press(choices[i], ctx),
                None => Transition::Stay,
            },
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
        let (x, y, cols, rows) = BOX;
        pen.window(canvas, x, y, cols, rows, 1);
        for (i, line) in LeaveScreen::lines(ctx).iter().enumerate() {
            pen.body(canvas, TEXT_X, HEAD_Y + 14 * i as i32, line, font::TEXT);
        }
        let flat = pen.flat();
        for (i, (c, r)) in LeaveScreen::choices(ctx).iter().zip(LeaveScreen::rects(ctx)).enumerate() {
            shell::button_recess(canvas, r.x, r.y, r.w, r.h);
            let ink = if i == self.selected { font::HIGHLIGHT } else { font::TEXT };
            flat.body_centred(canvas, r.x, r.y + 5, r.w, c.label(), ink);
        }
    }
}
