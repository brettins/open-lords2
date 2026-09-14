//! **The message scroll** — `Msg_DrawWindow` (`0x0047309E`, 10,915 bytes) and
//! `Msg_HandleInput` (`0x0047685D`), the arm that runs before every other arm
//! in the game.
//!
//! # It is a screen here
//!
//! `g_screenId` does not change when a message opens. The window is painted
//! over whatever was up by `Msg_Pump`, which `Battle_Frame` calls once a frame,
//! and its input is the **first** thing `Screen_FrameInput` does:
//!
//! ```c
//! Screen_HitRegion();
//! iVar2 = Msg_HandleInput();                                    /* 0x0047685D */
//! if ((iVar2 == 0) && (iVar2 = Screen_HandleInput(), iVar2 == 0)) {
//!     … the fifty per-screen arms …
//! }
//! ```
//!
//! Our machine says that with an **overlay** on top of the stack: an overlay
//! draws over what is beneath it, and [`crate::screen::Machine::handle`] offers
//! an event to the top screen first. The one place the analogy has to be exact
//! is the *fall-through*: `Msg_HandleInput` returning zero is a click that
//! reaches the screen underneath, and that is [`Transition::Pass`]. A left click
//! that misses the corner button and misses the answer widgets is **not
//! consumed** — see [`MessageScreen::handle`].
//!
//! # What the window is, arm by arm
//!
//! Twenty categories, enumerated in [`crate::message::category`]. Nine have a
//! layout of their own here and the rest share the plain one; every one of them
//! draws `Ui_OkButton` in the same corner — `(x + w - 0x30, y + h - 0x30)` —
//! except the floating tip, which draws no button at all and cannot be closed
//! by hand.
//!
//! # The three input arms and the five answers
//!
//! `Msg_HandleInput`, in the original's own order, which is not the order a
//! reader expects:
//!
//! 1. **a right release closes it, whatever it is** — tested *before* the
//!    widgets, so right-clicking an alliance offer is neither yes nor no;
//! 2. **five widget tests**, one per prompt, each running its handler;
//! 3. **a left press in the 48 × 48 box** round the corner button.
//!
//! Everything else returns zero and falls through.
//!
//! # What this screen deliberately does not do
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

/// `Pl8_DrawFrame(g_panelsSheet, shieldIndex + 0xFF, x + w - 0x1E, y + 0x12)`.
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

/// **The message scroll.**
///
/// It holds no state: the record, the timer and the ring are all on
/// [`crate::Game`],
/// because the window is opened by the frame driver and not by anything the
/// player did.
pub struct MessageScreen {
    /// **Where the pointer was**, for the one layout that needs it: the
    /// floating tip is placed at `g_mouseX`/`g_mouseY`, which the original reads
    /// straight out of the globals the window procedure writes.
    ///
/// Tracked here because nothing else in this
    /// workspace wants it and a cursor position on the world is a cursor
    /// position in the save. It starts at the middle of the screen, which is
    /// where the tip's own clamp puts it anyway if the pointer has not moved.
    pointer: (i32, i32),
    /// The open prompt's press timer.
    ///
    /// **All five prompt tables are `Widget_Test` kind 4**, read out of `+0x0F`
    /// of `0x004DDA90`, `0x004DDAC0`, `0x004DDAF0`, `0x004DDB20` and
    /// `0x004DDB50` — the same pair of mailed hands as the yes/no box and a
/// *different kind*, so the kind has to be read
    /// inferred from the picture. The repeat is unreachable: every one of the
    /// five handlers calls `Msg_Dismiss` first, so the table is gone before a
    /// second fire could come. What kind 4 buys is the pressed picture.
    press: Press,
}

/// The open prompt's two widgets as a table. Index 0 is **yes**, hotspot id 1.
///
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

    /// The record on screen, or `None` for the one frame the machine may still
    /// draw this after the queue closed it.
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

    /// `Widget_Test`'s `Sound_RestartSlot(1)`, carried up to the audio
    /// layer. See [`Screen::take_clicks`].
    fn take_clicks(&mut self) -> u8 {
        self.press.take_clicks()
    }

    /// The prompt's thumb coming back up. See [`Press::take_redraw`].
    fn take_redraw(&mut self) -> bool {
        self.press.take_redraw()
    }

    fn title(&self, ctx: &Ctx) -> String {
        match MessageScreen::record(ctx) {
            Some(r) => format!("Message {} — category {:#04x}", r.group, r.category),
            None => "Message".into(),
        }
    }

    /// It paints a box over whatever raised it and never clears.
    fn is_overlay(&self) -> bool {
        true
    }

    /// **`Msg_HandleInput` (`0x0047685D`), in its own order.**
    ///
    /// The three things worth noticing
    /// would guess from the function's shape:
    ///
    /// * the **right** release is tested before the widgets, so it closes an
///   unanswered question;
    /// * the corner button's hit box is **48 × 48**, twice the picture, while
    /// every other screen in the game uses `Ui_OkButtonClicked`'s 24 × 24;
    /// * anything else **falls through** to the screen underneath, which is how
    /// `Map_Click`'s dismissal and the campaign sidebar both stay live with a
    ///   message up.
    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        let Some(record) = MessageScreen::record(ctx) else { return Transition::Pop };
        match event {
            // `else { Msg_Dismiss(); return 1; }` — the whole of the
            // right-button branch, with no category test in front of it.
            // arm: 0x0047685D/message-scroll-dismiss right-release
            Event::RightClick { .. } => leave(ctx),
            Event::Click { x, y } => {
                // The five `Widget_Test` calls, in the original's order:
                // category 0x11, then 10, then 0x0B, then the three groups of
                // category 0x0C. Each returns 1 whether or not the click was on
                // a button.
                if let Some(prompt) = record.answer_widgets() {
                    if let Some(i) = self.press.event(&prompt_widgets(prompt), event) {
                        return answer(ctx, prompt, i == 0);
                    }
                    // `Widget_Test` returning 0 falls on through to the corner
// button below
                    // without answering it.
                }
                let shape = record.shape();
                if shape.has_ok_button() {
                    // A tip window's corner is wherever its wrapped text put it,
                    // so the hit box is computed from the same text the draw
                    // wraps. Before the tips existed this was `frame_of`, which
                    // has no row for `0x05`…`0x09` — a tip could not be closed
                    // with the left button at all.
                    if let Some(frame) = window_frame(ctx, &record) {
                        if frame.ok_hitbox().contains(x, y) {
                            // `FUN_004B18E3()` consumes the click so the screen
                            // underneath cannot also act on it, then dismisses.
                            // arm: 0x0047685D/message-ok-dismiss left-press
                            return leave(ctx);
                        }
                    }
                }
                // **Zero: not mine.** `Screen_HandleInput` and then the
                // per-screen arms get this click. On the campaign map that is
                // `Map_Click`, whose whole body is skipped and whose `else` is
                // `Msg_DismissUnlessQuestion`.
                Transition::Pass
            }
            // **Ours.** `Msg_HandleInput` tests no key at all
            // procedure has no arm for one either. A demo that can be driven
            // from the keyboard is worth more than the omission is faithful,
// and this is counted.
            // arm: ours/message-keyboard-dismiss key
            Event::KeyDown(Key::Escape) | Event::KeyDown(Key::Enter) => leave(ctx),
            // **A double click answers a prompt and does nothing else here.**
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

    /// **The draw's own side effects** — the tip clamp and the three arms
    /// `Msg_DrawWindow` runs on the frame the window opens.
    ///
    /// The **timer** is not here: it is `Msg_Pump`'s, and it lives in
    /// [`crate::screen::Machine::pump_messages`] beside the pull it is exclusive
    /// with. That split is the original's and it matters — see that function.
    ///
    /// **In the original all of this is in the draw.** It cannot be here:
    /// [`Screen::draw`] takes a `&Ctx`, which is the compiler enforcing that
    /// painting a frame cannot change the world (`crates/l2-game/src/screen/mod.rs`,
    /// *Draw cannot mutate*). The original's draw and input run once each per
    /// frame in a fixed order, so moving these three arms into `update` changes
    /// nothing about when they fire; it is recorded because it is a difference.
    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        // `Widget_Test`'s countdown, which runs the pressed picture down. It
        // cannot fire: the prompts are kind 4, whose fire is on the press, and
        // the handler dismisses before a repeat could arrive.
        let _ = self.press.tick();
        if !ctx.game.messages.is_open() {
            return Transition::Pop;
        }
        // `Msg_DrawWindow`'s `g_messageTimer == 2000` arms, one of which can
        // close the window and one of which can end the game.
        if !message::show(ctx.game) {
            // The two arms that reach here are the alliance lapses, which cannot
            // set an outcome — but the test is on the outcome and not on the
            // category, because that is what `Msg_Dismiss` tests.
            if ctx.game.campaign.outcome.is_over() {
                return Transition::Replace(ScreenId::Conquest);
            }
            return Transition::Pop;
        }
        // The animated capture and ending: the window closes itself and a film
        // plays where it was. See `message::animate`.
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

/// **`Msg_Dismiss` and what it asks for.**
///
/// Its last three lines are the ending — `if (outcome == 10 || outcome == 11)
/// { Campaign_EnterConquest(); g_screenId = 0x1C; }`
/// sometimes a screen change and not a pop. Every arm that closes the window
/// goes through here, which is what stops one of them from forgetting.
fn leave(ctx: &mut Ctx) -> Transition {
    match message::dismiss(ctx.game) {
        message::Dismissal::GameOver(_) => Transition::Replace(ScreenId::Conquest),
        _ => Transition::Pop,
    }
}

/// **The five widget handlers**, which are the only two places a person answers
/// a lord and three more besides.
///
/// Every one of them calls `Msg_Dismiss` **first** and then acts on
/// `g_uiHotspotId`, so the window is gone before the rule runs — so
/// none of them has anything to say about a refusal.
fn answer(ctx: &mut Ctx, prompt: Prompt, yes: bool) -> Transition {
    match prompt {
        // `Diplo_PayHelpClicked` (`0x004367FF`):
        //   Msg_Dismiss();
        //   if (hotspot != 0) Diplo_PayForHelp(myAlly, me, g_diploHelpCounty, g_diploHelpPrice);
        // **Declining does nothing at all** — not even a letter back.
        // The five arms are declared on [`prompt_widgets`].
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
        //   Msg_Dismiss();
        //   if (realms[offerer].isHuman || hotspot != 0)
        //       if (hotspot == 1) Diplo_FormAlliance(g_localPlayer, offerer);
        //
        // **The guard is the finding.** Declining an AI's offer in single player
        // runs *nothing*: no refusal, no grudge, no letter. The offer lapses
        // when the offering realm clears `offer_pending` on its next turn.
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
        //
        //   g_screenId = 0; g_redrawRequest = 2;          /* BEFORE the test */
        //   if (hotspot != 0) { … seed the basket …; Msg_Dismiss(); g_screenId = 0x11; }
        //
// The handler returns to the campaign map
        // without closing the window, so the prompt is still up and has to be
        // closed with the corner button or the right button. Reproduced;
        // `docs/bugs.md` B94.
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

