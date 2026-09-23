#![allow(unused_imports)]
use super::*;
use super::slots::*;
use super::super::*;
use crate::tables::Tables;

fn kingdom_bits() -> ([County; MAX_COUNTIES], [Realm; MAX_REALMS]) {
    let counties = core::array::from_fn(|_| County::new());
    let realms = core::array::from_fn(|_| Realm::new());
    (counties, realms)
}

#[test]
fn the_troop_recount_splits_friendly_from_enemy_by_the_countys_owner() {
    let (mut counties, mut realms) = kingdom_bits();
    counties[1].owner = 1;
    counties[2].owner = 2;
    realms[1].in_play = true;
    realms[2].in_play = true;

    let mut units = Units::new();
    let mut mine = army(1, 300);
    mine.county = 1;
    let mut invading = army(1, 200);
    invading.county = 2;
    units.spawn(mine);
    units.spawn(invading);

    units.recount_county_troops(&mut counties, &realms);
    assert_eq!((counties[1].friendly_troops, counties[1].enemy_troops), (300, 0));
    assert_eq!((counties[2].friendly_troops, counties[2].enemy_troops), (0, 200));
}

/// An ally's army eats as a friendly. Realm `+0x81` read as an alliance
/// flag is `[I]` — this is the only rule that reads it that way.
#[test]
fn an_allys_army_counts_as_friendly() {
    let (mut counties, mut realms) = kingdom_bits();
    counties[1].owner = 2;
    realms[1].ally = 2;

    let mut units = Units::new();
    let mut u = army(1, 400);
    u.county = 1;
    units.spawn(u);

    units.recount_county_troops(&mut counties, &realms);
    assert_eq!(counties[1].friendly_troops, 400);
    assert_eq!(counties[1].enemy_troops, 0);
}

#[test]
fn a_garrison_is_excluded_from_the_recount_but_a_besieger_is_not() {
    let (mut counties, realms) = kingdom_bits();
    counties[1].owner = 1;

    let mut units = Units::new();
    let mut inside = army(1, 500);
    inside.county = 1;
    inside.garrison_county = 1;
    let mut outside = army(2, 300);
    outside.county = 1;
    outside.besieging_county = 1;
    units.spawn(inside);
    units.spawn(outside);

    units.recount_county_troops(&mut counties, &realms);
    assert_eq!(counties[1].friendly_troops, 0, "the garrison eats out of the castle");
    assert_eq!(counties[1].enemy_troops, 300, "the besieger forages the county");
}

#[test]
fn revolting_peasants_are_counted_and_merchants_are_not() {
    let (mut counties, realms) = kingdom_bits();
    counties[1].owner = 1;

    let mut units = Units::new();
    let mut mob = Unit::new(UnitKind::PeasantMob, 6, 5, 5);
    mob.county = 1;
    mob.men = 90;
    let mut trader = Unit::new(UnitKind::Merchant, 1, 6, 6);
    trader.county = 1;
    trader.men = 1000;
    units.spawn(mob);
    units.spawn(trader);

    units.recount_county_troops(&mut counties, &realms);
    assert_eq!(counties[1].enemy_troops, 90);
    assert_eq!(counties[1].friendly_troops, 0, "a merchant is not troops");
}

#[test]
fn a_recount_clears_the_counts_it_is_about_to_rebuild() {
    let (mut counties, realms) = kingdom_bits();
    counties[3].friendly_troops = 999;
    counties[3].enemy_troops = 999;
    Units::new().recount_county_troops(&mut counties, &realms);
    assert_eq!((counties[3].friendly_troops, counties[3].enemy_troops), (0, 0));
}

#[test]
fn the_wage_bill_covers_armies_only_and_charges_a_quarter_a_man() {
    let (_, mut realms) = kingdom_bits();
    realms[1].is_human = true;
    realms[1].in_play = true;

    let mut units = Units::new();
    units.spawn(army(1, 250));
    units.spawn(army(1, 252));
    let mut mob = Unit::new(UnitKind::PeasantMob, 1, 3, 3);
    mob.men = 1000;
    units.spawn(mob);
    units.spawn(army(2, 400));

    assert_eq!(wages_for_realm(T, &units, &realms, 1, 0), 62 + 63);
    let bill = refresh_wages(T, &mut units, &mut realms, 1, 0);
    assert_eq!(bill, 62 + 63);
    assert_eq!(realms[1].wages, bill);
    assert_eq!(units.get(1).unwrap().wages, 62);
    assert_eq!(units.get(2).unwrap().wages, 63);
    assert_eq!(units.get(3).unwrap().wages, 0, "a mob is not paid");
}

#[test]
fn a_garrisoned_army_is_still_paid() {
    let (_, mut realms) = kingdom_bits();
    realms[1].is_human = true;
    let mut units = Units::new();
    let mut inside = army(1, 400);
    inside.garrison_county = 3;
    units.spawn(inside);
    assert_eq!(refresh_wages(T, &mut units, &mut realms, 1, 0), 100);
}

#[test]
fn resetting_moves_touches_every_slot_regardless_of_type() {
    let mut units = Units::new();
    for kind in [UnitKind::Army, UnitKind::PeasantMob, UnitKind::Merchant, UnitKind::Transport] {
        let mut u = Unit::new(kind, 1, 0, 0);
        u.moves_used = 9;
        u.moving = true;
        units.spawn(u);
    }
    units.reset_moves();
    for (_, u) in units.iter() {
        assert_eq!(u.moves_used, 0, "{}", u.kind.name());
        assert!(!u.moving);
    }
}

#[test]
fn destroying_an_army_frees_the_slot_and_rebuilds_the_wage_bill() {
    let (_, mut realms) = kingdom_bits();
    realms[1].is_human = true;
    let mut names = ArmyNames::new();
    let mut units = Units::new();
    let a = units.spawn(army(1, 400)).unwrap();
    let b = units.spawn(army(1, 800)).unwrap();
    units.get_mut(a).unwrap().name_index = names.pick(1);
    refresh_wages(T, &mut units, &mut realms, 1, 0);
    assert_eq!(realms[1].wages, 100 + 200);

    let gone = destroy(T, &mut units, &mut realms, &mut names, a, 0).unwrap();
    assert_eq!(gone.men, 400);
    assert!(units.get(a).is_none());
    assert_eq!(realms[1].wages, 200, "the bill is recomputed from what is left");
    assert!(units.get(b).is_some());
}

#[test]
fn destroying_a_besieger_clears_the_garrisons_back_pointer() {
    let (_, mut realms) = kingdom_bits();
    let mut names = ArmyNames::new();
    let mut units = Units::new();
    let garrison = units.spawn(army(1, 200)).unwrap();
    let besieger = units.spawn(army(2, 400)).unwrap();
    units.get_mut(garrison).unwrap().garrison_county = 4;
    units.get_mut(besieger).unwrap().besieging_county = 4;
    units.get_mut(garrison).unwrap().besieged_by = besieger as u8;

    destroy(T, &mut units, &mut realms, &mut names, besieger, 0);
    assert_eq!(units.get(garrison).unwrap().besieged_by, 0);

    let besieger = units.spawn(army(2, 400)).unwrap();
    units.get_mut(besieger).unwrap().besieging_county = 4;
    destroy(T, &mut units, &mut realms, &mut names, garrison, 0);
    assert_eq!(units.get(besieger).unwrap().besieging_county, 0);
}

#[test]
fn realm_totals_count_armies_and_their_men() {
    let mut units = Units::new();
    units.spawn(army(1, 100));
    units.spawn(army(1, 250));
    units.spawn(army(2, 900));
    let mut mob = Unit::new(UnitKind::PeasantMob, 1, 0, 0);
    mob.men = 40;
    units.spawn(mob);
    assert_eq!(units.realm_totals(1), (2, 350));
    assert_eq!(units.realm_totals(2), (1, 900));
    assert_eq!(units.realm_totals(3), (0, 0));
}

#[test]
fn a_unit_is_found_by_the_tile_it_stands_on() {
    let mut units = Units::new();
    let a = units.spawn(army(1, 100)).unwrap();
    assert_eq!(units.at(10, 10), Some(a));
    assert_eq!(units.at(11, 10), None);
}

