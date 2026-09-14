#![allow(unused_imports)]
use super::*;
use super::world_tests::*;
use super::validation_tests::*;
use super::*;
use super::diff::*;
use l2_formats::maps::{MapSet, Plane, PLANE_DIM, SLOT_LEN};
use l2_kingdom::map::MAP_TILES;
use l2_scenario::newgame::{self, MapError, NewGame};
use l2_scenario::{CountyState, Scenario};

/// A map that seats two cannot be started with five lords, and the refusal is a
/// refusal.
#[test]
fn a_two_seat_map_refuses_five_lords() {
    let bytes = maps!();
    let set = MapSet::parse(&bytes).expect("L2_maps.dat parses");
    let two_seaters: Vec<usize> = set
        .used_slots()
        .into_iter()
        .filter(|&i| set.slot(i).unwrap().player_start_count() == 2)
        .collect();
    assert_eq!(two_seaters.len(), 8, "eight shipped maps seat two — maps-layers.md §5.1");
    let slot_index = two_seaters[0];
    let slot = set.slot(slot_index).expect("a used slot");
    let setup = NewGame { slot: slot_index, lords: 5, ..NewGame::default() };
    assert_eq!(
        Scenario::from_map(&slot, &setup),
        Err(MapError::TooManyLords { lords: 5, seats: 2 })
    );
}

/// **Which realm you play is rolled, and the seed is what rolls it.**
#[test]
fn the_start_table_is_dealt_and_the_deal_follows_the_seed() {
    let bytes = maps!();
    let set = MapSet::parse(&bytes).expect("L2_maps.dat parses");
    let slot = set.slot(ENGLAND).expect("slot 0");
    let owner_of = |seed: u64| {
        let setup = NewGame { slot: ENGLAND, lords: 5, local_player: 1, seed, ..NewGame::default() };
        let s = Scenario::from_map(&slot, &setup).expect("England builds");
        let mut v: Vec<(u8, u8)> = (1..=s.county_count)
            .filter_map(|id| s.counties[id].as_ref().map(|c| (id as u8, c.owner)))
            .filter(|&(_, o)| o != 0)
            .collect();
        v.sort_unstable();
        v
    };
    let a = owner_of(1);
    // The same seed twice is the same world — a lockstep peer's whole
    // requirement of this constructor.
    assert_eq!(a, owner_of(1), "the deal is a function of the seed");
    // The set of owned counties never moves; only who owns them.
    for seed in 0..24u64 {
        let deal = owner_of(seed);
        let counties: Vec<u8> = deal.iter().map(|&(c, _)| c).collect();
        assert_eq!(counties, ENGLAND_STARTS, "seed {seed}: the start counties moved");
        let mut realms: Vec<u8> = deal.iter().map(|&(_, r)| r).collect();
        realms.sort_unstable();
        assert_eq!(realms, vec![1, 2, 3, 4, 5], "seed {seed}: one county each");
    }
    let deals: std::collections::BTreeSet<Vec<(u8, u8)>> =
        (0..64u64).map(owner_of).collect();
    assert!(deals.len() > 1, "the deal never moves, so it is not a deal");
}

// ------------------------------------------------------------------ the diff

];

