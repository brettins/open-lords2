//! `Turn_Tick` (`0x0049A010`) phase 4 is
//!
//! ```c
//! AI_RunTurnStep();                                  /* 0x0049A581 */
//! if (Turn_AllRealmsDone()) Turn_AdvancePhase();
//! ```
//!
//! — called every frame, with no test on the person. So the AI realms step
//! *while he is deciding*; `Turn_End` (`0x0043AC23`) writing 999 into his own
//! `aiStep` is the last thing the wait needs. Ours used to open and run the
//! whole of phase 4 inside the wind-on End Turn starts, which is the report
//! *"the AI seems to take its turn at End Turn instead of at the start of the
//! turn"*.

use l2_game::game::Assets;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::Game;
use l2_kingdom::AI_STEP_DONE;

fn world() -> (Game, Assets) {
    let mut g = Game::new(5);
    g.prefs.tip_screens = false;
    g.kingdom.set_county_count(3);
    l2_testkit::chain_neighbours!(g.kingdom);
    g.kingdom.realms[1].in_play = true;
    g.kingdom.realms[1].strength = 3;
    g.kingdom.realms[1].is_human = true;
    g.kingdom.realms[2].in_play = true;
    g.kingdom.realms[2].strength = 3;
    g.kingdom.counties[1].owner = 1;
    g.kingdom.counties[1].population = 400;
    g.kingdom.counties[2].owner = 0;
    g.kingdom.counties[2].population = 400;
    g.kingdom.counties[3].owner = 2;
    g.kingdom.counties[3].population = 400;
    g.player = 1;
    g.selected = 1;
    (g, Assets::placeholder())
}

fn tick(m: &mut Machine, game: &mut Game, assets: &Assets) {
    let mut ctx = Ctx { game, assets };
    m.update(&mut ctx);
}

/// Claims, each one an observed ablation:
#[test]
fn the_ai_realms_take_their_steps_while_the_person_is_still_deciding() {
    let (mut game, assets) = world();
    let mut m = Machine::new(ScreenId::Campaign);
    let phase_before = game.kingdom.turn.phase;

    for _ in 0..40 {
        tick(&mut m, &mut game, &assets);
    }

    assert!(!l2_game::turn::turn_in_flight(&game), "nobody has pressed End Turn");
    assert!(
        game.kingdom.realms[2].ai_step > 1,
        "the AI realm steps before End Turn, not inside it; it is at {}",
        game.kingdom.realms[2].ai_step
    );
    assert_eq!(game.kingdom.realms[1].ai_step, 1, "the person parks at 1 until Turn_End");
    assert_eq!(game.kingdom.turn.phase, phase_before, "the machine is parked between turns");
}

#[test]
fn end_turn_does_not_make_the_ai_run_its_turn_a_second_time() {
    let (mut game, assets) = world();
    let mut m = Machine::new(ScreenId::Campaign);
    for _ in 0..40 {
        tick(&mut m, &mut game, &assets);
    }
    let reached = game.kingdom.realms[2].ai_step;
    assert!(reached > 1, "the AI realm had started its turn");
    assert!(game.kingdom.turn.players_turn_open, "the frames opened phase 4");

    let mut ctx = Ctx { game: &mut game, assets: &assets };
    m.handle(
        l2_game::input::Event::KeyDown(l2_game::input::Key::Char('E')),
        &mut ctx,
    );
    tick(&mut m, &mut game, &assets);

    assert!(
        game.kingdom.realms[2].ai_step >= reached,
        "the AI's counter carries on from {reached}, it is not reset under it"
    );
    assert_eq!(
        game.kingdom.realms[1].ai_step, AI_STEP_DONE,
        "Turn_End (0x0043AC23) wrote 999 into the person's realm"
    );
}
