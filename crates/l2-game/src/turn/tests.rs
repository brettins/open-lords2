#![allow(unused_imports)]
use super::*;
use super::engine::*;
use super::types::*;
use super::battle::*;
use super::ai_part::*;
use l2_kingdom::ai::{self, AiStep};
use l2_kingdom::conquest::Attack;
use l2_kingdom::phase::{Phase, PhaseWait};
use l2_kingdom::units_tick::{Contact, Encounter};
use l2_kingdom::victory::Outcome;
use l2_kingdom::{Kingdom, SeasonReport};
use crate::engagement::{self, Answer, BattleReport, SiegePhase};
use crate::game::Game;

#[cfg(test)]
mod tests {
    use super::*;
    use l2_kingdom::AI_STEP_DONE;

    #[test]
    fn a_realm_has_ended_its_turn_only_inside_one() {
        let mut game = Game::new(1);
        game.player = 1;
        for id in 1..=3 {
            game.kingdom.realms[id].in_play = true;
        }
        game.kingdom.realms[1].is_human = true;

        game.kingdom.realms[1].ai_step = AI_STEP_DONE;
        game.kingdom.realms[2].ai_step = AI_STEP_DONE + 1;
        game.kingdom.realms[3].ai_step = AI_STEP_DONE + 1;
        assert!(!turn_in_flight(&game));
        for id in 1..=3 {
            assert!(
                !realm_turn_ended(&game, id),
                "realm {id}: nobody has ended a turn while the person is playing his"
            );
        }

        game.turn = Some(TurnProgress::default());
        game.kingdom.realms[1].ai_step = 0;
        assert!(realm_turn_ended(&game, 1), "the person's turn ends on the click");

        game.kingdom.realms[2].ai_step = 4;
        assert!(!realm_turn_ended(&game, 2), "an AI still stepping has not finished");
        assert!(realm_turn_ended(&game, 3), "an AI parked on 999 has");
    }
}


