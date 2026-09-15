#![allow(unused_imports)]
use super::*;
use super::wants::*;
use super::raid::*;
use super::aim::*;
use super::army::*;
use crate::county::{County, MAX_COUNTIES};
use crate::kingdom::Kingdom;
use crate::levy::{self, LevyBasket, Muster};
use crate::map::{flags, CampaignMap, MAP_DIM};
use crate::math::pct;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use crate::unit::{TroopType, UnitKind, Units};


/// `FUN_004A0AAA`'s score for one county, as `(muster, raid)`.
pub fn muster_score(t: &Tables, county: &County) -> (i32, i32) {
    let food = crate::ration::food_available(t, county);
    let mut s = (county.population / 500) * 2;
    if county.happiness < 40 {
        s -= 1;
    } else if county.happiness > 80 {
        s += 1;
    }
    let r;
    if food < county.population {
        r = s + 2;
        s -= 2;
    } else if county.population * 4 < food {
        r = s - 2;
        s += 1;
    } else {
        r = s;
    }
    (s, r)
}

/// Step 7's first pass — `FUN_004A0AAA`, which writes realm `+0xE5` and
/// `+0xE6`.
pub fn choose_muster_counties(
    t: &Tables,
    counties: &[County; MAX_COUNTIES],
    county_count: usize,
    realm_id: u8,
) -> (u8, u8) {
    let (mut best, mut best_score) = (0u8, 0i32);
    let (mut raid, mut raid_score) = (0u8, 0i32);
    for id in 1..=county_count.min(MAX_COUNTIES - 1) {
        if counties[id].owner != realm_id {
            continue;
        }
        let (s, r) = muster_score(t, &counties[id]);
        if s >= best_score {
            best_score = s;
            best = id as u8;
        }
        if r >= raid_score {
            raid_score = r;
            raid = id as u8;
        }
    }
    (best, raid)
}


/// `FUN_0049F12F`'s sizing ladder: how much of a castle's garrison shortfall
/// the AI raises in one go, given the shortfall as a percentage of the
/// county's population.
pub fn garrison_levy_share(gap: i32, population: i32) -> Option<i32> {
    let share = crate::industry::pct_of(gap, population);
    if share >= GARRISON_GAP_ABANDON_PCT {
        return None;
    }
    Some(if share >= 65 {
        gap / 5
    } else if share >= 50 {
        gap / 4
    } else if share >= 30 {
        gap / 3
    } else if share >= 15 {
        gap / 2
    } else {
        gap
    })
}


/// The men standing in a county that belong to `realm`, or that do not belong
/// to `realm` or its ally — `FUN_0049F850` and `FUN_0049F73F`.
///
/// `[D]`
pub fn men_in_county(units: &Units, realms: &[Realm; MAX_REALMS], county: u8, realm: u8, hostile: bool) -> i32 {
    let ally = realms.get(realm as usize).map_or(0, |r| r.ally);
    let mut men = 0;
    for (_, u) in units.iter() {
        if u.kind != UnitKind::Army || u.owner == 0 || u.county != county {
            continue;
        }
        let theirs = u.owner != realm && u.owner != ally;
        if theirs == hostile {
            men += u.men;
        }
    }
    men
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Evacuation {
    pub from: u8,
    pub to: u8,
    pub grain: i32,
    pub herd: i32,
}

pub fn worth_evacuating(county: &County) -> bool {
    county.grain > 10 || county.herd > 5
}

/// `AI_EmergencyWeapons` (`0x0049FCC5`) — the charity that lets a cornered
/// lord muster anyway.
///
/// A realm down to **fewer than two counties**, before year **1273**, is
/// handed 100 pikes and 100 bows if its lord is the Bishop, or 100 crossbows
/// if it is the Countess. **The Knight and the Baron get nothing**, so two of
/// the four lords cannot be rescued at all and — because the grant's return
/// value is what bypasses
/// [`crate::tables::AI_PERSONALITY_MUSTER_ARMS`] — two of the four also cannot
/// muster below their weapon threshold. `[D]`, and `symbols.json` marks it
/// `[inferred]`; this is a second reader agreeing.
pub fn emergency_weapons(realm: &mut Realm, year: i32) -> bool {
    if realm.county_count >= 2 || year >= EMERGENCY_WEAPONS_UNTIL_YEAR {
        return false;
    }
    match realm.lord {
        4 => {
            realm.weapons[3] += EMERGENCY_WEAPONS_GRANT;
            realm.weapons[4] += EMERGENCY_WEAPONS_GRANT;
            true
        }
        3 => {
            realm.weapons[0] += EMERGENCY_WEAPONS_GRANT;
            true
        }
        _ => false,
    }
}

pub const EMERGENCY_WEAPONS_UNTIL_YEAR: i32 = 1273;
pub const EMERGENCY_WEAPONS_GRANT: i32 = 100;

