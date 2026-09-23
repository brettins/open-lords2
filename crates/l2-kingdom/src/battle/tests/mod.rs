
mod resolve;
mod moves;
mod retreat;

use super::*;
use crate::unit::{TroopType, UnitKind};

const T: &Tables = &Tables::DEFAULT;

fn world() -> ([County; MAX_COUNTIES], [Realm; MAX_REALMS]) {
    let mut counties: [County; MAX_COUNTIES] = core::array::from_fn(|_| County::new());
    let mut realms: [Realm; MAX_REALMS] = core::array::from_fn(|_| Realm::new());
    for i in 1..4 {
        counties[i].population = 500;
        counties[i].happiness = 80;
    }
    realms[1].in_play = true;
    realms[1].is_human = true;
    realms[2].in_play = true;
    (counties, realms)
}

fn army(units: &mut Units, owner: u8, human: bool, troops: &[(TroopType, i32)]) -> usize {
    let mut u = Unit::new(UnitKind::Army, owner, 10, 10);
    u.owner_is_human = human;
    for &(t, n) in troops {
        u.troops[t.index()] = n;
    }
    u.men = u.troops.iter().sum();
    units.spawn(u).expect("a slot")
}


