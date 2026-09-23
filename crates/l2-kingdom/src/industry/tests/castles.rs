#![allow(unused_imports)]
use super::*;
use super::output::*;
use super::harvest::*;
use super::wages::*;
use super::super::*;
use crate::tables::{INDUSTRY_ORDER, JOB_BLACKSMITH, JOB_NAMES};

fn owned(pop: i32) -> County {
    let mut c = County::new();
    c.owner = 1;
    c.population = pop;
    c
}

/// **Ordering a castle staffs it** — `Castle_Order` (`0x00436D02`) calls
/// `Labour_ToggleIndustryShare(county, 3, 1)`, so the job leaves the order
/// with a share and the five-member group still sums to 100. It had a
/// share of 0 at any industry split before.
#[test]
fn ordering_a_castle_puts_builders_on_it() {
    use crate::tables::JOB_CASTLE_BUILDING;
    let industry = |c: &County| c.labour_share[JOB_CASTLE_BUILDING..].iter().sum::<i32>();

    let mut c = owned(2_000);
    c.industry_share = 100;
    let mut r = Realm::new();
    r.wood = 10_000;
    r.stone = 10_000;
    assert_eq!(c.labour_share[JOB_CASTLE_BUILDING], 0, "nobody builds before the order");

    assert!(order_castle(T, &mut c, &mut r, 3));
    assert!(c.labour_share[JOB_CASTLE_BUILDING] > 0, "the order staffs the castle job");
    assert_eq!(industry(&c), 100, "and the industry group still sums to 100");

    let mut out = Vec::new();
    c.castle_work_left = 0;
    assert!(build_tick(T, &mut c, &mut r, 0, &mut out).is_some());
    assert_eq!(c.labour_share[JOB_CASTLE_BUILDING], 0, "and the topped-out castle frees them");
    assert_eq!(industry(&c), 100);
}

#[test]
fn a_castle_can_be_ordered_with_nothing_in_the_store() {
    let mut c = owned(2_000);
    let mut r = Realm::new();
    r.wood = 199;
    r.stone = 0;
    assert!(order_castle(T, &mut c, &mut r, 3), "a keep is ordered on credit");
    assert_eq!(c.castle_type, 3, "and castleType moves at once");
    assert_eq!(c.castle_degraded, crate::siege::CASTLE_DEGRADED_BUILDING);
    assert_eq!((r.wood, r.stone), (0, 0), "everything there was is taken");
    assert_eq!((c.castle_wood_owed, c.castle_stone_owed), (1, 1_000), "the rest is owed");
    assert_eq!(castle_materials_percent(&c) < 100, true);
    assert_eq!(castle_labour_estimate(T, &c).1, 0, "and nobody may work on it");

    r.stone = 400;
    r.wood = 50;
    deliver_castle_materials(&mut c, &mut r);
    assert_eq!((c.castle_wood_owed, c.castle_stone_owed), (0, 600));
    assert_eq!(castle_labour_estimate(T, &c).1, 0, "still short of stone");
    r.stone = 600;
    deliver_castle_materials(&mut c, &mut r);
    assert_eq!(castle_labour_estimate(T, &c).1, 800, "the keep's whole workforce");
}

#[test]
fn a_castle_may_never_be_replaced_by_a_smaller_one() {
    let mut c = owned(2_000);
    c.castle_type = 3;
    assert_eq!(castle_refusal(T, &c, 3), Some(CastleRefusal::AlreadyBuilt));
    assert_eq!(castle_refusal(T, &c, 2), Some(CastleRefusal::Downgrade));
    assert_eq!(castle_refusal(T, &c, 1), Some(CastleRefusal::Downgrade));
    assert_eq!(castle_refusal(T, &c, 4), None);
    assert_eq!(castle_refusal(T, &c, 6), Some(CastleRefusal::NoSuchType));
    let mut r = Realm::new();
    assert!(!order_castle(T, &mut c, &mut r, 2));
    assert_eq!(c.castle_degraded, 0, "a refusal changes nothing");
}

#[test]
fn upgrading_pays_the_difference_and_refunds_what_the_new_castle_needs_less_of() {
    let mut c = owned(2_000);
    c.castle_type = 2;
    let mut r = Realm::new();
    r.wood = 0;
    r.stone = 10_000;
    assert!(order_castle(T, &mut c, &mut r, 3));
    assert_eq!(c.castle_building, 2, "the motte and bailey is what still stands");
    assert_eq!(c.castle_type, 3);
    assert_eq!(r.wood, 600, "600 wood came back out of the old castle");
    assert_eq!(r.stone, 10_000 - 920);
    assert_eq!((c.castle_wood_owed, c.castle_stone_owed), (0, 0));
}

#[test]
fn a_royal_castle_takes_several_seasons_of_workers() {
    let mut c = owned(2_000);
    let mut r = Realm::new();
    r.wood = 10_000;
    r.stone = 10_000;
    assert!(order_castle(T, &mut c, &mut r, 5));
    c.labour[T.job.castle_building] = 500;

    let mut out = Vec::new();
    for season in 1..5 {
        assert!(build_tick(T, &mut c, &mut r, 1, &mut out).is_none(), "season {season}");
        assert!(c.castle_percent < 100, "and it says so: {}", c.castle_percent);
    }
    let done = build_tick(T, &mut c, &mut r, 1, &mut out).expect("2500 at 500 a season");
    assert_eq!(c.castle_type, 5);
    assert_eq!(c.castle_degraded, 0);
    assert_eq!(c.castle_percent, 100);
    assert_eq!(done.free_archers, 300, "a royal castle's own garrison");
    assert!(!done.repaired);
    assert_eq!(out, vec![Message::CastleBuilt { county: 1, castle_type: 5 }]);
}

#[test]
fn a_castle_one_man_short_is_ninety_nine_percent_and_not_a_hundred() {
    let mut c = owned(2_000);
    let mut r = Realm::new();
    r.wood = 10_000;
    r.stone = 10_000;
    assert!(order_castle(T, &mut c, &mut r, 5));
    c.labour[T.job.castle_building] = 2_499;
    let mut out = Vec::new();
    assert!(build_tick(T, &mut c, &mut r, 1, &mut out).is_none());
    assert_eq!(c.castle_work_left, 1);
    assert_eq!(c.castle_percent, 99, "clamped, not rounded up");
    assert_eq!(c.castle_degraded, crate::siege::CASTLE_DEGRADED_BUILDING);
}

#[test]
fn the_free_garrison_is_what_the_upgrade_adds_and_a_small_county_gets_none() {
    let mut c = owned(2_000);
    c.castle_type = 3; // a keep, 150 archers
    c.castle_building = 2; // over a motte and bailey, also 150
    assert_eq!(free_garrison_archers(T, &c), 0, "an upgrade that adds nothing");
    c.castle_type = 4; // a stone castle, 200
    assert_eq!(free_garrison_archers(T, &c), 50);
    c.castle_building = 0;
    assert_eq!(free_garrison_archers(T, &c), 200, "a new castle brings all of them");
    c.population = 399;
    assert_eq!(free_garrison_archers(T, &c), 0, "more than half the county");
}

#[test]
fn a_county_not_building_anything_does_nothing() {
    let mut c = owned(2_000);
    let mut r = Realm::new();
    let mut out = Vec::new();
    for _ in 0..10 {
        assert!(build_tick(T, &mut c, &mut r, 1, &mut out).is_none());
    }
    assert!(out.is_empty());
    assert_eq!(c.castle_work_left, 0);
    assert_eq!(c.castle_percent, 0);
}

#[test]
fn a_bigger_castle_holds_more_men_and_comes_with_more_archers() {
    assert_eq!(garrison_cap(T, 0), 0);
    assert_eq!(free_archers(T, 0), 0);
    for t in 1..5u8 {
        assert!(garrison_cap(T, t + 1) >= garrison_cap(T, t), "castle {t}");
        assert!(free_archers(T, t + 1) >= free_archers(T, t), "castle {t}");
    }
    assert_eq!(garrison_cap(T, 5), 600);
    assert_eq!(free_archers(T, 5), 300);
}

#[test]
fn the_norman_keep_is_the_cheapest_castle_in_wood() {
    assert_eq!(crate::tables::CASTLE_STARTING_TYPE, 3);
    let (wood, _) = castle_cost(T, 3);
    for t in 1..=5u8 {
        assert!(castle_cost(T, t).0 >= wood, "castle {t}");
    }
}

/// `PctOf` is the original's `FUN_00404DC1`, including its zero case.
#[test]
fn pct_of_returns_zero_rather_than_dividing_by_zero() {
    assert_eq!(pct_of(50, 500), 10);
    assert_eq!(pct_of(1, 3), 33, "truncating");
    assert_eq!(pct_of(50, 0), 0);
    assert_eq!(pct_of(0, 100), 0);
    assert_eq!(pct_of(50, 49), 102, "and it can exceed 100");
}

