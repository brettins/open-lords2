//! One frame of `Turn_Tick` + `Units_Tick` (`0x004B99C0`), shared by the map
//! screen and the headless `crate::net::NetGame`, so the two cannot drift.

use super::*;
use super::super::types::*;
use crate::game::Game;
use l2_net::Canonical;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wound {
    /// The clock ran out: the caller issues `crate::command::EndTurn`.
    EndTurnDue,
    Asking,
    Reporting,
    Stepped(TurnStep),
    Idle { stepped: usize },
}

pub fn wind(game: &mut Game) -> Wound {
    if game.turn_clock.end_turn_pending() && !turn_in_flight(game) {
        game.turn_clock.take_end_turn();
        return Wound::EndTurnDue;
    }
    if turn_in_flight(game) {
        if pending_question(game).is_some() {
            return Wound::Asking;
        }
        if pending_report(game).is_some() {
            return Wound::Reporting;
        }
        return Wound::Stepped(tick_turn(game));
    }
    // `Turn_Tick`'s phase-4 arm: `AI_RunTurnStep` (`0x0049A581`) every frame.
    tick_ai_frame(game);
    Wound::Idle { stepped: tick_units_only(game) }
}

/// `[I]` the turn's control word only: the siege cursor, tail contents and
/// fought reports are not encoded; they reach the digest when they reach the
/// kingdom.
pub fn encode_progress(game: &Game, out: &mut Canonical) {
    out.section("turn");
    out.option(game.turn.as_ref(), |c, p| {
        c.bool(p.idle);
        c.u32(p.ticks);
        c.len32(p.steps);
        c.bool(p.granted);
        c.u8(p.stage as u8);
        c.option(p.question.as_ref(), |c, q| {
            c.len32(q.attacker);
            c.len32(q.defender);
            c.u8(q.county);
            c.u8(q.choice_owner);
        });
        c.bool(p.siege.is_some());
        c.bool(p.pending_assault.is_some());
        c.bool(p.tail.is_some());
        c.len32(p.reports.len());
        c.bool(p.unseen.is_some());
    });
    out.end_section();
}
