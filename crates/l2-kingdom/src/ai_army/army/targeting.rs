#![allow(unused_imports)]
use super::*;
use super::operations::*;
use super::movement::*;
use super::*;
use super::wants::*;
use super::muster::*;
use super::raid::*;
use super::aim::*;
use crate::county::{County, MAX_COUNTIES};
use crate::kingdom::Kingdom;
use crate::levy::{self, LevyBasket, Muster};
use crate::map::{flags, CampaignMap, MAP_DIM};
use crate::math::pct;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use crate::unit::{TroopType, UnitKind, Units};


/// `FUN_004673B8` — the nearest **enemy** army anywhere on the map, by
/// Chebyshev distance, as `(distance, slot)`. Distance is 1000 when there is
/// none.
///
/// Its filter is `owner != 0 && owner != mine && kind == 1 && !garrisoned`,
/// and it makes **no diplomacy check at all** — so an *allied* army is a
/// candidate here and is thrown out afterwards by
/// [`Mission::SEEK_ENEMY`]'s own test. The consequence is not cosmetic: a
/// nearer ally **masks** a slightly farther enemy, and the intercept is
/// skipped that turn. `[D]`
pub fn nearest_enemy_army(units: &Units, unit: usize) -> (i32, Option<usize>) {
    let Some(me) = units.get(unit) else { return (1000, None) };
    let mut best = 1000;
    let mut found = None;
    for (id, u) in units.iter() {
        if u.owner == 0 || u.owner == me.owner || u.kind != UnitKind::Army || u.garrison_county != 0
        {
            continue;
        }
        let d = chebyshev(me.tile(), u.tile());
        if d < best {
            best = d;
            found = Some(id);
        }
    }
    (best, found)
}

/// `FUN_00467532` — the nearest army **in one county** that
/// [`action_allowed`] lets this one attack.
pub fn nearest_attackable_army_in_county(
    units: &Units,
    realms: &mut [Realm; MAX_REALMS],
    unit: usize,
    county: u8,
) -> (i32, Option<usize>) {
    let Some(me) = units.get(unit) else { return (1000, None) };
    let (mine, my_tile) = (me.owner, me.tile());
    let mut best = 1000;
    let mut found = None;
    for (id, u) in units.iter() {
        if u.owner == 0 || u.kind != UnitKind::Army {
            continue;
        }
        if !action_allowed(realms, mine, u.owner) {
            continue;
        }
        if u.garrison_county != 0 || u.county != county {
            continue;
        }
        let d = chebyshev(my_tile, u.tile());
        if d < best {
            best = d;
            found = Some(id);
        }
    }
    (best, found)
}

/// `FUN_004A0649` — the target an army on [`Mission::SEEK_ENEMY`] picks for
/// itself when its standing order has run out.
pub fn pick_next_target(
    counties: &[County; MAX_COUNTIES],
    county_count: usize,
    realms: &mut [Realm; MAX_REALMS],
    realm_id: u8,
    from: u8,
) -> u8 {
    let mut best = TARGET_SCORE_CEILING;
    let mut found = 0u8;
    for id in 1..=county_count.min(MAX_COUNTIES - 1) {
        let owner = counties[id].owner;
        if owner == realm_id
            || !action_allowed(realms, realm_id, owner)
            || !county_borders_realm(counties, id as u8, realm_id)
        {
            continue;
        }
        let Some(from_county) = counties.get(from as usize) else { continue };
        let score = target_score(from_county, &counties[id], false);
        if score < best {
            best = score;
            found = id as u8;
        }
    }
    found
}

/// `FUN_004A07F2` — the **lowest-numbered** county of this realm whose castle
/// has room for `men`.
pub fn first_castle_with_room(
    t: &Tables,
    counties: &[County; MAX_COUNTIES],
    units: &Units,
    county_count: usize,
    realm: u8,
    men: i32,
) -> u8 {
    for id in 1..=county_count.min(MAX_COUNTIES - 1) {
        let c = &counties[id];
        if c.owner != realm
            || c.castle_type == 0
            || c.castle_degraded == crate::siege::CASTLE_DEGRADED_BUILDING
            || c.castle_ruined
        {
            continue;
        }
        let sitting = units.get(c.garrison_unit).map_or(0, |u| u.men);
        if men <= crate::industry::garrison_cap(t, c.castle_type) - sitting {
            return id as u8;
        }
    }
    0
}

