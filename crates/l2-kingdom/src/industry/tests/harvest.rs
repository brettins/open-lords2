#![allow(unused_imports)]
use super::*;
use super::output::*;
use super::wages::*;
use super::castles::*;
use super::super::*;
use crate::tables::{INDUSTRY_ORDER, JOB_BLACKSMITH, JOB_NAMES};

#[test]
fn thirty_serfs_at_fifteen_percent_produce_four() {
    let mut c = worker_county(Commodity::Iron.job(), 30);
    c.industry[Commodity::Iron.index()].efficiency = 15;
    let realm = Realm::new();
    assert_eq!(output(T, &c, Commodity::Iron, &realm, WeaponShare::UNSHARED), 4, "Pct(30 / 1, 15)");
}

#[test]
fn iron_and_wood_harvest_at_twice_the_quantity_of_stone() {
    let mut c = advanced_county(200);
    c.industry[Commodity::Wood.index()].efficiency = 15; // level the bases
    let realm = Realm::new();
    let iron = output(T, &c, Commodity::Iron, &realm, WeaponShare::UNSHARED);
    let stone = output(T, &c, Commodity::Stone, &realm, WeaponShare::UNSHARED);
    let wood = output(T, &c, Commodity::Wood, &realm, WeaponShare::UNSHARED);
    assert_eq!(iron, 30);
    assert_eq!(stone, 15);
    assert_eq!(iron, stone * 2);
    assert_eq!(wood, iron);
}

#[test]
fn wood_is_the_only_commodity_with_a_twenty_percent_base() {
    assert_eq!(Commodity::Wood.base_efficiency(), 20);
    for c in Commodity::ALL {
        if c != Commodity::Wood {
            assert_eq!(c.base_efficiency(), 15, "{c:?}");
        }
    }
}

#[test]
fn each_commodity_is_credited_to_its_own_stockpile() {
    let mut c = advanced_county(400);
    let mut r = Realm::new();
    for cm in [Commodity::Wood, Commodity::Iron, Commodity::Stone] {
        produce(T, &mut c, &mut r, cm, true);
    }
    assert_eq!(r.wood, 160, "Pct(400, 20 + 20) after one ramp step");
    assert_eq!(r.iron, 120, "Pct(400, 15 + 15)");
    assert_eq!(r.stone, 60, "Pct(400 / 2, 30)");
    assert_eq!(r.weapons, [0; WEAPON_TYPE_COUNT]);
}

#[test]
fn a_basic_game_out_produces_a_new_advanced_one() {
    let mut basic = advanced_county(100);
    let mut advanced = advanced_county(100);
    let mut r1 = Realm::new();
    let mut r2 = Realm::new();
    produce(T, &mut basic, &mut r1, Commodity::Iron, false);
    produce(T, &mut advanced, &mut r2, Commodity::Iron, true);
    assert_eq!(r1.iron, 80, "flat 80%");
    assert_eq!(r2.iron, 30, "15% base, ramped once");
}

#[test]
fn the_blacksmith_is_capped_by_the_stock_the_weapon_costs() {
    let mut c = worker_county(JOB_BLACKSMITH, 400);
    c.industry[Commodity::Weapons.index()].efficiency = 15;
    c.weapon_type = 0; // crossbow: 6 wood, 10 iron
    let mut r = Realm::new();
    r.wood = 1000;
    r.iron = 40;
    assert_eq!(
        output(T, &c, Commodity::Weapons, &r, WeaponShare::UNSHARED),
        15,
        "the workers allow 15"
    );
    assert_eq!(output(T, &c, Commodity::Weapons, &r, WeaponShare::single_smith(T, 0)), 4);

    produce(T, &mut c, &mut r, Commodity::Weapons, true);
    assert_eq!(r.weapons[0], 4);
    assert_eq!(r.iron, 0);
    assert_eq!(r.wood, 1000 - 4 * 6);
}

#[test]
fn bows_need_no_iron() {
    let mut c = worker_county(JOB_BLACKSMITH, 400);
    c.industry[Commodity::Weapons.index()].efficiency = 15;
    c.weapon_type = 4; // bow: 13 wood, 0 iron
    let mut r = Realm::new();
    r.wood = 1000;
    r.iron = 0;
    produce(T, &mut c, &mut r, Commodity::Weapons, true);
    assert_eq!(r.weapons[4], 15);
    assert_eq!(r.wood, 1000 - 15 * 13);
}

#[test]
fn a_realm_with_nothing_in_stock_makes_no_weapons_and_owes_nothing() {
    let mut c = worker_county(JOB_BLACKSMITH, 4000);
    let mut r = Realm::new();
    produce(T, &mut c, &mut r, Commodity::Weapons, true);
    assert_eq!(r.weapons[0], 0);
    assert_eq!(r.wood, 0);
    assert_eq!(r.iron, 0, "and no negative stockpile");
}

#[test]
fn a_disabled_industry_counts_down_and_then_returns() {
    let mut c = advanced_county(400);
    c.industry[Commodity::Iron.index()].disabled_seasons = 2;
    c.industry[Commodity::Iron.index()].total = 500;
    c.industry[Commodity::Iron.index()].enabled = false;
    let mut r = Realm::new();

    produce(T, &mut c, &mut r, Commodity::Iron, true);
    assert_eq!(r.iron, 0);
    assert_eq!(c.industry[Commodity::Iron.index()].total, 0, "the total is forgotten");
    assert_eq!(c.industry[Commodity::Iron.index()].disabled_seasons, 1);
    assert!(!c.industry[Commodity::Iron.index()].enabled);

    produce(T, &mut c, &mut r, Commodity::Iron, true);
    assert_eq!(c.industry[Commodity::Iron.index()].disabled_seasons, 0);
    assert!(c.industry[Commodity::Iron.index()].enabled, "reinstated");

    produce(T, &mut c, &mut r, Commodity::Iron, true);
    assert!(r.iron > 0, "and producing again");
}

#[test]
fn the_running_total_accumulates_across_seasons() {
    let mut c = advanced_county(100);
    c.industry[Commodity::Stone.index()].efficiency = 15;
    let mut r = Realm::new();
    let mut totals = Vec::new();
    for _ in 0..3 {
        produce(T, &mut c, &mut r, Commodity::Stone, true);
        totals.push(c.industry[Commodity::Stone.index()].total);
    }
    assert_eq!(totals, vec![15, 37, 67]);
    assert_eq!(r.stone, 67, "the realm has what the county totalled");
}


