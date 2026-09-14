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

// ---------------------------------------------------------------------------
// Step 7, pass 1 — where to muster
// ---------------------------------------------------------------------------

/// `FUN_004A0AAA`'s score for one county, as `(muster, raid)`.
///
/// ```c
/// s = (population / 500) * 2;
/// if      (happiness < 40) s -= 1;
/// else if (happiness > 80) s += 1;
/// if (foodAvailable < population)   { r = s + 2; s -= 2; }
/// else if (population * 4 < food)   { r = s - 2; s += 1; }
/// else                                r = s;
/// ```
///
/// **The two outputs take the food term in opposite directions**, and that is
/// the finding. The county the realm musters its main army from is the one
/// with **food to spare**; the county it raids *out of* is the one whose
/// larder is already short. A hungry county is where you send the surplus
/// mouths, and a fat one is where you raise the army that has to eat.
///
/// The population term is a *step*: `(population / 500) * 2` is 0 below 500,
/// 2 below 1000, 4 below 1500. A county of 999 people scores exactly what a
/// county of 500 does.
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
///
/// Returns `(muster county, raid county)`, either of which is **0 when no
/// county scores above zero**: both running maxima start at 0 and the
/// comparison is `>=`, so a realm every one of whose counties scores negative
/// musters nowhere. That is reachable — one small unhappy county with an empty
/// larder scores `0 - 1 - 2 = -3` — and it is how a realm on its last legs
/// stops raising armies.
///
/// The `>=` also means the **last** county on the highest score wins, where
/// every other scan in this module gives it to the first. Reproduced.
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

// ---------------------------------------------------------------------------
// Step 7, pass 2 — the castle garrisons
// ---------------------------------------------------------------------------

/// `FUN_0049F12F`'s sizing ladder: how much of a castle's garrison shortfall
/// the AI raises in one go, given the shortfall as a percentage of the
/// county's population.
///
/// `None` means the pass gives up on the county this turn.
///
/// | shortfall as % of the county | raised |
/// |---|---|
/// | 80 or more | **nothing at all** |
/// | 65 … 79 | a fifth of it |
/// | 50 … 64 | a quarter |
/// | 30 … 49 | a third |
/// | 15 … 29 | a half |
/// | 14 or less | all of it |
///
/// The ladder is a **cap**. Feed it any shortfall and the
/// number that comes out is between 13 % and 15 % of the county's population,
/// so a castle whose garrison is nearly empty is filled a slice at a time over
/// several turns.
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

// ---------------------------------------------------------------------------
// Step 7, pass 3 — hold the frontier or write it off
// ---------------------------------------------------------------------------

/// The men standing in a county that belong to `realm`, or that do not belong
/// to `realm` or its ally — `FUN_0049F850` and `FUN_0049F73F`.
///
/// Both walk every unit slot, take only **armies** (`kind == 1`) with a
/// non-zero owner, and ignore whether the unit is garrisoned or besieging —
/// unlike [`crate::unit::Units::recount_county_troops`], which excludes a
/// garrison. So a castle's garrison counts toward its own realm's total here.
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

/// What step 7's third pass wanted to move out of a county it has written off.
///
/// **Nothing is moved.** See the module documentation's *seams*: without
/// `Transport_Deliver` a spawned transport would delete the county's food from
/// the game, and without a county stall field the sale branch has no market.
/// The pass returns its intent so that neither is silently dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Evacuation {
    /// The county being written off.
    pub from: u8,
    /// Where the goods were bound — the realm's muster county.
    pub to: u8,
    pub grain: i32,
    pub herd: i32,
}

/// The evacuation only happens at all when there is something worth moving:
/// `grain > 10 || herd > 5`.
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
///
/// Returns whether anything was granted.
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

/// `g_year < 0x4F9`.
pub const EMERGENCY_WEAPONS_UNTIL_YEAR: i32 = 1273;
/// How many of each type the grant hands over.
pub const EMERGENCY_WEAPONS_GRANT: i32 = 100;

