#![allow(unused_imports)]
use super::*;
use super::helpers::*;
use super::reproduction::*;
use super::food_and_ration::*;
use super::simulation::*;
use l2_formats::save::Save;
use l2_kingdom::county::County;
use l2_kingdom::phase::SEASON_PIPELINE;
use l2_kingdom::tables::{health_band, Season, Tables, Weather};
use l2_scenario::{Scenario, STARTING_HEALTH_METER};

#[test]
fn every_countys_nine_labour_records_sum_to_its_population() {
    let s = england!();
    let k = s.kingdom(SEED);
    for id in s.county_ids() {
        let c = &k.counties[id];
        let assigned: i32 = c.labour.iter().sum();
        assert_eq!(assigned, c.population, "county {id}");
        assert!(c.labour[Tables::DEFAULT.job.cattle_farming] > 0, "county {id} farms cattle");
    }
}

/// `+0x254` is written by `Herd_SeasonTick` at the top of the season, so what
/// is in the file is the herd as the *first* season found it — and it is the
/// `herd` column of the new-game table at `0x004DC0D0 + difficulty * 0x14`,
/// whose row 1 is `{grain 0, herd 95, population 417, health 65, health 65}`.
#[test]
fn every_county_opened_the_game_on_the_same_herd_and_the_same_people() {
    let save = l2_testkit::england!();
    let s = Scenario::from_save(&save).expect("the England turn-one fixture must import");
    for id in s.county_ids() {
        assert_eq!(county_i32(&save, id, 0x254), 95, "county {id} herd at the top of season 1");
        let stored = s.counties[id].as_ref().expect("a county");
        assert_eq!(stored.population_last, 417, "county {id} popLast");
    }
    assert_eq!(STARTING_HEALTH_METER, 65, "and the health meter is the same table's column 3");
}

#[test]
fn migration_runs_on_the_real_adjacency_and_moves_nobody() {
    let s = england!();
    let file = s.kingdom(SEED);
    let k = kingdom_after_the_first_season(&s);

    for id in s.county_ids() {
        assert!(k.counties[id].neighbour_count > 0, "county {id} must have neighbours");
        assert_eq!(k.counties[id].emigrants, 0, "county {id}");
        assert_eq!(k.counties[id].immigrants, 0, "county {id}");
        assert_eq!(k.counties[id].emigrants, file.counties[id].emigrants, "county {id}");
        assert_eq!(k.counties[id].immigrants, file.counties[id].immigrants, "county {id}");
    }
}

/// `FUN_0044F6E7` is 2,147 bytes and it is the writer of all nine job records:
///
/// it splits the population into a farm half and an industry half by county
/// `+0x08`, gives each job `Pct(half, share[job])` people or as many as its
/// useful ceiling allows, walks the leftovers round the jobs that still have
/// room in an uneven rota, and drops whatever nobody could take into *Idle
/// townsfolk*.
#[test]
fn the_labour_allocator_rebuilds_every_countys_own_worker_counts() {
    let s = england!();
    let k = s.kingdom(SEED);
    let mut checked = 0;
    for id in s.county_ids() {
        let mut c = k.counties[id].clone();
        let stored = c.labour;
        let idle = l2_kingdom::labour::allocate(&mut c);
        assert_eq!(
            c.labour, stored,
            "county {id}: pop {} at {}% industry, shares {:?}, ceilings {:?}",
            c.population,
            c.industry_share,
            c.labour_share,
            l2_kingdom::labour::ceilings(&c)
        );
        assert_eq!(c.labour.iter().sum::<i32>(), c.population, "county {id}");
        assert_eq!(idle > 0, c.owner == 0, "county {id} owner {}", c.owner);
        checked += 1;
    }
    assert_eq!(checked, 14);
}

#[test]
fn recomputing_the_shares_from_the_shipped_counties_keeps_both_halves_at_a_hundred() {
    let s = england!();
    let k = s.kingdom(SEED);
    for id in s.county_ids() {
        let mut c = k.counties[id].clone();
        l2_kingdom::labour::recompute_shares(&mut c);
        assert_eq!(c.labour_share[0..3].iter().sum::<i32>(), 100, "county {id} farm");
        assert_eq!(c.labour_share[3..8].iter().sum::<i32>(), 100, "county {id} industry");
        l2_kingdom::labour::recompute_industry_share(&mut c);
        assert!((0..=100).contains(&c.industry_share), "county {id}: {}", c.industry_share);
        l2_kingdom::labour::allocate(&mut c);
        assert_eq!(c.labour.iter().sum::<i32>(), c.population, "county {id}");
    }
}

