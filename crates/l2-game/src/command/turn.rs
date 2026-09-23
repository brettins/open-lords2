//! The turn's own questions, answered as orders.

use super::*;
use crate::turn::{self, TurnStep};

/// `Turn_End` (`0x0043AC23`) writes 999 into `aiStep`; the wind-on starts here.
/// Seated, it is the issuing realm's alone — `FUN_004479E9` writes it for a
/// remote seat — and the world waits for every seated human. `docs/netcode.md` §5.
impl Order for EndTurn {
    type Reply = TurnStep;
    fn run(self, game: &mut Game) -> TurnStep {
        if turn::turn_in_flight(game) {
            return TurnStep::Running;
        }
        if game.seated() {
            turn::open_players_turn(game);
            if let Some(r) = game.kingdom.realms.get_mut(game.player as usize) {
                r.ai_step = l2_kingdom::AI_STEP_DONE;
            }
            if !turn::every_seat_ready(game) {
                return TurnStep::Running;
            }
        }
        turn::end_players_turn(game);
        turn::begin_turn(game)
    }
}

/// `Turn_End` from the clock (`0x0049A010` expiry): every seat at once.
pub(crate) fn time_up(game: &mut Game) -> TurnStep {
    if turn::turn_in_flight(game) {
        return TurnStep::Running;
    }
    turn::end_players_turn(game);
    turn::begin_turn(game)
}

impl Order for AnswerBattle {
    type Reply = TurnStep;
    fn run(self, game: &mut Game) -> TurnStep {
        turn::answer_battle(game, self.answer)
    }
}

/// `FUN_0043B593` hotspot 1 → `Battle_Start` (`0x004778A0`).
impl Order for TakeTheField {
    type Reply = bool;
    fn run(self, game: &mut Game) -> bool {
        turn::take_the_field(game)
    }
}

/// `Battle_CheckOutcome` (`0x00477DFC`) or `FUN_0043BE65`'s early exit.
impl Order for FinishBattle {
    type Reply = TurnStep;
    fn run(self, game: &mut Game) -> TurnStep {
        turn::finish_battle(game)
    }
}

/// `[I]` sim state: an unseen report holds the turn's wind-on.
impl Order for DismissReport {
    type Reply = TurnStep;
    fn run(self, game: &mut Game) -> TurnStep {
        turn::dismiss_report(game)
    }
}
