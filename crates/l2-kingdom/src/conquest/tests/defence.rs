#![allow(unused_imports)]
use super::*;
use super::capture_rules::*;
use super::change_owner::*;
use super::super::*;
use crate::explore::Explored;
use crate::map::{CampaignMap, MAP_DIM, MAP_TILES};
use crate::unit::Unit;

fn standing(units: &mut Units, owner: u8, x: u8, y: u8, men: i32) -> usize {
    let mut u = Unit::new(UnitKind::Army, owner, x, y);
    u.men = men;
    u.county = 2;
    units.spawn(u).unwrap()
}

fn owned_county_two(anchor: (u8, u8)) -> ([County; MAX_COUNTIES], [Realm; MAX_REALMS]) {
    let (mut counties, mut realms) = world();
    counties[2].owner = 2;
    counties[2].anchor_x = anchor.0;
    counties[2].anchor_y = anchor.1;
    realms[2].is_human = false;
    (counties, realms)
}

#[test]
fn an_army_four_rows_from_the_town_does_not_defend_it() {
    let (counties, _) = owned_county_two((31, 50));

    let mut units = Units::new();
    standing(&mut units, 2, 30, 46, 300);
    assert_eq!(
        find_defender(&units, &counties, 2),
        None,
        "battle-before.sav: the army at (30,46) is outside the town's 4x4 block"
    );

    let mut units = Units::new();
    let close = standing(&mut units, 2, 30, 49, 300);
    assert_eq!(
        find_defender(&units, &counties, 2),
        Some(close),
        "…and one row nearer, it defends"
    );
}

#[test]
fn the_defence_window_is_the_town_block_plus_its_one_tile_ring() {
    let (counties, _) = owned_county_two((31, 50));
    let inside = |x: u8, y: u8| {
        let mut units = Units::new();
        let u = standing(&mut units, 2, x, y, 300);
        find_defender(&units, &counties, 2) == Some(u)
    };

    for x in 29..=32u8 {
        for y in 48..=51u8 {
            assert!(inside(x, y), "({x},{y}) is inside the block");
        }
    }
    for (x, y) in [(28, 50), (33, 50), (31, 47), (31, 52), (28, 47), (33, 52)] {
        assert!(!inside(x, y), "({x},{y}) is outside it");
    }
}

#[test]
fn the_largest_army_beside_the_town_defends_it_not_the_earliest_slot() {
    let (counties, _) = owned_county_two((31, 50));
    let mut units = Units::new();
    let _small = standing(&mut units, 2, 29, 48, 40);
    let big = standing(&mut units, 2, 32, 51, 400);
    let _middling = standing(&mut units, 2, 31, 50, 200);
    assert_eq!(find_defender(&units, &counties, 2), Some(big));
}

#[test]
fn the_defence_search_ignores_other_realms_and_other_unit_kinds() {
    let (counties, _) = owned_county_two((31, 50));
    let mut units = Units::new();
    standing(&mut units, 1, 31, 50, 900);
    let mut trader = Unit::new(UnitKind::Merchant, 2, 30, 50);
    trader.men = 900;
    trader.county = 2;
    units.spawn(trader).unwrap();
    assert_eq!(find_defender(&units, &counties, 2), None);

    let own = standing(&mut units, 2, 29, 49, 10);
    assert_eq!(find_defender(&units, &counties, 2), Some(own), "ten men still beat nobody");
}

#[test]
fn a_town_against_the_map_edge_finds_nobody() {
    for anchor in [(1u8, 50u8), (63, 50), (31, 1), (31, 63)] {
        let (counties, _) = owned_county_two(anchor);
        let mut units = Units::new();
        standing(&mut units, 2, anchor.0, anchor.1, 300);
        assert_eq!(find_defender(&units, &counties, 2), None, "anchor {anchor:?}");
    }
}

#[test]
fn attacking_your_own_county_is_refused() {
    let m = two_county_map();
    let (mut counties, mut realms) = world();
    let mut units = Units::new();
    let mut names = ArmyNames::new();
    let a = attacker(&mut units, 1, 1);
    assert_eq!(
        attack_county(T, &m, &mut counties, &mut realms, &mut units, &mut names, a, 1, 0, 1268, &mut Explored::new(), Restore::NEUTRAL),
        Attack::Refused(Refusal::AlreadyYours)
    );
}

#[test]
fn a_merchant_cannot_take_a_county() {
    let m = two_county_map();
    let (mut counties, mut realms) = world();
    let mut units = Units::new();
    let mut names = ArmyNames::new();
    let trader = units.spawn(Unit::new(UnitKind::Merchant, 1, 40, 10)).unwrap();
    assert_eq!(
        attack_county(T, &m, &mut counties, &mut realms, &mut units, &mut names, trader, 2, 0, 1268, &mut Explored::new(), Restore::NEUTRAL),
        Attack::Refused(Refusal::NotAnArmy)
    );
}


