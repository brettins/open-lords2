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

    /// // arm: 0x0042FF10/dismiss-report right-release
    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        let dismiss = matches!(event, Event::RightClick { .. })
            || matches!(event, Event::Click { x, y } if ok_rect().contains(x, y));
        if !dismiss {
            return Transition::Stay;
        }
        match ctx.issue(crate::command::DismissReport {}) {
            TurnStep::Ask(_) => Transition::Replace(ScreenId::BattlePrompt),
            TurnStep::Report(_) => Transition::Stay,
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

        draw_roster(
            ctx,
            canvas,
            (&r.attacker_roster.1, Some(&r.attacker_roster.0)),
            (&r.defender_roster.1, Some(&r.defender_roster.0)),
            (r.attacker_men.1, r.defender_men.1),
        );

        // [V] Screen_BattleResult 0x00422FF1, `00420000.c:925`: `Ui_OkButton(400, 400, 0)`.
        p.ok_button(canvas, OK.0, OK.1, 0);
    }
}

