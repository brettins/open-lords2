#![allow(unused_imports)]
use super::*;
use super::harvest::*;
use super::wages::*;
use super::castles::*;
use super::super::*;
use crate::tables::{INDUSTRY_ORDER, JOB_BLACKSMITH, JOB_NAMES};

#[test]
fn each_commodity_draws_on_the_job_its_name_says() {
    assert_eq!(JOB_NAMES[Commodity::Wood.job()], "Wood cutting");
    assert_eq!(JOB_NAMES[Commodity::Iron.job()], "Iron mining");
    assert_eq!(JOB_NAMES[Commodity::Stone.job()], "Stone quarrying");
    assert_eq!(JOB_NAMES[Commodity::Weapons.job()], "Blacksmith");
    assert_eq!(JOB_NAMES[crate::tables::JOB_GRAIN_FARMING], "Grain farming");
    assert_eq!(JOB_NAMES.len(), crate::tables::JOB_COUNT);
    assert_eq!(crate::tables::JOB_COUNT, 9, "nine records, not ten");
}

#[test]
fn no_two_commodities_share_a_job() {
    let mut jobs: Vec<usize> = Commodity::ALL.iter().map(|c| c.job()).collect();
    jobs.sort_unstable();
    jobs.dedup();
    assert_eq!(jobs.len(), 4);
    assert_eq!(jobs, vec![4, 5, 6, 7], "iron, stone, wood, blacksmith");
}

#[test]
fn the_blacksmith_runs_before_the_mines() {
    assert_eq!(INDUSTRY_ORDER[0], Commodity::Weapons);
    assert_eq!(
        INDUSTRY_ORDER,
        [Commodity::Weapons, Commodity::Iron, Commodity::Stone, Commodity::Wood]
    );
    let mut seen: Vec<Commodity> = INDUSTRY_ORDER.to_vec();
    seen.sort_unstable();
    assert_eq!(seen, Commodity::ALL.to_vec(), "all four, exactly once");
}


#[test]
fn a_basic_game_runs_every_industry_at_eighty_percent() {
    for base in [15, 20] {
        for workers in [0, 1, 30, 10_000] {
            assert_eq!(efficiency_ramp(T, 0, workers, 50, base, false), 80);
            assert_eq!(efficiency_ramp(T, 100, workers, 50, base, false), 80);
        }
    }
}

#[test]
fn an_iron_mine_ramps_fifteen_points_a_season_up_to_a_hundred() {
    let mut e = 0;
    let mut seen = Vec::new();
    for _ in 0..9 {
        e = efficiency_ramp(T, e, 30, 1_000, 15, true);
        seen.push(e);
    }
    assert_eq!(seen, vec![15, 30, 45, 60, 75, 90, 100, 100, 100]);
}

#[test]
fn the_ramp_never_falls_below_the_base() {
    assert_eq!(efficiency_ramp(T, -50, 10, 1_000, 20, true), 20);
    assert_eq!(efficiency_ramp(T, 0, 10, 1_000, 20, true), 20);
}

#[test]
fn an_industry_with_no_workers_has_no_efficiency() {
    assert_eq!(efficiency_ramp(T, 90, 0, 100, 15, true), 0);
}

#[test]
fn working_more_serfs_than_the_capacity_slows_the_improvement() {
    assert_eq!(efficiency_ramp(T, 0, 100, 100, 15, true), 15);
    assert_eq!(efficiency_ramp(T, 0, 200, 100, 15, true), 15, "but never below the base");
    assert_eq!(efficiency_ramp(T, 50, 100, 100, 15, true), 65);
    assert_eq!(efficiency_ramp(T, 50, 200, 100, 15, true), 57, "50 + Pct(15, 50)");
    assert_eq!(efficiency_ramp(T, 50, 400, 100, 15, true), 53, "50 + Pct(15, 25)");
}

#[test]
fn an_industry_with_no_capacity_is_pinned_at_its_base() {
    let mut e = 15;
    for _ in 0..20 {
        e = efficiency_ramp(T, e, 30, 0, 15, true);
    }
    assert_eq!(e, 15);
}


#[test]
fn a_mine_is_limited_only_by_being_switched_on() {
    let realm = Realm::new();
    let mut c = County::new();
    for cm in [Commodity::Wood, Commodity::Iron, Commodity::Stone] {
        assert_eq!(resource_limit(T, &c, cm, &realm, WeaponShare::UNSHARED), 999);
    }

    c.industry[Commodity::Iron.index()].has_resource = false;
    assert_eq!(resource_limit(T, &c, Commodity::Iron, &realm, WeaponShare::UNSHARED), 0, "no ore in the ground");

    c.industry[Commodity::Wood.index()].enabled = false;
    assert_eq!(resource_limit(T, &c, Commodity::Wood, &realm, WeaponShare::UNSHARED), 0, "switched off");

    c.industry[Commodity::Stone.index()].disabled_seasons = 2;
    assert_eq!(resource_limit(T, &c, Commodity::Stone, &realm, WeaponShare::UNSHARED), 0, "counting down");
}

#[test]
fn nine_hundred_and_ninety_nine_is_a_real_cap() {
    let realm = Realm::new();
    let mut c = advanced_county(0);
    c.labour[Commodity::Wood.job()] = 100_000;
    assert_eq!(output(T, &c, Commodity::Wood, &realm, WeaponShare::UNSHARED), 999);
}

#[test]
fn the_blacksmith_is_limited_by_the_realms_share_of_the_stockpile() {
    let mut c = County::new();
    c.weapon_type = 0; // crossbow: 6 wood, 10 iron
    let mut realm = Realm::new();
    realm.wood = 600;
    realm.iron = 300;
    let unshared = WeaponShare::UNSHARED;
    assert_eq!(
        resource_limit(T, &c, Commodity::Weapons, &realm, unshared),
        300,
        "the iron is scarcer"
    );
    assert_eq!(
        resource_limit(T, &c, Commodity::Weapons, &realm, WeaponShare { wood: 18, iron: 30 }),
        10,
        "three smithies forging crossbows split the 300 iron three ways, at 10 a crossbow"
    );

    c.weapon_type = 4;
    realm.iron = 0;
    assert_eq!(resource_limit(T, &c, Commodity::Weapons, &realm, unshared), 600);
}

/// `FUN_0044F15B`,
#[test]
fn the_weapon_share_sums_the_costs_of_every_staffed_smithy() {
    let mut counties = vec![County::new(); 4];
    for (id, weapon) in [(1usize, 0usize), (2, 1), (3, 0)] {
        counties[id].owner = 1;
        counties[id].weapon_type = weapon;
        counties[id].labour[T.commodity[Commodity::Weapons.index()].job] = 40;
    }
    assert_eq!(weapon_shares(T, &counties, 3, 1), WeaponShare { wood: 16, iron: 24 });

    counties[2].labour[T.commodity[Commodity::Weapons.index()].job] = 0;
    assert_eq!(weapon_shares(T, &counties, 3, 1), WeaponShare { wood: 12, iron: 20 });

    counties[3].industry[Commodity::Weapons.index()].enabled = false;
    counties[1].owner = 2;
    assert_eq!(weapon_shares(T, &counties, 3, 1), WeaponShare { wood: 1, iron: 1 });
}

#[test]
fn a_lone_smithy_is_limited_to_what_the_stockpile_can_buy() {
    let mut c = County::new();
    c.owner = 1;
    c.weapon_type = 0; // 6 wood, 10 iron
    c.labour[T.commodity[Commodity::Weapons.index()].job] = 10;
    let mut realm = Realm::new();
    realm.wood = 600;
    realm.iron = 300;
    let counties = vec![County::new(), c.clone()];
    let share = weapon_shares(T, &counties, 1, 1);
    assert_eq!(share, WeaponShare { wood: 6, iron: 10 });
    assert_eq!(
        resource_limit(T, &c, Commodity::Weapons, &realm, share),
        30,
        "300 iron at 10 a crossbow"
    );
}


