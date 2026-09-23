
mod orders;
mod phase;

use super::*;
use crate::unit::Unit;

const T: &Tables = &Tables::DEFAULT;

fn besieged_county() -> ([County; MAX_COUNTIES], [Realm; MAX_REALMS], Units) {
    let mut counties: [County; MAX_COUNTIES] = core::array::from_fn(|_| County::new());
    let mut realms: [Realm; MAX_REALMS] = core::array::from_fn(|_| Realm::new());
    let mut units = Units::new();
    realms[1].in_play = true;
    realms[1].is_human = true;
    realms[2].in_play = true;
    realms[2].lord = 1;
    counties[4].owner = 2;
    counties[4].castle_type = 4;
    let mut garrison = Unit::new(UnitKind::Army, 2, 10, 10);
    garrison.men = 200;
    garrison.troops[0] = 200;
    garrison.garrison_county = 4;
    let g = units.spawn(garrison).unwrap();
    counties[4].garrison_unit = g;
    (counties, realms, units)
}

fn besieger(units: &mut Units, men: i32) -> usize {
    let mut a = Unit::new(UnitKind::Army, 1, 11, 10);
    a.owner_is_human = true;
    a.men = men;
    a.troops[0] = men;
    units.spawn(a).unwrap()
}

