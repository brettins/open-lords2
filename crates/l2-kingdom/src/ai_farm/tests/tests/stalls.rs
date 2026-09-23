#![allow(unused_imports)]
use super::*;
use super::allocators::*;
use super::super::*;
use crate::field::terrain;
use crate::map::CampaignMap;

pub(crate) fn neutral_with_purse(purse: i32) -> (County, CampaignMap, crate::unit::Units) {
    let (mut c, map) = county_with(12);
    c.owner = 0;
    c.farm_style = 1;
    c.purse = purse;
    c.grain = 57;
    c.herd = 135;
    c.merchant_count = 1;
    c.merchant_unit = 4;
    let mut units = crate::unit::Units::new();
    let mut m = crate::unit::Unit::new(crate::unit::UnitKind::Merchant, 6, 8, 8);
    m.morale = 100;
    m.county = 1;
    units.put(4, m);
    (c, map, units)
}

pub(crate) fn stall<'a>(t: &'a Tables, c: &County, units: &crate::unit::Units) -> CountyStall<'a> {
    stall_with(t, c, units, Box::leak(realms().into_boxed_slice()))
}

fn stall_with<'a>(
    t: &'a Tables,
    c: &County,
    units: &crate::unit::Units,
    realms: &'a mut [Realm],
) -> CountyStall<'a> {
    let mut counties = vec![County::new(); crate::county::MAX_COUNTIES];
    counties[1] = c.clone();
    let counties: [County; crate::county::MAX_COUNTIES] =
        counties.try_into().expect("MAX_COUNTIES entries");
    CountyStall::new(
        t,
        &counties,
        units,
        realms,
        Season::Spring,
        true,
        crate::ration::Sowing::new(Season::Spring, false),
    )
}

/// `Ai_TradeForCounty` (`0x0049E39B`) — the surplus sale, then the weapon
/// order, on one county of the Knight's.
///
/// *Ablation*: return `trade_for_county` to the trait's empty default and
/// every assertion below goes red at once.
#[test]
fn a_lord_sells_his_surplus_and_buys_the_countys_weapon() {
    let (mut c, mut map, units) = neutral_with_purse(0);
    c.owner = 1; // realm 1 is lord 1, the Knight: reserve 250, floor 1,000, 100 weapons
    c.weapon_type = 1; // maces, `Ai_TradeForCounty`'s good 11
    let mut rs = realms();
    rs[1].gold = 2_015;
    rs[1].wood = 300;
    rs[1].iron = 100;
    rs[1].stone = 250;
    let mut m = stall_with(T, &c, &units, &mut rs);
    m.trade_for_county(1, &mut c, &mut map);
    drop(m);
    assert_eq!((rs[1].wood, rs[1].iron, rs[1].stone), (250, 100, 250));
    assert_eq!(rs[1].weapons[1], 100, "good 11 lands in weapons[1]");
    assert_eq!(rs[1].gold, 2_015 + 50 * 1 - 100 * 20);
    assert_eq!((rs[1].trade_received_a, rs[1].trade_received_b), (50, 50));
    assert_eq!((rs[1].trade_spent_a, rs[1].trade_spent_b), (2_000, 2_000));
}

/// The gold floor is strict
/// `Ai_BuyGoodDownTo` (`0x004A4C41`)'s `qty = qty / 2` loop.
#[test]
fn the_weapon_order_halves_until_the_treasury_covers_it() {
    for (gold, bought) in [(1_000, 0), (1_001, 50), (2_000, 100)] {
        let (mut c, mut map, units) = neutral_with_purse(0);
        c.owner = 1;
        c.weapon_type = 1;
        let mut rs = realms();
        rs[1].gold = gold;
        let mut m = stall_with(T, &c, &units, &mut rs);
        m.trade_for_county(1, &mut c, &mut map);
        drop(m);
        assert_eq!(rs[1].weapons[1], bought, "treasury {gold}");
        assert_eq!(rs[1].gold, gold - bought * 20, "treasury {gold}");
    }
}

/// Two refusals in front of all of it: **no stall** (county `+0x1A4`) and a
/// **bankrupt realm** (`bankruptStage != 0`).
#[test]
fn a_lord_with_no_stall_or_no_credit_trades_nothing() {
    for case in ["no stall", "bankrupt"] {
        let (mut c, mut map, units) = neutral_with_purse(0);
        c.owner = 1;
        c.weapon_type = 1;
        let mut rs = realms();
        rs[1].gold = 5_000;
        rs[1].wood = 5_000;
        if case == "no stall" {
            c.merchant_count = 0;
        } else {
            rs[1].bankrupt_stage = 1;
        }
        let mut m = stall_with(T, &c, &units, &mut rs);
        m.trade_for_county(1, &mut c, &mut map);
        drop(m);
        assert_eq!((rs[1].gold, rs[1].wood, rs[1].weapons[1]), (5_000, 5_000, 0), "{case}");
    }
}

/// *Ablation*: put the owned arm back to `county.owner != 0 → false` and
/// the first case goes red (nothing bought); test the purse for an owned
/// county instead of the gold
#[test]
fn an_ai_lords_county_pays_for_its_grain_out_of_the_treasury() {
    for (gold, bought) in [(1_600, 400), (1_599, 0)] {
        let (mut c, mut map, units) = neutral_with_purse(10_000);
        c.owner = 2;
        c.grain = 0;
        c.herd = 50; // above the grazing style's cattle floor of 41
        let mut rs = realms();
        rs[2].gold = gold;
        let mut m = stall_with(T, &c, &units, &mut rs);
        run_buys(FarmStyle::RealmGrazing, 1, &mut c, &mut map, &mut m);
        drop(m);
        assert_eq!(c.grain, bought, "treasury {gold}");
        assert_eq!(c.purse, 10_000, "treasury {gold}: an owned county's purse is not spent");
        let bill = bought * 4;
        assert_eq!(rs[2].gold, gold - bill, "treasury {gold}");
        assert_eq!((rs[2].trade_spent_a, rs[2].trade_spent_b), (bill, bill), "treasury {gold}");
        for (i, r) in rs.iter().enumerate().filter(|(i, _)| *i != 2) {
            assert_eq!(r.gold, 0, "realm {i} is not the owner and pays nothing");
        }
    }
}


