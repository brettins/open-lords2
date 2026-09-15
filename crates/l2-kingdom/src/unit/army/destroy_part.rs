#![allow(unused_imports)]
use super::*;
use super::unit_impl::*;
use super::units_impl::*;
use super::wages::*;
use super::starvation::*;
use super::combine_part::*;
use super::*;
use crate::county::{County, MAX_COUNTIES};
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;

/// `Army_Destroy` (`0x004AA039`) — free the slot and put the realm back in
/// order.
///
/// 1. the garrison or siege link is broken, in whichever direction it points;
/// 2. the realm's name counter at `+0x2D + nameIndex` is **decremented by
///    one**;
/// 3. the realm's wage bill is recomputed from what is left.
///
/// > Corrected in the document. `[D]`
pub fn destroy(t: &Tables, units: &mut Units, realms: &mut [Realm; MAX_REALMS], names: &mut ArmyNames, id: usize, difficulty: u8) -> Option<Unit> {
    let unit = units.get(id)?.clone();
    if unit.garrison_county == 0 {
        if unit.besieging_county != 0 {
            for (_, other) in units.iter_mut() {
                if other.besieged_by == id as u8 {
                    other.besieged_by = 0;
                }
            }
        }
    } else {
        for (_, other) in units.iter_mut() {
            if other.besieging_county == unit.garrison_county {
                other.besieging_county = 0;
            }
        }
    }
    units.remove(id);
    names.release(unit.owner, unit.name_index);
    refresh_wages(t, units, realms, unit.owner, difficulty);
    Some(unit)
}


