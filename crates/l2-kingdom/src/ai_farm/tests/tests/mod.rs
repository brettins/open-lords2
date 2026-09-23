
mod market;

use super::*;
use crate::field::terrain;
use crate::map::CampaignMap;

const T: &Tables = &Tables::DEFAULT;

fn env() -> FarmEnv {
    FarmEnv::new(Season::Winter, Season::Spring)
}

fn realms() -> Vec<Realm> {
    let mut realms = vec![Realm::new(); crate::realm::MAX_REALMS];
    for (i, r) in realms.iter_mut().enumerate().skip(1) {
        r.in_play = true;
        r.lord = ((i - 1) % crate::tables::AI_PERSONALITY_COUNT + 1) as u8;
    }
    realms
}

fn lay(style: FarmStyle, c: &mut County, map: &mut CampaignMap, e: &FarmEnv) {
    let mut counties = vec![County::new(), c.clone()];
    lay_out(T, style, &mut counties, 1, 1, map, &realms(), &mut NoMarket, e);
    *c = counties.remove(1);
}

fn county_with(n: usize) -> (County, CampaignMap) {
    let mut c = County::new();
    let mut map = CampaignMap::empty();
    for i in 0..n {
        let tile = crate::map::index(i as u8, 8);
        c.set_field_tile(i, Some(tile));
        map.terrain[tile] = terrain::FALLOW;
    }
    c.population = 500;
    crate::field::recount(&mut c, &map);
    (c, map)
}

/// **An AI realm that never plants is the original's rule, not a gap in
/// ours** — `Ai_FarmStyleGrazing` (`0x004A42E3`), whose first act is to
/// clear every grain field, in every season.
///
/// `Ai_ManageCountyFarms` (`0x0049DD01`) copies the lord's style byte into
/// county `+0x1FE` and dispatches on it,
/// `[1, 1, 0, 9]` — **two of the four graze**, so an AI realm behind either
/// sows nothing all game.
///
/// This pins **which lords it happens to**, which is the half that makes an
/// AI realm look broken: ablation, carried inside — lord 3's style is 0 and
/// the same county sows.
#[test]
fn two_of_the_four_lords_graze_so_their_realms_never_sow() {
    assert_eq!(crate::tables::AI_PERSONALITY_FARM_STYLE, [1, 1, 0, 9]);
    for lord in [1u8, 2] {
        assert_eq!(T.ai_farm_style(lord), Some(1), "lord {lord}");
        assert_eq!(FarmStyle::for_realm(1), Some(FarmStyle::RealmGrazing));
    }
    assert_eq!(T.ai_farm_style(3), Some(0));
    assert_eq!(T.ai_farm_style(4), Some(9));

    let sown = |style| {
        let (mut c, mut map) = county_with(8);
        let e = FarmEnv::new(Season::Winter, Season::Spring);
        lay(style, &mut c, &mut map, &e);
        counts(&c, &map).0
    };
    assert_eq!(sown(FarmStyle::RealmGrazing), 0, "lords 1 and 2");
    assert!(sown(FarmStyle::RealmArable) > 0, "lord 3 sows the same county");
}

fn counts(county: &County, map: &CampaignMap) -> (i32, i32, i32) {
    let mut probe = county.clone();
    crate::field::recount(&mut probe, map);
    (probe.fields_grain, probe.fields_cattle, probe.fields_fallow)
}

struct Ledger {
    asked: Vec<(i32, Good)>,
    grant: bool,
}

impl Market for Ledger {
    fn buy(
        &mut self,
        _id: usize,
        county: &mut County,
        _map: &mut CampaignMap,
        qty: i32,
        good: Good,
    ) -> bool {
        self.asked.push((qty, good));
        if !self.grant {
            return false;
        }
        match good {
            Good::Grain => county.grain += qty,
            Good::Cattle => county.herd += qty,
        }
        true
    }
}

fn ledger(grant: bool) -> Ledger {
    Ledger { asked: Vec::new(), grant }
}

#[test]
fn there_are_five_allocators_and_the_two_passes_reach_different_ones() {
    assert_eq!(FarmStyle::ALL.len(), 5);
    let mut addrs: Vec<u32> = FarmStyle::ALL.iter().map(|s| s.address()).collect();
    addrs.sort_unstable();
    addrs.dedup();
    assert_eq!(addrs.len(), 5, "five distinct allocators");

    assert_eq!(FarmStyle::for_realm(0), Some(FarmStyle::RealmArable));
    assert_eq!(FarmStyle::for_realm(1), Some(FarmStyle::RealmGrazing));
    assert_eq!(FarmStyle::for_realm(9), Some(FarmStyle::RealmMixed));
    assert_eq!(FarmStyle::for_realm(2), None, "no other byte dispatches");

    assert_eq!(FarmStyle::for_neutral(0), Some(FarmStyle::NeutralArable));
    assert_eq!(FarmStyle::for_neutral(1), Some(FarmStyle::NeutralGrazing));
    assert_eq!(
        FarmStyle::for_neutral(9),
        None,
        "an ex-Knight county is left alone by the neutral pass"
    );
}

/// `FUN_004A3C67` is the **neutral** pass's style 0 and no AI realm can
/// reach it. This is the briefing error, asserted.
#[test]
fn the_neutral_arable_style_is_unreachable_from_an_ai_realm() {
    assert_eq!(FarmStyle::NeutralArable.address(), 0x004A3C67);
    for byte in 0..=255u8 {
        assert_ne!(
            FarmStyle::for_realm(byte),
            Some(FarmStyle::NeutralArable),
            "byte {byte}"
        );
    }
}

#[test]
fn the_four_lords_between_them_use_every_realm_allocator() {
    let mut seen = Vec::new();
    for lord in 1..=crate::tables::AI_PERSONALITY_COUNT as u8 {
        let byte = T.ai_farm_style(lord).expect("lord {lord} has a record");
        let style = FarmStyle::for_realm(byte).expect("and a live allocator");
        seen.push(style);
    }
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(
        seen,
        vec![FarmStyle::RealmArable, FarmStyle::RealmGrazing, FarmStyle::RealmMixed]
    );
    assert_eq!(T.ai_farm_style(crate::realm::LORD_HUMAN), None);
}

#[test]
fn a_lot_that_arrives_stops_the_cascade_and_one_refused_does_not() {
    let (mut c, mut cmap) = county_with(4);
    c.grain = 0;
    let mut m = ledger(true);
    run_buys(FarmStyle::NeutralArable, 1, &mut c, &mut cmap, &mut m);
    assert_eq!(m.asked, vec![(400, Good::Grain)], "the first lot satisfied the floor");
    assert_eq!(c.grain, 400);

    let (mut c, mut cmap) = county_with(4);
    c.grain = 0;
    let mut m = ledger(false);
    run_buys(FarmStyle::NeutralArable, 1, &mut c, &mut cmap, &mut m);
    assert_eq!(
        m.asked,
        vec![(400, Good::Grain), (200, Good::Grain), (100, Good::Grain), (50, Good::Grain)],
        "every lot is offered when the purse refuses"
    );
}

fn neutral_with_purse(purse: i32) -> (County, CampaignMap, crate::unit::Units) {
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

fn stall<'a>(t: &'a Tables, c: &County, units: &crate::unit::Units) -> CountyStall<'a> {
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

