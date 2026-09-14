//! Adjacency for synthetic test worlds.
//!
//! A world built by hand — `Game::new` plus `set_county_count` — has every
//! `County::neighbours` list empty, and an empty list is not a neutral
//! starting point. `County_BordersRealm` reads it, so `County_ChangeOwner`
//! sends every capture down its `else` branch (the county declares
//! independence) and `Realm_SecedeIsolatedCounties` treats a realm's counties
//! as disjoint blocks. Six tests in `l2-kingdom`'s `conquest` module and one in
//! `campaign` once went red for the want of two lines of neighbour data;
//! `l2-game/tests/military` recorded the same trap from the secession side.
//!
//! So adjacency is the default and its absence is spelled out:
//! [`chain_neighbours!`] on a world that wants a map,
//! [`isolated_counties!`] on one that means to test the no-adjacency branch.
//!
//! Neither macro names a type, so this crate stays free of `l2-kingdom`: both
//! expand against `counties`, `county_count` and `add_neighbour` at the call
//! site. Worlds loaded from a save need neither — the file carries the real
//! adjacency, and `l2-scenario` reads it.

/// Chain adjacency `1 — 2 — … — n` over a kingdom's counties, replacing
/// whatever each county listed before. Takes the kingdom by `&mut`-able
/// expression: `chain_neighbours!(game.kingdom)`.
///
/// A chain, not a ring: it is the shape the hand-written fixtures in
/// `tests/military`, `tests/castles` and `tests/arrival` already used, and it
/// leaves no county bordering every other one. Call it after
/// `set_county_count`, which is what it reads for `n`.
#[macro_export]
macro_rules! chain_neighbours {
    ($kingdom:expr) => {{
        let k = &mut $kingdom;
        let n = k.county_count;
        for id in 1..=n {
            let c = &mut k.counties[id];
            c.neighbour_count = 0;
            if id > 1 {
                c.add_neighbour(id as u8 - 1);
            }
            if id < n {
                c.add_neighbour(id as u8 + 1);
            }
        }
    }};
}

/// Every county a block of its own — no adjacency at all. The deliberate
/// choice, for a test whose subject *is* the branch an empty neighbour list
/// takes: an independence declaration, or a secession.
#[macro_export]
macro_rules! isolated_counties {
    ($kingdom:expr) => {{
        let k = &mut $kingdom;
        for id in 1..=k.county_count {
            k.counties[id].neighbour_count = 0;
        }
    }};
}
