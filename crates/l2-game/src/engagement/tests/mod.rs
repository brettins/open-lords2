#![allow(unused_imports)]
use super::*;
use super::report::*;
use super::fight::*;
use super::siege::*;
use l2_kingdom::battle::{self, Aftermath, Settlement, Verdict};
use l2_kingdom::conquest::Attack;
use l2_kingdom::kingdom::Kingdom;
use l2_kingdom::unit::TROOP_TYPES;
use l2_sim::runner::{blank_field, BattleRunner, Muster};
use l2_sim::{End, Troop, SIDE_A, SIDE_B};

/// `docs/battle.md` §4.1: the campaign record's `+0x16C`, `TROOPS*.ENG`'s
/// columns and the `.skr` army record all agree on it — three independent
/// sources, so it is safe to cross between the crates by index at
/// all.
#[cfg(test)]
const COLUMN_ORDER: [(&str, &str); TROOP_TYPES] = [
    ("Peasant", "Peasants"),
    ("Crossbowman", "Crossbowmen"),
    ("Maceman", "Macemen"),
    ("Swordsman", "Swordsmen"),
    ("Pikeman", "Pikemen"),
    ("Archer", "Archers"),
    ("Knight", "Knights"),
];

#[test]
fn the_two_crates_number_the_troop_types_identically() {
    for (t, (kingdom, sim)) in COLUMN_ORDER.iter().enumerate() {
        assert_eq!(l2_kingdom::unit::ALL_TROOP_TYPES[t].index(), t);
        assert_eq!(l2_sim::ALL_TROOPS[t].index(), t);
        assert_eq!(
            format!("{:?}", l2_kingdom::unit::ALL_TROOP_TYPES[t]),
            *kingdom,
            "l2-kingdom column {t}"
        );
        assert_eq!(format!("{:?}", l2_sim::ALL_TROOPS[t]), *sim, "l2-sim column {t}");
    }
}

#[cfg(test)]
mod tests;

