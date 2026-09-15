//! # The ablations

mod gestures;
pub use gestures::*;

use l2_game::game::Assets;
use l2_game::input::{Event, Rect};
use l2_game::press::{self, Kind, Press, Widget};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::county::{self, Panel};
use l2_game::Game;

pub(crate) fn world() -> (Game, Assets) {
    let mut g = Game::new(4);
    g.kingdom.set_county_count(2);
    l2_testkit::chain_neighbours!(g.kingdom);
    for id in 1..=2usize {
        let c = &mut g.kingdom.counties[id];
        c.owner = 1;
        c.population = 1_000;
        c.happiness = 90;
        c.grain = 5_000;
        c.herd = 100;
    }
    g.kingdom.realms[1].in_play = true;
    g.kingdom.realms[1].is_human = true;
    g.player = 1;
    g.selected = 1;
    (g, Assets::placeholder())
}

fn send(m: &mut Machine, g: &mut Game, a: &Assets, e: Event) {
    let mut ctx = Ctx { game: g, assets: a };
    m.handle(e, &mut ctx);
}

fn tick(m: &mut Machine, g: &mut Game, a: &Assets) {
    let mut ctx = Ctx { game: g, assets: a };
    m.update(&mut ctx);
}

fn on(r: Rect) -> (i32, i32) {
    (r.centre_x(), r.y + r.h / 2)
}

fn tax_panel() -> (Game, Assets, Machine) {
    let (g, a) = world();
    let m = Machine::new(ScreenId::County(1, Panel::Tax));
    (g, a, m)
}

