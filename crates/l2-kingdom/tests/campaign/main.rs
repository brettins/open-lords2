//! The campaign layer driven through whole seasons — `docs/armies.md`.
//!
//! The unit tests in `src/` each check one rule against the decompiled function
//! it came from. This checks the things that only exist once the rules are
//! *composed into a turn*, which is where the hooks `docs/armies.md` §6.4 calls
//! *"the missing half"*
//!
//! * the starvation ladder over five consecutive seasons, including the
//!   ordering that makes an army desert and then be billed the reduced wage in
//!   the same season;
//! * army food as extra mouths at the county's ration level, which is a
//!   *multiplier* on the ration and not a flat subtraction;
//! * the mercenary bands walking the map for a year;
//! * `disabled_seasons` counting itself back down after a trampling;
//! * a whole kingdom with armies in it round-tripping through a save and
//!   playing on identically.
//!
//! Needs no game install: everything here is built by hand.

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

/// A two-county kingdom with a map: county 1 in the west, county 2 in the east,
/// realm 1 holding the first and nobody holding the second.
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
    // **Counties 1 and 2 are neighbours.** Without an adjacency list
    // `County_BordersRealm` says no and `County_ChangeOwner` takes its `else`
    // branch: the county declares independence instead of changing hands.
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

