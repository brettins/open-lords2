
mod starvation;
pub use starvation::*;
mod mercenaries;
pub use mercenaries::*;
mod mines;
pub use mines::*;
mod conquest_part;
pub use conquest_part::*;
mod persistence;
pub use persistence::*;

use l2_kingdom::conquest;
use l2_kingdom::county::MAX_COUNTIES;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::movement::Routing;
use l2_kingdom::phase::Pass;
use l2_kingdom::report::Message;
use l2_kingdom::unit::{Unit, UnitKind, Units};
use l2_kingdom::{Kingdom, MercenaryBands, Options, TroopType};

fn kingdom() -> Kingdom {
    let mut k = Kingdom::new(0xA12);
    assert!(k.set_county_count(2));
    k.season = 1;
    k.season_next = 2;
    k.year = 1268;
    k.year_next = 1269;
    k.turn_count = 1;

    k.realms[1].in_play = true;
    k.realms[1].is_human = true;
    k.realms[1].shield_index = 2;
    k.realms[1].gold = 10_000;

    for id in 1..=2 {
        let c = &mut k.counties[id];
        c.population = 500;
        c.happiness = 77;
        c.health_meter = 65;
        c.herd = 100;
        c.grain = 200;
        c.ration_wanted = 3;
        c.ration_split = 100;
        c.fields_grain = 4;
        c.fields_cattle = 4;
        c.crop[1] = 400;
    }
    k.counties[1].owner = 1;
    k.counties[1].neighbour_count = 1;
    k.counties[1].neighbours[0] = 2;
    k.counties[2].neighbour_count = 1;
    k.counties[2].neighbours[0] = 1;

    let mut map = CampaignMap::empty();
    for i in 0..MAP_TILES {
        map.county[i] = if i % MAP_DIM < 32 { 1 } else { 2 };
    }
    k.campaign.map = map;
    k.campaign.mercenaries = MercenaryBands::init(2);
    k
}

fn army(k: &mut Kingdom, owner: u8, county: u8, men: i32, x: u8, y: u8) -> usize {
    let mut u = Unit::new(UnitKind::Army, owner, x, y);
    u.men = men;
    u.troops[TroopType::Peasant.index()] = men;
    u.county = county;
    u.home_county = county;
    u.owner_is_human = k.realms[owner as usize].is_human;
    k.campaign.units.spawn(u).expect("a free slot")
}

