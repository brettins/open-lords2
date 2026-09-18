#![allow(unused_imports)]
use super::*;
use super::unit_impl::*;
use super::units_impl::*;
use super::starvation::*;
use super::combine_part::*;
use super::destroy_part::*;
use super::*;
use crate::county::{County, MAX_COUNTIES};
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;

/// `Wages_ForRealm` (`0x004AD495`) — sum `Wages_ForUnit` over every **type-1**
/// unit this realm owns.
pub fn wages_for_realm(t: &Tables, units: &Units, realms: &[Realm; MAX_REALMS], realm: u8, difficulty: u8) -> i32 {
    let Some(r) = realms.get(realm as usize) else { return 0 };
    let mut total = 0;
    for (_, u) in units.iter() {
        if u.owner == realm && u.kind == UnitKind::Army {
            total += r.wage_for_unit(t, u.men, difficulty);
        }
    }
    total
}

/// The per-unit number is `L2.eng` 31/8 *"Wages"* on the army panel, and it is
/// the second source that makes `+0x15C` `[V]`. Kept in step with the realm
/// total by being computed in the same pass.
pub fn refresh_wages(t: &Tables, units: &mut Units, realms: &mut [Realm; MAX_REALMS], realm: u8, difficulty: u8) -> i32 {
    let Some(r) = realms.get(realm as usize) else { return 0 };
    let mut total = 0;
    let mut per_unit: Vec<(usize, i32)> = Vec::new();
    for (i, u) in units.iter() {
        if u.owner == realm && u.kind == UnitKind::Army {
            let w = r.wage_for_unit(t, u.men, difficulty);
            per_unit.push((i, w));
            total += w;
        }
    }
    for (i, w) in per_unit {
        if let Some(u) = units.get_mut(i) {
            u.wages = w;
        }
    }
    realms[realm as usize].wages = total;
    total
}

