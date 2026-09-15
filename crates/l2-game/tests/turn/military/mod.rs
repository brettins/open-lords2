#![allow(unused_imports)]

mod army_movement;
pub use army_movement::*;
mod combat;
pub use combat::*;
mod tile_effects;
pub use tile_effects::*;
mod merchants_and_mobs;
pub use merchants_and_mobs::*;
mod purity;
pub use purity::*;

use super::*;
use super::spine::*;
use l2_kingdom::phase::Phase;
use l2_kingdom::realm::AI_STEP_DONE;
use l2_kingdom::tables::Tables;
use l2_game::turn;
use l2_game::Game;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::units_tick::Contact;
use l2_kingdom::merchant::MerchantRoutes;
use l2_kingdom::unit::{Unit, UnitKind};

fn with_a_map() -> Game {
    let mut g = five_realms();
    let mut map = CampaignMap::empty();
    for i in 0..MAP_TILES {
        map.county[i] = ((i % MAP_DIM) / 5 + 1).min(14) as u8;
    }
    for x in 0..64u8 {
        map.set_flags(x, 10, flags::ROAD);
    }
    g.kingdom.campaign.map = map;
    for id in 1..=14usize {
        g.kingdom.counties[id].anchor_x = (id as u8 - 1) * 5 + 2;
        g.kingdom.counties[id].anchor_y = 10;
    }
    g
}

/// `Units_Tick` runs on ordinary frames as well as inside a turn —
/// [`turn::tick_units_only`], `docs/decisions.md` C115 — and since
/// `Unit_StepOnce`'s sub-tile counter landed (`docs/decisions.md`
/// **C134**) a unit takes 8 ticks to cross a road tile and 32 to cross
/// anything else. **Nothing in the seven phases waits on the human's armies**:
pub(super) fn march(g: &mut Game) -> u32 {
    for ticks in 1..=turn::MAX_TICKS {
        turn::tick_units_only(g);
        if !g.kingdom.campaign.units.iter().any(|(_, u)| u.moving) {
            return ticks;
        }
    }
    panic!("the march never finished");
}

fn army(g: &mut Game, owner: u8, x: u8, y: u8) -> usize {
    let mut u = Unit::new(UnitKind::Army, owner, x, y);
    u.men = 120;
    u.troops[0] = 120;
    u.county = g.kingdom.campaign.map.county_at(x, y);
    u.home_county = u.county;
    u.owner_is_human = g.kingdom.realms[owner as usize].is_human;
    g.kingdom.campaign.units.spawn(u).expect("a free slot")
}

