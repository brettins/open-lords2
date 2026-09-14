#![allow(unused_imports)]
use super::*;
use super::render::*;
use super::result::*;
use super::tests_part::*;
use l2_kingdom::battle::Outcome;
use l2_kingdom::unit::ALL_TROOP_TYPES;
use l2_view::chrome::system;
use l2_view::{text, Canvas};
use crate::engagement::{Answer, Roster};
use crate::input::{Event, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};
use crate::turn::{self, Question, TurnStep};

// ---------------------------------------------------------------- the prompt

/// Screen `0x12`. The question lives on the suspended turn, which is where the
/// answer has to go back to; the only state here is the two widgets' press
/// timer.
pub struct BattlePromptScreen {
    press: Press,
}

/// **`DAT_004DDBB0` as a table, with the kind byte its two records carry.**
///
/// Both are `Widget_Test` kind **4**, read out of `+0x0F` of `0x004DDBB0` and
/// `0x004DDBC8`. `docs/arms.json` filed this pair as `left-release` — inferred
/// from the shape of the buttons — and the exe
/// says the press. It is the same pair of pictures as the yes/no box and a
/// *different kind*, which is exactly the case that makes the kind a fact to be
/// read.
///
/// The repeat that comes with kind 4 never runs: `Battle_PromptAnswered`
/// (`0x0043B593`) leaves screen `0x12` on the first fire, and a table that is no
/// longer being walked cannot repeat. What the kind buys a player here is the
/// **pressed picture**, the press edge, and a double click being a press:
/// `Widget_Test`'s kind-4 guard is `g_mouseLeftPressed ||
/// g_mouseLeftDoubleClick`, and [`BattlePromptScreen::handle`] hands every
/// event to [`Press::event`],
///
/// `DAT_004DDBB0[0]`, hotspot id 1, is `FUN_0043B593` → `Battle_Start`
/// (`0x004778A0`); `DAT_004DDBB0[1]`, hotspot id 0, is `Battle_Decline`
/// (`0x0043B622`). Both are hit-tested by `Screen_HandleInput`
/// (`0x004BA9C8`), so the arms carry its address.
fn prompt_widgets() -> [Widget; 2] {
    [
        Widget::new(widget_rect(TAKE_THE_FIELD), crate::arm!("0x004BA9C8/prompt-fight", Repeat)),
        Widget::new(widget_rect(DECLINE), crate::arm!("0x004BA9C8/prompt-decline", Repeat)),
    ]
}

impl BattlePromptScreen {
    pub fn new() -> BattlePromptScreen {
        BattlePromptScreen { press: Press::new() }
    }

    /// The question this screen is about, or `None` if the turn is no longer
    /// suspended — which a screen must survive, because the machine may still
    /// draw it for one frame after the turn moved on.
    fn question(ctx: &Ctx) -> Option<Question> {
        turn::pending_question(ctx.game)
    }

    /// **Answer, and carry the turn on.**
    ///
    /// The battle is fought or calculated inside this call, so what comes back
    /// is normally [`TurnStep::Report`] and the transition is a `Replace` onto
    /// screen `0x13` — which is the original's sequence exactly: `0x12`, the
    /// battle, `0x13`.
    fn answer(ctx: &mut Ctx, answer: Answer) -> Transition {
        match turn::answer_battle(ctx.game, answer) {
            TurnStep::Report(_) => Transition::Replace(ScreenId::BattleResult),
            // Another battle in the same turn, straight after this one.
            TurnStep::Ask(_) => Transition::Stay,
            // The turn is carrying on and has nothing more to ask: the map
            // underneath takes it from here, a tick a frame. See
            // [`turn::TurnStep::Running`].
            TurnStep::Running | TurnStep::Done(_) | TurnStep::Stuck => Transition::Pop,
        }
    }
}

impl Default for BattlePromptScreen {
    fn default() -> Self {
        BattlePromptScreen::new()
    }
}

impl Screen for BattlePromptScreen {
    fn id(&self) -> ScreenId {
        ScreenId::BattlePrompt
    }

    /// `Widget_Test`'s `Sound_RestartSlot(1)`, carried up to the audio
    /// layer. See [`Screen::take_clicks`].
    fn take_clicks(&mut self) -> u8 {
        self.press.take_clicks()
    }

    /// The thumb's picture coming back up. See [`Press::take_redraw`].
    fn take_redraw(&mut self) -> bool {
        self.press.take_redraw()
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "A Battle is to be fought".into()
    }

    /// A window over the campaign map, which the original repaints underneath
    /// it — `Screen_DrawCampaign(1)` is the first statement of both painters.
    fn is_overlay(&self) -> bool {
        true
    }

    /// **Two widgets and nothing else.**
    ///
/// This is what `Screen_FrameInput`'s `0x12` arm is, and it took
    /// the input audit to find out. The whole arm is:
    ///
    /// ```c
    /// else if (g_screenId == '\x12') {
    ///     if (DAT_00553fc8 != 0) { Battle_Decline(); … }      /* the sync latch */
    ///     if (FUN_004bbea7() != 0) { Battle_Decline(); … }    /* the answer timeout */
    /// }
    /// ```
    ///
    /// and `FUN_004BBEA7` (`0x004BBEA7`) opens with
    /// `if (g_multiplayer == 0) return 0;`. **In a single-player game the arm
    /// does nothing at all** — no right-button test, no key, no OK corner. The
    /// only two exits are the two widgets of `DAT_004DDBB0`, which
    /// `Screen_HandleInput` (`0x004BA9C8`) hit-tests at offset `(0x20, 0x30)`
    /// with a count of `DAT_00554408`; that count is written by
    /// `Battle_ChooseSettlement` (`0x004A6A30`) and is **2 when
    /// `g_battleChoiceOwner == 1` and 0 otherwise**,
    /// no widgets and no way out but the multiplayer timeout. The table holds
    /// exactly two records — `g_sliderWidgets` begins at `0x004DDBE0`, 48 bytes
    /// on —.
    ///
/// Gone from here, and counted as inventions
    /// (`docs/arms.json`): **right-click to Decline**, **Escape to Decline**,
    /// **Enter to take the field**, and answering on any click for a bystander.
    /// The prompt waits for ever in single player and that is correct — it is
    /// what `docs/symbols.json` records of `Battle_Decline` and it is not a
    /// thing to fix.
    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        // **A bystander has no choice to make**, and no widget either. The
        // screen is a notice; the original leaves it up until the multiplayer
        // timeout, which single player does not have. Ours is reachable only
        // through the interactive door, and a bystander battle in a
        // single-player game cannot arise — `Battle_ChooseSettlement` returns 0
        // when neither side is human and the battle is settled with no screen —
        // so this arm is a guard against a state that has no route to it.
        let Some(q) = BattlePromptScreen::question(ctx) else { return Transition::Pop };
        if q.choice_owner != 1 {
            return Transition::Stay;
        }
        let fired = self.press.event(&prompt_widgets(), event);
        if fired.is_some() {
            // **End the hold on the fire.** In the original the repeat is
            // unreachable because `Battle_PromptAnswered` leaves screen `0x12`
            // and a table nobody walks cannot repeat. Ours can stay — the
            // muster below can fail — so the hold is ended explicitly rather
            // than left to be unreachable for a reason that is true elsewhere.
            // The press timer keeps running, which is what `rec[0x0D] = 3` is
            // for.
            self.press.release();
        }
        match fired {
            // `DAT_004DDBB0[0]`, hotspot id 1 → `FUN_0043B593` →
            // `Battle_Start` (`0x004778A0`). It raises the battlefield; it does
            // **not** settle the battle. Its arm is declared on
            // [`prompt_widgets`].
            Some(0) => {
                if turn::take_the_field(ctx.game) {
                    Transition::Replace(ScreenId::Battlefield)
                } else {
                    // The armies could not be mustered — a slot is no longer a
// unit. Settle it the way a headless turn would
                    // leaving the prompt up with nothing behind it.
                    BattlePromptScreen::answer(ctx, Answer::TakeTheField)
                }
            }
            // `DAT_004DDBB0[1]`, hotspot id 0 → `Battle_Decline`
            // (`0x0043B622`),
            Some(_) => BattlePromptScreen::answer(ctx, Answer::Decline),
            None => Transition::Stay,
        }
    }

    /// The countdown at the top of `Widget_Test`, which runs the press timer
    /// down whether or not anything is under the pointer.
    fn update(&mut self, _ctx: &mut Ctx) -> Transition {
        // Nothing can come back from it: both records are kind 4, which fires
        // on the press, and [`BattlePromptScreen::handle`] ends the hold on
        // that fire. It runs for the pressed picture.
        let _ = self.press.tick();
        Transition::Stay
    }

    pub(crate) fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let Some(q) = BattlePromptScreen::question(ctx) else { return };
        let p = pen(ctx);
        draw_frame(ctx, canvas, q.county, q.attacker_owner, q.defender_owner, q.is_siege);

        // **The heading is one of two**, and the siege one is the game saying
// something different about a siege: *"The
        // Siege commences."*
        let heading = if q.is_siege { 7 } else { 0 };
        let s = ctx.assets.shell.text(GROUP_PROMPT, heading).to_string();
        let s = if s.is_empty() { "A BATTLE IS TO BE FOUGHT.".to_string() } else { s };
        p.heading(canvas, HEADING.0, HEADING.1, &s, font::TEXT);

        // The paragraph, and the wrap width the original uses for each. 180 for
        // the question, 280 for the two statements — a narrower column where
        // the thumbs sit beside it.
        let (index, wrap) = match q.choice_owner {
            1 => (1usize, 0xB4),
            2 => (2, 0x118),
            _ => (3, 0x118),
        };
        let body = ctx.assets.shell.text(GROUP_PROMPT, index).to_string();
        let body = if body.is_empty() {
            match index {
                1 => "WILL YOU TAKE THE FIELD?".to_string(),
                2 => "YOUR OPPONENT HAS THE CHOICE.".to_string(),
                _ => "THE OPPONENTS ARE DECIDING.".to_string(),
            }
        } else {
            body
        };
        p.body_wrapped(canvas, PARAGRAPH.0, PARAGRAPH.1, wrap, &body, font::TEXT);

        draw_roster(
            ctx,
            canvas,
            (&q.attacker_roster, None),
            (&q.defender_roster, None),
            (q.attacker_men, q.defender_men),
        );

        // The two thumbs, and only when the choice is the local player's.
        if q.choice_owner == 1 {
            // `Widget_Draw`'s `base + 1` while `+0x0D` runs — the thumb goes
            // down when it is pressed.
            for (i, w) in [TAKE_THE_FIELD, DECLINE].into_iter().enumerate() {
                let frame = if self.press.is_pressed(i) { w.2 + 1 } else { w.2 };
                let drawn = ctx
                    .assets
                    .chrome
                    .as_ref()
                    .is_some_and(|c| c.draw_system(canvas, frame, w.0, w.1));
                if !drawn {
                    shell::button_recess(canvas, w.0, w.1, w.3, w.3);
                }
            }
            // OURS: the thumbs are a mailed hand up and down and a modern
            // player has no legend for them. `docs/decisions.md` C21 — visibly
            // ours, in our own font, outside the original's widgets.
            // Debug overlay only: the original's prompt has no such words.
            let dim = ctx.assets.ink.dim;
            if ctx.game.prefs.debug_overlay {
                text::draw(canvas, TAKE_THE_FIELD.0 - 4, TAKE_THE_FIELD.1 + 34, "FIGHT", dim);
                text::draw(canvas, DECLINE.0 - 4, DECLINE.1 + 34, "AUTO", dim);
            }
        }
    }
}

