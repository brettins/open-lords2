#![allow(unused_imports)]
use super::*;
use super::basket::*;
use super::muster::*;
use super::super::*;
use crate::map::MAP_TILES;

#[test]
fn the_militia_ladder_equips_by_size_and_arms_nobody_below_a_hundred_and_twenty() {
    for (men, want) in [
        (500, (150, 100, 50)),
        (480, (150, 100, 50)),
        (479, (100, 70, 0)),
        (360, (100, 70, 0)),
        (240, (80, 40, 0)),
        (120, (60, 0, 0)),
        (119, (0, 0, 0)),
        (50, (0, 0, 0)),
    ] {
        let found = MILITIA_LADDER
            .iter()
            .find(|&&(floor, ..)| men >= floor)
            .map(|&(_, a, p, m)| (a, p, m))
            .unwrap_or((0, 0, 0));
        assert_eq!(found, want, "{men} men");
    }
}

#[test]
fn a_county_of_fewer_than_forty_people_raises_no_defence_at_all() {
    let m = open_map();
    let (mut counties, mut realms) = world();
    counties[1].population = 39;
    let mut units = Units::new();
    let mut names = ArmyNames::new();
    assert!(raise_defence(
        T, &m, &mut counties, &mut realms, &mut units, &mut names, 1, 40, Defence::AiCounty, 1268,
        &mut crate::explore::Explored::new(),
    )
    .is_none());
    assert_eq!(units.len(), 0);
}

#[test]
fn a_human_countys_defence_is_all_peasants_however_full_the_armoury_is() {
    let m = open_map();
    let (mut counties, mut realms) = world();
    realms[1].weapons = [500; WEAPON_TYPE_COUNT];
    let mut units = Units::new();
    let mut names = ArmyNames::new();

    let id = raise_defence(
        T, &m, &mut counties, &mut realms, &mut units, &mut names, 1, DEFENCE_PERCENT,
        Defence::HumanCounty, 1268, &mut crate::explore::Explored::new(),
    )
    .unwrap();
    let u = units.get(id).unwrap();
    assert_eq!(u.men, 200);
    assert_eq!(u.troops, [200, 0, 0, 0, 0, 0, 0]);
    assert_eq!(realms[1].weapons, [500; WEAPON_TYPE_COUNT], "nothing was issued");
}

#[test]
fn an_ai_countys_defence_is_equipped_from_its_own_armoury() {
    let m = open_map();
    let (mut counties, mut realms) = world();
    realms[1].is_human = false;
    realms[1].weapons = [500; WEAPON_TYPE_COUNT];
    let mut units = Units::new();
    let mut names = ArmyNames::new();

    let id = raise_defence(
        T, &m, &mut counties, &mut realms, &mut units, &mut names, 1, DEFENCE_PERCENT,
        Defence::AiCounty, 1268, &mut crate::explore::Explored::new(),
    )
    .unwrap();
    let u = units.get(id).unwrap();
    assert_eq!(u.men, 200);
    assert!(u.troops[TroopType::Peasant.index()] < 10, "all but the last few are armed");
    assert_eq!(u.troops.iter().sum::<i32>(), 200);
}

#[test]
fn the_militia_difficulty_ladder_only_ever_takes_more_people() {
    let mut last = 0;
    for pct in MILITIA_PERCENT_BY_DIFFICULTY {
        assert!(pct > last, "the harder game must not levy fewer");
        last = pct;
    }
    assert_eq!(MILITIA_PERCENT_BY_DIFFICULTY[0], 25);
}

