#![allow(unused_imports)]
use super::*;
use super::wants::*;
use super::muster::*;
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


/// `FUN_004A01EA` — realm `+0x48`, **who the realm has decided is the threat**.
pub fn pick_threat(realms: &[Realm; MAX_REALMS], realm_id: u8) -> u8 {
    let Some(me) = realms.get(realm_id as usize) else { return 0 };
    if me.war_target != 0 {
        return me.war_target;
    }
    let mut threat = 0;
    for i in 1..MAX_REALMS.min(6) {
        if realms[i].rank >= 2 {
            continue;
        }
        if i as u8 == realm_id || realms[i].strength == 0 || realms[i].share_of_map_pct < THREAT_SHARE_PCT
        {
            return threat;
        }
        threat = i as u8;
    }
    threat
}

/// `wide_grain` picks which grain ladder: **true** is `FUN_004A03F2`'s, used by
/// step 9 and step 10, and **false** is `FUN_004A0649`'s, used by
/// [`Mission::SEEK_ENEMY`] when an army picks its own next target.
///
/// | grain | `FUN_004A03F2` | `FUN_004A0649` |
/// |---|---:|---:|
/// | 2501+ | −20 | −10 |
/// | 1201 … 2500 | −10 | −10 |
/// | 1001 … 1200 | −5 | −10 |
/// | 501 … 1000 | −5 | −5 |
pub fn target_score(from: &County, cand: &County, wide_grain: bool) -> i32 {
    let mut score = chebyshev(
        (from.anchor_x, from.anchor_y),
        (cand.anchor_x, cand.anchor_y),
    );
    if cand.castle_type != 0 && cand.garrison_unit != 0 {
        score += DEFENDED_CASTLE_PENALTY;
    }
    if cand.herd >= 201 {
        score -= 20;
    } else if cand.herd > 100 {
        score -= 10;
    }
    if wide_grain {
        if cand.grain >= 2501 {
            score -= 20;
        } else if cand.grain >= 1201 {
            score -= 10;
        } else if cand.grain > 500 {
            score -= 5;
        }
    } else if cand.grain >= 1001 {
        score -= 10;
    } else if cand.grain > 500 {
        score -= 5;
    }
    score
}

pub fn chebyshev((ax, ay): (u8, u8), (bx, by): (u8, u8)) -> i32 {
    let dx = (ax as i32 - bx as i32).abs();
    let dy = (ay as i32 - by as i32).abs();
    dx.max(dy)
}

pub fn manhattan((ax, ay): (u8, u8), (bx, by): (u8, u8)) -> i32 {
    (ax as i32 - bx as i32).abs() + (ay as i32 - by as i32).abs()
}

/// `Diplo_ActionAllowed` (`0x004A16F7`) — may `actor` act against `target`?
pub fn action_allowed(realms: &mut [Realm; MAX_REALMS], actor: u8, target: u8) -> bool {
    if target == 0 {
        return true;
    }
    let Some(me) = realms.get_mut(actor as usize) else { return false };
    if me.ally == target {
        let pair = me.pair_mut(target);
        pair.grudge = pair.grudge.wrapping_add(1);
        return false;
    }
    actor != target
}

/// `FUN_00467EB0` — does any of `county`'s neighbours belong to `realm`?
pub fn county_borders_realm(counties: &[County; MAX_COUNTIES], county: u8, realm: u8) -> bool {
    let Some(c) = counties.get(county as usize) else { return false };
    c.neighbours()
        .iter()
        .take_while(|&&n| n != 0)
        .any(|&n| counties.get(n as usize).is_some_and(|c| c.owner == realm))
}

/// `FUN_0049FF71` — is the ally's request still worth marching on?
///
/// Returns the county, or 0 to cancel: **no ally** cancels it, a county the
/// asking realm no longer holds *with no enemy standing in it* cancels it, and
/// a county that has become the asked realm's **own** cancels it. The middle
/// test reads [`crate::county::County::enemy_troops`] (`+0x19C`), which
/// `Army_RecountCountyTroops` maintains — so it is literally *"my ally still
/// owns it and there are still enemies in it"*.
pub fn ally_request_still_stands(
    counties: &[County; MAX_COUNTIES],
    realms: &[Realm; MAX_REALMS],
    county: u8,
    realm: u8,
) -> u8 {
    let (Some(c), Some(me)) = (counties.get(county as usize), realms.get(realm as usize)) else {
        return 0;
    };
    if me.ally == 0 {
        return 0;
    }
    if me.ally == c.owner {
        if c.enemy_troops == 0 {
            return 0;
        }
    } else if c.owner == realm {
        return 0;
    }
    county
}

/// `FUN_004A0309` — who to raid: the declared war target, else the rival this
/// realm thinks worst of, provided that is **strictly below**
/// [`RAID_STANDING_THRESHOLD`].
pub fn pick_raid_victim(realms: &[Realm; MAX_REALMS], realm_id: u8) -> u8 {
    let Some(me) = realms.get(realm_id as usize) else { return 0 };
    if me.war_target != 0 {
        return me.war_target;
    }
    let mut worst = RAID_STANDING_THRESHOLD;
    let mut victim = 0;
    for i in 1..MAX_REALMS.min(6) {
        if i as u8 == realm_id || realms[i].strength == 0 {
            continue;
        }
        let standing = me.pair(i as u8).standing;
        if standing < worst {
            worst = standing;
            victim = i as u8;
        }
    }
    victim
}

