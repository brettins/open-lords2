#![allow(unused_imports)]
use super::*;
use super::basket::*;
use super::militia::*;
use super::super::*;
use crate::map::MAP_TILES;

#[test]
fn morale_is_the_countys_happiness_before_the_levy_is_charged() {
    let m = open_map();
    let (mut counties, mut realms) = world();
    let mut units = Units::new();
    let mut names = ArmyNames::new();
    let basket = LevyBasket::seed(&realms[1], 100);

    let id = create_army(
        T,
        &m,
        &mut counties,
        &mut realms,
        &mut units,
        &mut names,
        &basket,
        Muster { realm: 1, county: 1, happiness_cost: 40, year: 1268 },
        &mut crate::explore::Explored::new(),
    )
    .unwrap();
    assert_eq!(units.get(id).unwrap().morale, 100, "not the 60 it is left at");
    assert_eq!(counties[1].happiness, 60);
}

#[test]
fn a_county_that_cannot_afford_the_cost_goes_to_zero_and_the_panel_agrees() {
    let m = open_map();
    let (mut counties, mut realms) = world();
    counties[1].happiness = 12;
    let mut units = Units::new();
    let mut names = ArmyNames::new();
    let basket = LevyBasket::seed(&realms[1], 100);

    create_army(
        T,
        &m,
        &mut counties,
        &mut realms,
        &mut units,
        &mut names,
        &basket,
        Muster { realm: 1, county: 1, happiness_cost: 50, year: 1268 },
        &mut crate::explore::Explored::new(),
    )
    .unwrap();
    assert_eq!(counties[1].happiness, 0);
    assert_eq!(counties[1].shown_army, -12, "what was taken, not the fifty asked for");
}

#[test]
fn an_army_is_mustered_on_a_road_tile_where_there_is_one() {
    let mut m = open_map();
    m.set_flags(20, 20, flags::ROAD);
    let units = Units::new();
    assert_eq!(muster_tile(&m, &units, (20, 20)), Some((20, 20)));

    let mut units = Units::new();
    units.spawn(Unit::new(UnitKind::Army, 1, 20, 20));
    assert_eq!(muster_tile(&m, &units, (20, 20)), Some((19, 19)));
}

/// **C47.** The finders search a box around the county's *anchor*, radius 1
/// then 2 then 3, and stop. A road tile four tiles away is out of reach,
#[test]
fn the_muster_never_reaches_further_than_three_tiles_from_the_anchor() {
    let mut m = open_map();
    m.set_flags(0, 0, flags::ROAD);
    m.set_flags(24, 20, flags::ROAD); // four east of the anchor
    let units = Units::new();
    let at = muster_tile(&m, &units, (20, 20)).expect("open ground beside the anchor");
    assert_eq!(at, (19, 19), "neither road is within three of (20, 20)");

    m.set_flags(23, 20, flags::ROAD);
    assert_eq!(muster_tile(&m, &units, (20, 20)), Some((23, 20)));
}

#[test]
fn farmland_is_not_open_ground_for_a_muster() {
    let mut m = open_map();
    for y in 16..25u8 {
        for x in 16..25u8 {
            m.set_flags(x, y, flags::FARMLAND);
        }
    }
    let units = Units::new();
    assert_eq!(muster_tile(&m, &units, (20, 20)), None);
}

#[test]
fn a_county_with_nowhere_to_stand_refuses_the_army() {
    let mut m = CampaignMap::empty();
    for i in 0..MAP_TILES {
        m.county[i] = 1;
        m.flags[i] = flags::NO_COUNTY;
    }
    let (mut counties, mut realms) = world();
    let mut units = Units::new();
    let mut names = ArmyNames::new();
    let basket = LevyBasket::seed(&realms[1], 100);
    assert_eq!(
        create_army(
            T,
            &m,
            &mut counties,
            &mut realms,
            &mut units,
            &mut names,
            &basket,
            Muster { realm: 1, county: 1, happiness_cost: 0, year: 1268 },
            &mut crate::explore::Explored::new(),
        ),
        Err(LevyRefusal::NowhereToStand)
    );
}

#[test]
fn a_neutral_countys_defence_is_ownerless_rather_than_owned_by_realm_zero() {
    let m = open_map();
    let (mut counties, mut realms) = world();
    counties[2].owner = 0;
    counties[2].population = 500;
    counties[2].happiness = 77;
    for i in 0..MAP_TILES {
        if i % 64 > 40 {
        }
    }
    let mut m2 = m.clone();
    for y in 0..crate::map::MAP_DIM as u8 {
        for x in 40..crate::map::MAP_DIM as u8 {
            m2.set_county(x, y, 2);
        }
    }
    let mut units = Units::new();
    let mut names = ArmyNames::new();

    let id = raise_defence(
        T, &m2, &mut counties, &mut realms, &mut units, &mut names, 2, 25, Defence::Militia, 1268,
        &mut crate::explore::Explored::new(),
    )
    .expect("five hundred people can defend themselves");
    let u = units.get(id).unwrap();
    assert_eq!(u.owner, OWNERLESS);
    assert_eq!(u.shield, 0);
    assert_eq!(u.men, 125, "a quarter of five hundred");
    assert_eq!(u.troops[TroopType::Archer.index()], 60);
    assert_eq!(u.troops[TroopType::Peasant.index()], 65);
    assert_eq!(counties[2].happiness, 77, "a militia costs the county nothing");
}

