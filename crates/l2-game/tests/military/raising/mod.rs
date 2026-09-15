#![allow(unused_imports)]

mod raising_flow;
pub use raising_flow::*;
mod armoury_controls;
pub use armoury_controls::*;

use super::*;
use super::battle_part::*;
use super::marching::*;
use super::division::*;
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::battlefield as bf;
use l2_game::screens::{armoury, army, battle, divide, info, map};
use l2_game::Game;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::unit::{TroopType, Unit, UnitKind};
use l2_kingdom::MercenaryBands;
use l2_view::campaign;


fn set_levy(m: &mut Machine, g: &mut Game, a: &Assets, percent: i32) {
    click(m, g, a, (army::SLIDER_X + percent, army::base(false) + 0x20));
}

fn one_crossbowman(m: &mut Machine, g: &mut Game, a: &Assets) -> (i32, i32) {
    press(m, g, a, 'r');
    tick(m, g, a);
    set_levy(m, g, a, 30);
    press_and_wait(m, g, a, on(army::continue_button(false)));
    assert_eq!(m.top_id(), Some(ScreenId::Armoury(1)));
    let rack = armoury::RACK_HOTSPOTS.iter().find(|h| h.4 == 1).expect("the crossbow rack");
    let at = ((rack.0 + rack.2) / 2, (rack.1 + rack.3) / 2);
    click(m, g, a, at);
    assert_eq!(m.top_id(), Some(ScreenId::Rack(1, 1)));
    assert!(
        !g.levy.anim.walker.active,
        "the first pick after a door sends nobody: Levy_Seed zeroed g_armourySelectedType, \
         so FUN_004AABD8 is handed type 0 and `0 < type` refuses"
    );
    click(m, g, a, on(armoury::button_box(0)));
    assert_eq!(g.levy.basket.troops()[TroopType::Crossbowman.index()], 1);
    assert!(!g.levy.anim.walker.active, "equipping sends nobody: FUN_004AABD8 has one caller");
    at
}

