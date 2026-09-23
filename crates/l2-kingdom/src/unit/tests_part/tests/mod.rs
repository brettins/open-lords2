
mod slots;
pub use slots::*;
mod recount;
pub use recount::*;

use super::*;
use crate::tables::Tables;

const T: &Tables = &Tables::DEFAULT;

fn army(owner: u8, men: i32) -> Unit {
    let mut u = Unit::new(UnitKind::Army, owner, 10, 10);
    u.men = men;
    u.troops[TroopType::Peasant.index()] = men;
    u
}

