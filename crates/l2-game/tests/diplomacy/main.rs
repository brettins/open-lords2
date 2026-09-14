//! **The player's side of diplomacy, driven the way a player drives it.**
//!
//! `docs/agents.md`: *"an agent testing our engine does not touch the OS input
//! queue"* — everything here is an `Event` value handed to `Machine::handle`,
//! and nothing opens a window or needs a copy of the game.
//!
//! The point of the file is the thing the unit tests in
//! `l2_kingdom::diplomacy` cannot say: **that there is a route from a click to
//! the rule.** Every one of those tests calls the rule directly, and a rule
//! nobody can reach is the failure this whole subsystem was blocking on in the
//! other direction — see `docs/decisions.md` C62 and `tests/ai_war.rs`.
//!
//! So each test below starts at the sidebar or at the lord card and ends by
//! reading `l2_kingdom`'s own state.

mod dialog_flow;
pub use dialog_flow::*;
mod offers_and_requests;
pub use offers_and_requests::*;
mod letters;
pub use letters::*;

use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::screen::{Ctx, Machine, Screen, ScreenId, Transition};
use l2_game::screens::diplomacy::{self, DiplomacyScreen, Menu};
use l2_game::Game;
use l2_kingdom::diplomacy::Kind;
use l2_view::Canvas;

/// Realm 1 is the person, realms 2 and 3 are the Knight and the Baron, and
/// county 3 has an enemy standing in it so that *ask for help* has something to
/// be about.
pub(crate) fn world() -> (Game, Assets) {
    let mut g = Game::new(5);
    g.kingdom.set_county_count(4);
    for realm in 1..=3usize {
        let r = &mut g.kingdom.realms[realm];
        r.in_play = true;
        r.strength = 3;
        r.county_count = 1;
        r.gold = 4_000;
        r.population_mean = 900;
        r.lord = realm as u8 - 1;
        r.shield_index = realm as u8;
        g.kingdom.counties[realm].owner = realm as u8;
        g.kingdom.counties[realm].population = 800;
    }
    g.kingdom.realms[1].is_human = true;
    g.kingdom.realms[1].lord = 0;
    // County 4 is nobody's, which is `Diplo_SendClicked`'s group 241.
    g.kingdom.counties[4].owner = 0;
    g.kingdom.counties[4].population = 400;
    // County 1 is mine and under threat, which is what kind 5 wants.
    g.kingdom.counties[1].enemy_troops = 40;
    g.player = 1;
    g.selected = 1;
    g.kingdom.init_diplomacy();
    (g, Assets::placeholder())
}

fn send(machine: &mut Machine, game: &mut Game, assets: &Assets, event: Event) {
    let mut ctx = Ctx { game, assets };
    machine.handle(event, &mut ctx);
}

fn middle(r: l2_game::input::Rect) -> Event {
    Event::Click { x: r.x + r.w / 2, y: r.y + r.h / 2 }
}

/// **Press a kind-5 widget and let its countdown run out.**
///
/// Every widget on these two screens that changes what is on screen — the six
/// verb buttons, the send, the cancel — is `Widget_Test` kind 5: the press puts
/// the picture down and sets `rec[0x0D] = 0x14`, and the handler runs from the
/// countdown at the top of the next call. So a test that clicks one and asserts
/// on the next line is asserting about a press the game has not answered yet.
///
/// It asserts the delay as it goes, which is what makes it a test of the
/// gesture: the screen stack must not move on any
/// tick before the last. `docs/input.md` §4.
fn press_and_wait(machine: &mut Machine, game: &mut Game, assets: &Assets, event: Event) {
    let before = machine.depth();
    send(machine, game, assets, event);
    assert_eq!(machine.depth(), before, "a kind-5 press must not act on the press");
    for i in 1..l2_game::press::DELAYED_FRAMES as u32 {
        let mut ctx = Ctx { game, assets };
        machine.update(&mut ctx);
        assert_eq!(machine.depth(), before, "nor on tick {i}");
    }
    let mut ctx = Ctx { game, assets };
    machine.update(&mut ctx);
}

