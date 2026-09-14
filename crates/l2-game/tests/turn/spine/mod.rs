#![allow(unused_imports)]

mod spine_tests;
pub use spine_tests::*;

use super::*;
use super::military::*;
use l2_kingdom::phase::Phase;
use l2_kingdom::realm::AI_STEP_DONE;
use l2_kingdom::tables::Tables;
use l2_game::turn;
use l2_game::Game;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::units_tick::Contact;
use l2_kingdom::merchant::MerchantRoutes;
use l2_kingdom::unit::{Unit, UnitKind};

/// One human realm holding county 1, four AI realms holding one each, and nine
/// unowned — the shape of the England turn-one scenario, without needing the file.
fn five_realms() -> Game {
    let mut g = Game::new(0x51EED);
    g.player = 1;
    g.kingdom.set_county_count(14);
    for id in 1..=14 {
        let c = &mut g.kingdom.counties[id];
        c.population = 417;
        c.pop_last = 417;
        c.happiness = 65;
        c.happiness_last = 65;
        c.health_meter = 65;
        c.health_band = l2_kingdom::tables::health_band(65);
        c.herd = 67;
        c.ration_wanted = 3;
        c.ration_split = 100;
        for n in 1..=14u8 {
            if n as usize != id {
                c.add_neighbour(n);
            }
        }
    }
    for realm in 1..=5usize {
        g.kingdom.counties[realm].owner = realm as u8;
        g.kingdom.realms[realm].in_play = true;
        g.kingdom.realms[realm].strength = 3;
        g.kingdom.realms[realm].county_count = 1;
        g.kingdom.realms[realm].gold = 1000;
        g.kingdom.realms[realm].lord = realm as u8 - 1;
    }
    g.kingdom.realms[1].is_human = true;
    g.kingdom.realms[1].lord = 0;
    g
}

