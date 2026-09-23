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

/// Every field of the turn in flight: the siege cursor, the tail's contacts and
/// report, the fought reports and the unseen one decide what the next frames
/// do. `docs/netcode.md` §6 *What we hash*.
pub fn encode_progress(game: &Game, out: &mut Canonical) {
    out.section("turn");
    out.option(game.turn.as_ref(), |c, p| {
        c.bool(p.idle);
        c.u32(p.ticks);
        c.len32(p.steps);
        c.bool(p.granted);
        c.u8(p.stage as u8);
        c.option(p.question.as_ref(), |c, q| debug(c, q));
        c.seq(&p.contacts, debug);
        c.seq(&p.pending_battles, debug);
        c.option(p.siege.as_ref(), |c, s| s.encode(c));
        c.option(p.pending_assault.as_ref(), debug);
        c.option(p.tail.as_ref(), debug);
        c.seq(&p.reports, debug);
        c.option(p.unseen.as_ref(), debug);
    });
    out.end_section();
}

/// `[I]` derived `Debug` is every field in declaration order, and these types
/// hold no map and no float (D-1, D-4); the digest is never decoded.
fn debug<T: core::fmt::Debug>(out: &mut Canonical, value: &T) {
    out.str(&format!("{value:?}"));
}
