#![allow(unused_imports)]
use super::*;
use super::behavior::*;
use super::rendering::*;
use l2_game::game::Assets;
use l2_game::input::{Event, Rect};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::county::Panel;
use l2_game::screens::options::{self, Setting};
use l2_game::shell::Pen;
use l2_game::tooltip::{self, Shown, Sidebar};
use l2_game::Game;
use l2_kingdom::unit::{TroopType, Unit, UnitKind};
use l2_view::Canvas;

pub(super) fn battlefield() -> (Game, Assets, Machine) {
    let mut g = world();
    g.kingdom.counties[2].owner = 2;
    for realm in 1..=2usize {
        g.kingdom.realms[realm].in_play = true;
    }
    let spawn = |g: &mut Game, owner: u8, x: u8| {
        let mut u = Unit::new(UnitKind::Army, owner, x, 20);
        u.men = 100;
        u.troops[TroopType::Peasant.index()] = 100;
        u.county = owner;
        u.home_county = owner;
        u.owner_is_human = owner == 1;
        g.kingdom.campaign.units.spawn(u).expect("a free slot")
    };
    let attacker = spawn(&mut g, 1, 30);
    let defender = spawn(&mut g, 2, 31);
    let runner = l2_game::engagement::begin_fight(&mut g.kingdom, attacker, defender, None, 7)
        .expect("two armies");
    g.battle =
        Some(Box::new(l2_game::battlefield::LiveBattle::new(runner, attacker, defender, 2, None, 1, 1)));
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Battlefield);
    (g, Assets::placeholder(), m)
}

