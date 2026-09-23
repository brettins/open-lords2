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

    fn asked(men: i32) -> Game {
        let mut game = Game::new(5);
        game.kingdom.set_county_count(3);
        game.player = 1;
        let mut army = |owner: u8, x: u8| {
            let mut u = l2_kingdom::unit::Unit::new(l2_kingdom::UnitKind::Army, owner, x, 5);
            u.men = 50;
            u.troops[0] = 50;
            game.kingdom.campaign.units.spawn(u).unwrap()
        };
        let (attacker, defender) = (army(1, 5), army(2, 6));
        let q = Question {
            attacker,
            defender,
            county: 1,
            is_siege: false,
            attacker_owner: 1,
            defender_owner: 2,
            attacker_men: men,
            defender_men: 80,
            attacker_roster: [0; l2_kingdom::unit::TROOP_TYPES],
            defender_roster: [0; l2_kingdom::unit::TROOP_TYPES],
            choice_owner: 1,
            castle_level: None,
        };
        suspend_on(&mut game, q);
        game
    }

    fn digest(game: &Game) -> u64 {
        let mut c = l2_net::Canonical::hashing();
        crate::net::encode_state(game, &mut c);
        c.finish().hash
    }

    /// Ablation: `debug(c, q)` in `encode_progress` back to four fields.
    #[test]
    fn the_digest_sees_the_whole_turn_in_flight() {
        assert_ne!(digest(&asked(100)), digest(&asked(101)), "the question's men");
        let (mut a, b) = (asked(100), asked(100));
        let e = Encounter { mover: 3, occupant: 4, county: 1 };
        a.turn.as_mut().unwrap().pending_battles.push(e);
        assert_ne!(digest(&a), digest(&b), "a battle waiting its turn");
    }

    /// Ablation: `out.bool(self.mercenary)` in `Figure::encode`, or the band in
    /// `Unit::encode` (`+0x195..+0x197`).
    #[test]
    fn the_digest_sees_the_mercenary_band_and_its_figures() {
        let (mut a, b) = (asked(100), asked(100));
        let q = a.turn.as_ref().and_then(|t| t.question.clone()).map(|q| q.attacker);
        let band = l2_kingdom::Mercenaries { band: 3, troop: l2_kingdom::TroopType::Knight, men: 20 };
        a.kingdom.campaign.units.get_mut(q.unwrap()).unwrap().mercenaries = Some(band);
        assert_ne!(digest(&a), digest(&b), "the band riding with the army");
        let seats = vec![(l2_net::PlayerSlot::new(0), 1)];
        a.seats = seats;
        a.apply(l2_net::PlayerSlot::new(0), &crate::command::TakeTheField {}.into());
        let sim = &mut a.battle.as_mut().expect("the field taken").runner.sim;
        let hired = sim.figures.iter().position(|f| f.mercenary);
        let i = hired.expect("Muster.mercenaries raised a flagged figure");
        let before = digest(&a);
        a.battle.as_mut().unwrap().runner.sim.figures[i].mercenary = false;
        assert_ne!(before, digest(&a), "Figure::mercenary");
    }

    /// Ablation: the bystander guard in `take_the_field`.
    #[test]
    fn a_bystander_seat_cannot_take_the_field() {
        let mut game = asked(100);
        let seats = [(l2_net::PlayerSlot::new(0), 1), (l2_net::PlayerSlot::new(1), 3)];
        game.seats = seats.to_vec();
        let order = crate::command::TakeTheField {}.into();
        game.apply(l2_net::PlayerSlot::new(1), &order);
        assert!(game.battle.is_none(), "realm 3 is neither side of 1 against 2");
        game.apply(l2_net::PlayerSlot::new(0), &order);
        assert_eq!(game.battle.as_ref().map(|b| b.owner), Some(1), "realm 1 fights as itself");
    }
}


