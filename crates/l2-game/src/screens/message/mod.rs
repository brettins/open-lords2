//! **The message scroll** — `Msg_DrawWindow` (`0x0047309E`, 10,915 bytes) and
//! `Msg_HandleInput` (`0x0047685D`), the arm that runs before every other arm
//! in the game.
//!
//! ```c
//! Screen_HitRegion();
//! iVar2 = Msg_HandleInput();                                    /* 0x0047685D */
//! if ((iVar2 == 0) && (iVar2 = Screen_HandleInput(), iVar2 == 0)) {
//!     … the fifty per-screen arms …
//! }
//! ```
//!
//! * **Voice.** `Msg_PlayVoice` (`0x004B35C1`) is a `.wav` lookup and belongs to
//!   the audio branch, and it is built: [`crate::audio::voice_tick`] is the
//!   per-category schedule and [`crate::audio::Director`] fires it. The five
//!   timer values (`0x7C6`, `0x776`, `0x708`, `0x76C`, `0x5A`) were cited here
//!   as *"recorded in `crate::message`"* and **had never been written there** --
//!   a citation that did not resolve, which is a rule with no way in wearing a
//!   doc comment. They live beside `voice_tick` now, with the category each one
//!   belongs to, which was the half nobody had recorded. `docs/decisions.md`
//!   C126.
//!
//! * **The film.** Categories `0x0D` and `0x0E` play `cap_cty<n>.smk` and
//!   `FUN_00475B41`'s choice when `g_optAnimations` is on, and *dismiss
//!   themselves from inside the draw* to do it. This screen only notices and
//!   hands over — [`crate::message::animate`] decides, and
//!   [`crate::screens::movie`] draws the taller window and plays the film in
//!   it. With animations off the ordinary window below is the whole of it.

mod help;
use help::draw_help;
mod render;
pub use render::*;

use l2_view::Canvas;

use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::message::{self, category, Prompt, Record, Shape};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};

/// `FUN_004093E0(…, …, …, …)`'s border set. Every one of `Msg_DrawWindow`'s
/// arms passes the four-argument form, which is border set 1.
const BOX_SET: usize = 1;

/// `Faces.pl8` — the portrait beside a lord's letter, blitted at
/// `(x + 0x10, y + 0x12)` into a `Ui_DrawInsetRect(x + 0xF, y + 0x11, 0x52,
/// 0x4E)` well. `FUN_00475D73` picks the frame: `lord * 3 - 3`, **12 for a
/// human**, 16 for realm 0 and 17 for a realm above 5.
const FACES: &str = "Faces.pl8";
const FACE_AT: (i32, i32) = (0x10, 0x12);
const FACE_WELL: (i32, i32, i32, i32) = (0xF, 0x11, 0x52, 0x4E);

const SHIELD_BASE: usize = 0xFF;

/// **The peasants' own face**, for the county greeting — `Msg_DrawWindow`'s
/// category-`0x02` arm (`0x0047309E`):
///
/// ```c
/// if (*(int *)(&DAT_0053F9D4 + county * 0x300) < 0xF0) FUN_00475d73(6);
/// else                                                 FUN_00475d73(0);
/// ```
///
/// `DAT_0053F9D4` is the county's **population** (`g_levyMen = Pct(*(int
/// *)(&DAT_0053F9D4 + county * 0x300), pct)`, `0x00430BA2`), and neither
/// argument is a realm: they are the two ends of [`face_frame`]'s ladder, so
/// `6` lands on frame `0x11` and `0` on frame `0x10`. The letter carries one of
/// the two non-lord faces in `Faces.pl8`, picked by how many peasants wrote it.
///
/// **[V]** — from the decompiled arm; *which* of the two pictures is the fuller
/// county is inferred from the comparison alone.
pub fn peasant_face_frame(population: i32) -> usize {
    if population < 0xF0 { face_frame(0, false, 6) } else { face_frame(0, false, 0) }
}

/// `FUN_00475D73`'s frame ladder.
pub fn face_frame(lord: u8, is_human: bool, realm: u8) -> usize {
    if realm < 1 {
        return 16;
    }
    if realm > 5 {
        return 17;
    }
    if is_human {
        return 12;
    }
    (lord as usize).saturating_mul(3).saturating_sub(3)
}

pub struct MessageScreen {
    pointer: (i32, i32),
    /// **All five prompt tables are `Widget_Test` kind 4**, read out of `+0x0F`
    /// of `0x004DDA90`, `0x004DDAC0`, `0x004DDAF0`, `0x004DDB20` and
    /// `0x004DDB50` — the same pair of mailed hands as the yes/no box and a
/// *different kind*, so the kind has to be read
    /// inferred from the picture. The repeat is unreachable: every one of the
    /// five handlers calls `Msg_Dismiss` first, so the table is gone before a
    /// second fire could come. What kind 4 buys is the pressed picture.
    press: Press,
}

/// **Each prompt is its own table with its own kind byte**, so each carries its
/// own `arm!` — the marker for the handler behind it, and the kind it is
/// answered with. The two ally answers share `FUN_004368FD`'s record.
fn prompt_widgets(prompt: Prompt) -> [Widget; 2] {
    let [yes, no] = prompt.widgets();
    let side = Prompt::SIDE;
    let kind = match prompt {
        // `Diplo_PayHelpClicked`, `0x004DDA90`.
        Prompt::PayForHelp => crate::arm!("0x004367FF/pay-for-help-prompt", Repeat),
        // `FUN_00436872`, `0x004DDAC0`.
        Prompt::AcceptAlliance => crate::arm!("0x00436872/accept-alliance-prompt", Repeat),
        // `FUN_004368FD` at `0x004DDAF0` and `FUN_0043695D` at `0x004DDB20`.
        Prompt::AnswerHelpRequest | Prompt::AnswerAttackRequest => {
            crate::arm!("0x004368FD/answer-help-request", Repeat)
        }
        // `FUN_004376BB`, `0x004DDB50`.
        Prompt::Garrison => crate::arm!("0x004376BB/garrison-split-prompt", Repeat),
    };
    [
        Widget::new(Rect::new(yes.0, yes.1, side, side), kind),
        Widget::new(Rect::new(no.0, no.1, side, side), kind),
    ]
}

impl MessageScreen {
    pub fn new() -> MessageScreen {
        MessageScreen { pointer: (320, 240), press: Press::new() }
    }

    pub(crate) fn record(ctx: &Ctx) -> Option<Record> {
        ctx.game.messages.open().copied()
    }
}

impl Default for MessageScreen {
    fn default() -> MessageScreen {
        MessageScreen::new()
    }
}

impl Screen for MessageScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Message
    }

    fn take_clicks(&mut self) -> u8 {
        self.press.take_clicks()
    }

    fn take_redraw(&mut self) -> bool {
        self.press.take_redraw()
    }

    fn title(&self, ctx: &Ctx) -> String {
        match MessageScreen::record(ctx) {
            Some(r) => format!("Message {} — category {:#04x}", r.group, r.category),
            None => "Message".into(),
        }
    }

    fn is_overlay(&self) -> bool {
        true
    }

    /// **`Msg_HandleInput` (`0x0047685D`), in its own order.**
    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        let Some(record) = MessageScreen::record(ctx) else { return Transition::Pop };
        match event {
            // arm: 0x0047685D/message-scroll-dismiss right-release
            Event::RightClick { .. } => leave(ctx),
            Event::Click { x, y } => {
                if let Some(prompt) = record.answer_widgets() {
                    if let Some(i) = self.press.event(&prompt_widgets(prompt), event) {
                        return answer(ctx, prompt, i == 0);
                    }
                }
                let shape = record.shape();
                if shape.has_ok_button() {
                    if let Some(frame) = window_frame(ctx, &record) {
                        if frame.ok_hitbox().contains(x, y) {
                            // `FUN_004B18E3()` consumes the click so the screen
                            // underneath cannot also act on it, then dismisses.
                            //
                            // arm: 0x0047685D/message-ok-dismiss left-press
                            return leave(ctx);
                        }
                    }
                }
                Transition::Pass
            }
            // arm: ours/message-keyboard-dismiss key
            Event::KeyDown(Key::Escape) | Event::KeyDown(Key::Enter) => leave(ctx),
            // The five `Widget_Test` tables are kind 4, whose guard is
            // `g_mouseLeftPressed || g_mouseLeftDoubleClick`, and a hit returns
            // 1 and swallows the frame. The 48 × 48 corner opens
            // `if (g_mouseLeftPressed == 0) { uVar1 = 0; }`
            // anywhere else returns 0 and `Screen_FrameInput` offers it to the
            // screen underneath. `[V]` This screen passed every double click
            // down, prompt or not.
            Event::DoubleClick { .. } => {
                if let Some(prompt) = record.answer_widgets() {
                    if let Some(i) = self.press.event(&prompt_widgets(prompt), event) {
                        return answer(ctx, prompt, i == 0);
                    }
                }
                Transition::Pass
            }
            Event::Pointer { x, y } => {
                self.pointer = (x, y);
                if let Some(prompt) = record.answer_widgets() {
                    self.press.event(&prompt_widgets(prompt), event);
                }
                Transition::Pass
            }
            Event::Release { .. } => {
                self.press.release();
                Transition::Pass
            }
            _ => Transition::Pass,
        }
    }

    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        let _ = self.press.tick();
        if !ctx.game.messages.is_open() {
            return Transition::Pop;
        }
        if !message::show(ctx.game) {
            if ctx.game.campaign.outcome.is_over() {
                return Transition::Replace(ScreenId::Conquest);
            }
            return Transition::Pop;
        }
        // arm: 0x0047309E/capture-smacker draw
        if let Some(film) = message::animate(ctx.game) {
            return Transition::Replace(ScreenId::Movie(film));
        }
        Transition::Stay
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let Some(record) = MessageScreen::record(ctx) else { return };
        let pen = Pen {
            assets: &ctx.assets.shell,
            ink: &ctx.assets.ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: None,
        };
        if let Shape::Paragraphs(n) = record.shape() {
            draw_paragraphs(&pen, ctx, canvas, &record, n);
            return;
        }
        let Some(frame) = message::frame_of(&record) else {
            // A category with no geometry of its own — the tip, or one
            // `Msg_DrawWindow` has no arm for. The tip is the only one of them
            // anything enqueues; the help window's four numbers now come out of
            // `g_helpWindowGeom` (`0x004D6EB8`) above.
            draw_tip(&pen, ctx, canvas, &record, self.pointer);
            return;
        };
        pen.window(canvas, frame.x, frame.y, frame.w / 16, frame.h / 16, BOX_SET);

        match record.shape() {
            Shape::Letter | Shape::Prompt => draw_letter(&pen, ctx, canvas, &record, frame),
            Shape::CountyPortrait => draw_county_portrait(&pen, ctx, canvas, &record, frame),
            Shape::Ending => draw_ending(&pen, ctx, canvas, &record, frame),
            Shape::Garrison => draw_garrison(&pen, ctx, canvas, &record, frame),
            Shape::Event => draw_event(&pen, ctx, canvas, &record, frame),
            Shape::Capture => draw_capture(&pen, ctx, canvas, &record, frame),
            // `Msg_DrawWindow`'s category-0x13 arm; `Menu_HelpHowDoI`
            // (`0x0043480C`) and its four siblings post the records.
            Shape::Help => draw_help(&pen, ctx, canvas, &record, frame),
            _ => draw_notice(&pen, ctx, canvas, &record, frame),
        }

        if let Some(prompt) = record.answer_widgets() {
            draw_prompt(&pen, canvas, prompt, &self.press);
        }
        if record.shape().has_ok_button() {
            let (x, y) = frame.ok_button();
            pen.ok_button(canvas, x, y, 0);
        }
    }
}

fn leave(ctx: &mut Ctx) -> Transition {
    match message::dismiss(ctx.game) {
        message::Dismissal::GameOver(_) => Transition::Replace(ScreenId::Conquest),
        _ => Transition::Pop,
    }
}

fn answer(ctx: &mut Ctx, prompt: Prompt, yes: bool) -> Transition {
    match prompt {
        // `Diplo_PayHelpClicked` (`0x004367FF`):
        Prompt::PayForHelp => {
            message::dismiss(ctx.game);
            if yes {
                let me = ctx.game.player;
                let ally = ctx.game.kingdom.realms[me as usize].ally;
                let county = ctx.game.kingdom.diplomacy.help_county;
                let price = ctx.game.kingdom.diplomacy.help_price;
                l2_kingdom::diplomacy::pay_for_help(
                    &mut ctx.game.kingdom.realms,
                    ally,
                    me,
                    county,
                    price,
                );
            }
            Transition::Pop
        }
        // `FUN_00436872` (`0x00436872`):
        Prompt::AcceptAlliance => {
            let offerer = ctx.game.messages.open().map_or(0, |r| r.from);
            message::dismiss(ctx.game);
            if yes && offerer != 0 {
                let me = ctx.game.player;
                l2_kingdom::diplomacy::form_alliance(&mut ctx.game.kingdom.realms, me, offerer);
            }
            Transition::Pop
        }
        // `FUN_004368FD` / `FUN_0043695D` — the *ally's* answer to a request the
        // player made. Both set `g_diploKind` to one of two values by hotspot
        // and post it: 7/8 for help, 9/10 for attack. Those four kinds are
        // beyond `l2_kingdom::diplomacy::Kind`'s seven, which stops at 6 — they
        // are network commands and not composer kinds, and `Net_SendCommand`
        // (`0x49`) is their only consumer. In a single-player game the two
        // handlers therefore do nothing but dismiss.
        Prompt::AnswerHelpRequest | Prompt::AnswerAttackRequest => {
            message::dismiss(ctx.game);
            Transition::Pop
        }
        // `FUN_004376BB` (`0x004376BB`) — *"Cannot garrison castle."*
        Prompt::Garrison => {
            if !yes {
                return Transition::Pass;
            }
            let unit = ctx.game.messages.open().map_or(0, |r| r.variant as usize);
            message::dismiss(ctx.game);
            if unit != 0 && ctx.game.kingdom.campaign.units.get(unit).is_some() {
                // `g_screenId = 0x11` — and `DivideScreen` seeds its own basket
                // from the army the first time it is drawn, which is
                // `FUN_004378B3`'s seeding that `FUN_004376BB` duplicates.
                return Transition::Replace(ScreenId::Divide(unit));
            }
            Transition::Pop
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `FUN_00475D73`'s ladder, which is four cases and not one.
    #[test]
    fn a_human_rival_shows_the_twelfth_face_and_a_lord_shows_his_own() {
        assert_eq!(face_frame(1, false, 3), 0);
        assert_eq!(face_frame(2, false, 3), 3);
        assert_eq!(face_frame(4, false, 3), 9);
        assert_eq!(face_frame(2, true, 3), 12, "a human rival");
        assert_eq!(face_frame(2, false, 0), 16, "realm 0 is the game itself");
        assert_eq!(face_frame(2, false, 6), 17, "and 6 is the merchant");
    }
}

