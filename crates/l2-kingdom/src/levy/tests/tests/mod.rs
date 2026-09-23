
mod basket;
pub use basket::*;
mod muster;
pub use muster::*;
mod militia;
pub use militia::*;

use super::*;
use crate::map::MAP_TILES;

const T: &Tables = &Tables::DEFAULT;

fn open_map() -> CampaignMap {
    let mut m = CampaignMap::empty();
    for i in 0..MAP_TILES {
        m.county[i] = 1;
    }
    m
}

fn world() -> ([County; MAX_COUNTIES], [Realm; MAX_REALMS]) {
    let mut counties: [County; MAX_COUNTIES] = core::array::from_fn(|_| County::new());
    let mut realms: [Realm; MAX_REALMS] = core::array::from_fn(|_| Realm::new());
    counties[1].owner = 1;
    counties[1].population = 500;
    counties[1].happiness = 100;
    realms[1].in_play = true;
    realms[1].is_human = true;
    realms[1].shield_index = 3;
    (counties, realms)
}


