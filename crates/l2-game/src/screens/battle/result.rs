#![allow(unused_imports)]
use super::*;
use super::render::*;
use super::prompt::*;
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

// ---------------------------------------------------------------- the result

/// Screen `0x13`. A **pure report** over a settled report: nothing it
/// draws changes anything, which is the one structural difference from the
/// original's painter (see the module header).
pub struct BattleResultScreen;

impl BattleResultScreen {
    pub fn new() -> BattleResultScreen {
        BattleResultScreen
    }
}

impl Default for BattleResultScreen {
    fn default() -> Self {
        BattleResultScreen::new()
    }
}

impl Screen for BattleResultScreen {
    fn id(&self) -> ScreenId {
        ScreenId::BattleResult
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "The Battle is decided".into()
    }

    fn is_overlay(&self) -> bool {
        true
    }

    /// **Two ways out, and `0x13` really does have the right-button one that
    /// `0x12` does not.**
    ///
    /// ```c
    /// if (g_mouseRightReleased == '\0') {
    ///     if (Ui_OkButtonClicked()) { g_screenId = 0; g_redrawRequest = 2; }
    /// } else { g_screenId = 0; g_redrawRequest = 2; }
    /// ```
    ///
    /// The two neighbouring screens differing on this is what made
    /// right-click-to-Decline on `0x12` look reasonable. There are no keys on
    /// either; Escape and Enter used to be here and were ours.
    ///
    /// // arm: 0x0042FF10/dismiss-report right-release
    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        let dismiss = matches!(event, Event::RightClick { .. })
            || matches!(event, Event::Click { x, y } if ok_rect().contains(x, y));
        if !dismiss {
            return Transition::Stay;
        }
        match turn::dismiss_report(ctx.game) {
            // Another battle this turn: back to the prompt for it.
            TurnStep::Ask(_) => Transition::Replace(ScreenId::BattlePrompt),
            TurnStep::Report(_) => Transition::Stay,
            // The rest of the turn belongs to the map, which winds it on one
            // tick a frame. See [`turn::TurnStep::Running`].
            TurnStep::Running | TurnStep::Done(_) | TurnStep::Stuck => Transition::Pop,
        }
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let Some(r) = turn::pending_report(ctx.game) else { return };
        let p = pen(ctx);
        draw_frame(ctx, canvas, r.county, r.attacker_owner, r.defender_owner, r.is_siege);

        let heading = if r.is_siege { 8 } else { 0 };
        let s = ctx.assets.shell.text(GROUP_RESULT, heading).to_string();
        let s = if s.is_empty() { "THE BATTLE IS DECIDED.".to_string() } else { s };
        p.heading(canvas, HEADING.0, HEADING.1, &s, font::TEXT);

        // **The banner that belongs to screen `0x2B`.** Group 82's pair for
        // this battle, from the local player's point of view — printed here
        //
        // told only *"The Battle is decided."* has not been told the outcome.
// Marked as ours by being under the original's heading
        // place of it.
        let pair = r.outcome(ctx.game.player).pair();
        let banner = ctx.assets.shell.text(GROUP_BANNER, pair * 2).to_string();
        let banner = if banner.is_empty() { ours_banner(r.outcome(ctx.game.player)) } else { banner };
        p.body(canvas, PARAGRAPH.0, PARAGRAPH.1, &banner, font::HIGHLIGHT);

        draw_roster(
            ctx,
            canvas,
            (&r.attacker_roster.1, Some(&r.attacker_roster.0)),
            (&r.defender_roster.1, Some(&r.defender_roster.0)),
            // Mode 1 ends with the same two `Ui_DrawCount(menTotal, …)` calls
            // as mode 0, and by this point `menTotal` is what the fight left.
            (r.attacker_men.1, r.defender_men.1),
        );

        let drawn = ctx
            .assets
            .chrome
            .as_ref()
            .is_some_and(|c| c.draw_system(canvas, system::OK, OK.0, OK.1));
        if !drawn {
            crate::widget::button(canvas, &ctx.assets.ink, ok_rect(), "OK", false);
        }
    }
}

