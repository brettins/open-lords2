//! The point of the file is the thing the unit tests in
//! `l2_kingdom::diplomacy` cannot say: **that there is a route from a click to
//! the rule.** Every one of those tests calls the rule directly, and a rule
//! nobody can reach is the failure this whole subsystem was blocking on in the
//! other direction — see `docs/decisions.md` C62 and `tests/ai_war.rs`.

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

pub(crate) fn world() -> (Game, Assets) {
    let mut g = Game::new(5);
    g.kingdom.set_county_count(4);
    l2_testkit::chain_neighbours!(g.kingdom);
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

