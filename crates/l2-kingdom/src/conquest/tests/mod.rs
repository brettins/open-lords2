
mod capture_rules;
mod defence;
mod change_owner;
pub(crate) use change_owner::*;

use super::*;
use crate::map::{CampaignMap, MAP_DIM, MAP_TILES};
use crate::unit::Unit;

const T: &Tables = &Tables::DEFAULT;

fn two_county_map() -> CampaignMap {
    let mut m = CampaignMap::empty();
    for i in 0..MAP_TILES {
        m.county[i] = if i % MAP_DIM < 32 { 1 } else { 2 };
    }
    m
}

fn world() -> ([County; MAX_COUNTIES], [Realm; MAX_REALMS]) {
    let mut counties: [County; MAX_COUNTIES] = core::array::from_fn(|_| County::new());
    let mut realms: [Realm; MAX_REALMS] = core::array::from_fn(|_| Realm::new());
    for id in 1..=2 {
        counties[id].population = 500;
        counties[id].happiness = 77;
    }
    counties[1].owner = 1;
    realms[1].in_play = true;
    realms[1].is_human = true;
    realms[1].shield_index = 2;
    realms[2].in_play = true;
    bordering(&mut counties);
    (counties, realms)
}

fn attacker(units: &mut Units, owner: u8, county: u8) -> usize {
    let mut u = Unit::new(UnitKind::Army, owner, 40, 10);
    u.men = 400;
    u.troops[0] = 400;
    u.county = county;
    u.home_county = 1;
    units.spawn(u).unwrap()
}

