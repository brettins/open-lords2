
mod standing;
pub use standing::*;
mod allies;
pub use allies::*;

use super::*;
use crate::realm::{LORD_BISHOP, LORD_ELIMINATED};

fn world() -> ([Realm; MAX_REALMS], Diplomacy) {
    let mut realms: [Realm; MAX_REALMS] = core::array::from_fn(|_| Realm::new());
    for (id, r) in realms.iter_mut().enumerate().take(MAX_REALMS).skip(1) {
        r.in_play = true;
        r.strength = 3;
        r.lord = ((id - 1) % 4 + 1) as u8;
        r.rank = id as u8;
        r.gold = 5000;
        r.population_mean = 4000;
    }
    realms[1].is_human = true;
    realms[1].lord = crate::realm::LORD_HUMAN;
    let mut d = Diplomacy::new(7);
    init(&mut realms, &mut d);
    (realms, d)
}

