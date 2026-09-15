#![allow(unused_imports)]
use super::*;
use super::split_part::*;
use crate::county::{County, MAX_COUNTIES};
use crate::map::{flags, CampaignMap};
use crate::mercenary::MercenaryBands;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use crate::unit::{
    ArmyNames, Mercenaries, TroopType, Unit, UnitKind, Units, MAX_UNIT_ID, TROOP_TYPES,
};


/// `Panel_DisbandButton` (`0x0043733A`) — **which county an army would disband
/// into**, or `None` when neither is the owner's.
///
/// ```c
/// c = unit.homeCounty;
/// if (unit.owner != counties[c].owner) c = unit.county;
/// if (unit.owner != counties[c].owner) message 0x91;   /* group 145 */
/// ```
///
/// The fallback is the county the army is *standing in*
/// Readme's remedy — *"move it to any friendly county before disbanding it"* —
/// works, and it is the same two clauses in the same order. `[V]`
pub fn disband_county(counties: &[County; MAX_COUNTIES], units: &Units, army: usize) -> Option<u8> {
    let unit = units.get(army).filter(|u| u.kind == UnitKind::Army)?;
    let owned = |c: u8| counties.get(c as usize).is_some_and(|county| county.owner == unit.owner);
    if owned(unit.home_county) {
        Some(unit.home_county)
    } else if owned(unit.county) {
        Some(unit.county)
    } else {
        None
    }
}

/// **`Army_Disband` (`0x00438681`)** — the army goes home and stops being an
/// army.
#[allow(clippy::too_many_arguments)]
pub fn disband(
    t: &Tables,
    counties: &mut [County; MAX_COUNTIES],
    realms: &mut [Realm; MAX_REALMS],
    units: &mut Units,
    names: &mut ArmyNames,
    bands: &mut MercenaryBands,
    army: usize,
    difficulty: u8,
) -> Result<(u8, i32), DisbandRefusal> {
    if units.get(army).filter(|u| u.kind == UnitKind::Army).is_none() {
        return Err(DisbandRefusal::NotAnArmy);
    }
    let county = disband_county(counties, units, army).ok_or(DisbandRefusal::NowhereToGo)?;
    bands.release(units, army);

    let unit = units.get(army).expect("still an army").clone();
    if let Some(realm) = realms.get_mut(unit.owner as usize) {
        for troop in crate::unit::ALL_TROOP_TYPES {
            if let Some(w) = troop.weapon_slot() {
                realm.weapons[w] += unit.troops[troop.index()];
            }
        }
    }
    if let Some(c) = counties.get_mut(county as usize) {
        c.population += unit.men;
        c.labour[crate::tables::JOB_IDLE_TOWNSFOLK] += unit.men;
        c.army += unit.men;
        if c.garrison_unit == army {
            c.garrison_unit = 0;
        }
    }
    let men = unit.men;
    crate::unit::destroy(t, units, realms, names, army, difficulty);
    let snapshot: [Realm; MAX_REALMS] = realms.clone();
    units.recount_county_troops(counties, &snapshot);
    Ok((county, men))
}

pub fn has_room(units: &Units) -> bool {
    units.free_slot().is_some_and(|s| s <= MAX_UNIT_ID)
}

