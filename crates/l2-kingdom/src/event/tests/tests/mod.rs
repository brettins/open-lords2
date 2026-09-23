
mod deck;
pub use deck::*;
mod stream;
pub use stream::*;

use super::*;

const Q: Quirks = Quirks::FAITHFUL;

const T: &Tables = &Tables::DEFAULT;
use std::collections::BTreeSet;

fn county(owner: u8) -> County {
    let mut c = County::new();
    c.owner = owner;
    c
}

