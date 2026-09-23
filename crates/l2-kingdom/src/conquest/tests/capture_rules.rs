#![allow(unused_imports)]
use super::*;
use super::defence::*;
use super::change_owner::*;
use super::super::*;
use crate::explore::Explored;
use crate::map::{CampaignMap, MAP_DIM, MAP_TILES};
use crate::unit::Unit;

#[test]
fn a_county_needs_both_a_castle_and_a_garrison_to_be_shut() {
    let (mut counties, _) = world();
    let mut units = Units::new();
    let garrison = units.spawn(Unit::new(UnitKind::Army, 1, 0, 0)).unwrap();

    counties[1].castle_type = 0;
    counties[1].garrison_unit = 0;
    assert!(can_be_entered(&counties, &units, 1, 2), "open country");

    counties[1].castle_type = 3;
    assert!(can_be_entered(&counties, &units, 1, 2), "a castle nobody is in");

    counties[1].castle_type = 0;
    counties[1].garrison_unit = garrison;
    assert!(can_be_entered(&counties, &units, 1, 2), "a garrison with no castle");

    counties[1].castle_type = 3;
    assert!(!can_be_entered(&counties, &units, 1, 2), "both: this is a siege");
    assert!(can_be_entered(&counties, &units, 1, 1), "…unless the garrison is yours");
}

#[test]
fn the_shipped_position_has_no_sieges_in_it() {
    let (counties, _) = world();
    let units = Units::new();
    for id in 1..=2u8 {
        assert_eq!(counties[id as usize].garrison_unit, 0);
        assert!(can_be_entered(&counties, &units, id, 9));
    }
}

#[test]
fn a_shut_county_refuses_the_attack_rather_than_starting_a_battle() {
    let m = two_county_map();
    let (mut counties, mut realms) = world();
    let mut units = Units::new();
    let mut names = ArmyNames::new();
    let a = attacker(&mut units, 2, 1);
    let g = units.spawn(Unit::new(UnitKind::Army, 1, 1, 1)).unwrap();
    counties[1].castle_type = 4;
    counties[1].garrison_unit = g;

    let out = attack_county(T, &m, &mut counties, &mut realms, &mut units, &mut names, a, 1, 0, 1268, &mut Explored::new(), Restore::NEUTRAL);
    assert_eq!(out, Attack::Refused(Refusal::Garrisoned));
    assert_eq!(counties[1].owner, 1, "and nothing changed hands");
    assert_eq!(units.get(a).unwrap().moves_used, 0, "not even the eight moves");
}

#[test]
fn a_wretched_neutral_county_surrenders_and_a_contented_one_fights() {
    let m = two_county_map();
    let (mut counties, mut realms) = world();
    counties[2].happiness = 10;
    let mut units = Units::new();
    let mut names = ArmyNames::new();
    let a = attacker(&mut units, 1, 2);

    let out = attack_county(T, &m, &mut counties, &mut realms, &mut units, &mut names, a, 2, 0, 1268, &mut Explored::new(), Restore::NEUTRAL);
    assert!(matches!(out, Attack::Captured(_)), "got {out:?}");
    assert_eq!(counties[2].owner, 1);
    assert_eq!(units.get(a).unwrap().moves_used, ATTACK_MOVE_COST);

    let (mut counties, mut realms) = world();
    counties[2].happiness = SURRENDER_HAPPINESS;
    let mut units = Units::new();
    let a = attacker(&mut units, 1, 2);
    let out = attack_county(T, &m, &mut counties, &mut realms, &mut units, &mut names, a, 2, 0, 1268, &mut Explored::new(), Restore::NEUTRAL);
    assert!(matches!(out, Attack::Battle { .. }), "got {out:?}");
    assert_eq!(counties[2].owner, 0, "the county is not taken by walking in");
}

#[test]
fn a_neutral_county_at_the_shipped_happiness_raises_a_militia() {
    let m = two_county_map();
    let (mut counties, mut realms) = world();
    let mut units = Units::new();
    let mut names = ArmyNames::new();
    let a = attacker(&mut units, 1, 2);

    let out = attack_county(T, &m, &mut counties, &mut realms, &mut units, &mut names, a, 2, 0, 1268, &mut Explored::new(), Restore::NEUTRAL);
    let Attack::Battle { attacker: att, defender } = out else { panic!("got {out:?}") };
    assert_eq!(att, a);
    let d = units.get(defender).unwrap();
    assert_eq!(d.owner, crate::levy::OWNERLESS);
    assert_eq!(d.men, 125, "a quarter of five hundred, at difficulty 0");
    assert_eq!(d.county, 2);
}

#[test]
fn a_county_of_fewer_than_forty_people_is_simply_taken() {
    let m = two_county_map();
    let (mut counties, mut realms) = world();
    counties[2].population = 30;
    let mut units = Units::new();
    let mut names = ArmyNames::new();
    let a = attacker(&mut units, 1, 2);
    assert!(matches!(
        attack_county(T, &m, &mut counties, &mut realms, &mut units, &mut names, a, 2, 0, 1268, &mut Explored::new(), Restore::NEUTRAL),
        Attack::Captured(_)
    ));
    assert_eq!(counties[2].owner, 1);
}

#[test]
fn an_existing_army_in_an_owned_county_defends_it_rather_than_a_fresh_levy() {
    let m = two_county_map();
    let (mut counties, mut realms) = world();
    counties[2].owner = 2;
    counties[2].anchor_x = 45;
    counties[2].anchor_y = 20;
    let mut units = Units::new();
    let mut names = ArmyNames::new();
    let a = attacker(&mut units, 1, 2);
    let mut standing = Unit::new(UnitKind::Army, 2, 45, 20);
    standing.men = 300;
    standing.county = 2;
    let existing = units.spawn(standing).unwrap();

    let out = attack_county(T, &m, &mut counties, &mut realms, &mut units, &mut names, a, 2, 0, 1268, &mut Explored::new(), Restore::NEUTRAL);
    assert_eq!(out, Attack::Battle { attacker: a, defender: existing });
    assert_eq!(units.len(), 2, "nothing new was levied");
    assert_eq!(counties[2].population, 500, "and nobody was called up");
}


