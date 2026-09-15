#![allow(unused_imports)]
use super::*;
use super::ownership::*;
use super::attack::*;
use crate::county::{County, MAX_COUNTIES};
use crate::levy::{self, Defence};
use crate::map::CampaignMap;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::{Season, Tables};
use crate::unit::{ArmyNames, UnitKind, Units};

/// `Unit_ReachCastleBuilding` (`0x004686A0`) — **the fork an army walks into**,
///
/// Two unrelated functions agree on the test — `Map_HoverUnitTarget` offers
/// *"Garrison castle?"* or *"Besiege castle?"* from the same owner comparison —
/// which is what makes it `[V]`.
pub fn reach_castle_building(
    t: &Tables,
    map: &CampaignMap,
    counties: &mut [County; MAX_COUNTIES],
    realms: &[Realm; MAX_REALMS],
    units: &mut Units,
    army: usize,
    county: u8,
    season: u8,
) -> CastleArrival {
    let Some(u) = units.get(army) else { return CastleArrival::NotAnArmy };
    if u.kind != UnitKind::Army {
        return CastleArrival::NotAnArmy;
    }
    let owner = u.owner;
    if counties.get(county as usize).map(|c| c.owner) == Some(owner) {
        match garrison_apply(t, map, counties, realms, units, army, county) {
            Some(slot) => CastleArrival::Garrisoned(slot),
            None => CastleArrival::GarrisonFull,
        }
    } else {
        CastleArrival::Siege(crate::siege::begin_siege(
            t, counties, realms, units, army, county, season,
        ))
    }
}

/// **`Army_LeaveCastle` (`0x004374C4`) and its body `FUN_00437535`** — the
/// garrison marches out.
///
/// ```c
/// /* 0x004374C4: the garrisoned info panel's first button */
/// g_screenId = 0;
/// if (!g_multiplayer) FUN_00437535(g_pickedTileUnit, g_pickedTileCounty);
/// else                Net_SendCommand(0x36, 0);
///
/// /* 0x00437535 */
/// if (!Map_FindFreeTileNear(unit.x, unit.y)) { Army_Destroy(unit); return; }
/// Unit_UnlinkFromTile(unit);
/// unit.x = g_foundTileX;  unit.y = g_foundTileY;
/// unit.garrisonCounty = 0;  counties[county].garrisonUnit = 0;
/// Unit_LinkToTile(unit);
/// if (unit.besiegedBy && Battle_BeginFromCampaign(unit, unit.besiegedBy))
///     g_battleCounty = county;
/// Army_RecountCountyTroops();
/// ```
///
/// The tile search is `Map_FindFreeTileNear` (`0x0046733C`), which is
/// [`crate::divide::free_tile_near`] — the road-preferred five-ring
/// `Army_Split` uses for the daughter. `[V]`: the same call in both bodies.
pub fn leave_castle(
    map: &CampaignMap,
    counties: &mut [County; MAX_COUNTIES],
    realms: &[Realm; MAX_REALMS],
    units: &mut Units,
    unit: usize,
    county: u8,
) -> LeftCastle {
    let Some(u) = units.get(unit).filter(|u| u.garrison_county == county) else {
        return LeftCastle::NotAGarrison;
    };
    let (from, sortie) = (u.tile(), (u.besieged_by != 0).then_some(u.besieged_by as usize));
    let spot = crate::divide::free_tile_near(map, units, from.0, from.1);
    if let Some(c) = counties.get_mut(county as usize) {
        if c.garrison_unit == unit {
            c.garrison_unit = 0;
        }
    }
    let Some((x, y)) = spot else {
        units.remove(unit);
        units.recount_county_troops(counties, realms);
        return LeftCastle::Destroyed;
    };
    if let Some(u) = units.get_mut(unit) {
        u.x = x;
        u.y = y;
        u.garrison_county = 0;
    }
    units.recount_county_troops(counties, realms);
    LeftCastle::Marched { tile: (x, y), sortie }
}

/// `Army_GarrisonApply` (`0x004A79A3`) — **put an army in the castle**.
///
/// `map` is read only, for the castle tile: `County_FindCastleTile` caches it in
/// county `+0x74`/`+0x75` and [`crate::map::castle_tile`] finds it instead.
pub fn garrison_apply(
    t: &Tables,
    map: &CampaignMap,
    counties: &mut [County; MAX_COUNTIES],
    realms: &[Realm; MAX_REALMS],
    units: &mut Units,
    army: usize,
    county: u8,
) -> Option<usize> {
    let sitting = counties.get(county as usize)?.garrison_unit;
    if sitting != 0 && sitting == army {
        return Some(army);
    }
    let castle_type = counties[county as usize].castle_type;
    if let Some(u) = units.get_mut(army) {
        u.needs_destination = true;
    }
    let men = units.get(army)?.men + units.get(sitting).map_or(0, |g| g.men);
    if men > crate::industry::garrison_cap(t, castle_type) {
        if let Some(u) = units.get_mut(army) {
            u.moving = false;
            u.mission = crate::ai_army::Mission::SEEK_ENEMY;
        }
        return None;
    }
    let slot = if sitting == 0 {
        let tile = crate::map::castle_tile(map, county);
        let u = units.get_mut(army)?;
        u.garrison_county = county;
        if let Some(tile) = tile {
            let (x, y) = crate::map::coords(tile);
            u.x = x;
            u.y = y;
        }
        u.path.clear();
        u.moving = false;
        u.needs_destination = true;
        u.player_driven = true;
        u.moves_used += GARRISON_MOVE_COST;
        u.dest_county = county;
        u.county = county;
        counties[county as usize].garrison_unit = army;
        army
    } else {
        match crate::unit::combine(units, sitting, army) {
            Ok(_) => sitting,
            Err(_) => return None,
        }
    };
    units.recount_county_troops(counties, realms);
    Some(slot)
}

