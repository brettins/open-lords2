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

/// **The labour import checks itself.** Nine job records a county, and every
/// county's nine sum to its population *exactly* — 218 + 217 = 435 in county 1,
/// 323 + 133 = 456 in county 2, and so on for all fourteen.
///
/// That is the evidence the stride is `0x0C` and the worker count is the first
/// word of the record, and it is the kind of check `docs/method.md` §2 rates
/// above any amount of reading: a property of the data, not of our code. A
/// stride of 4, 8 or 16, or a count at word 1 or 2, would not land on the
/// population once, let alone fourteen times.
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

/// **Every county opened the game on the same 95 head**, which is the number
/// `docs/kingdom.md`'s reproduction could not see while the herd rule was
/// missing.
///
/// `+0x254` is written by `Herd_SeasonTick` at the top of the season, so what
/// is in the file is the herd as the *first* season found it — and it is the
/// `herd` column of the new-game table at `0x004DC0D0 + difficulty * 0x14`,
/// whose row 1 is `{grain 0, herd 95, population 417, health 65, health 65}`.
/// Every one of those four reappears in the save: `popLast` is 417 in all
/// fourteen counties, and 65 is [`STARTING_HEALTH_METER`], which
/// `crates/l2-scenario` had marked as the one number in the reproduction taken
/// from prior art. It is in the binary.
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

/// Migration runs on the file's real adjacency — which is sparse, not the
/// fully-connected map the old test invented — and moves **nobody**, which is
/// what the save's arithmetic requires: `435 = 417+63-45` and `456 = 417+84-45`
/// leave no room for a migrant.
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

/// **The labour allocator, reproduced.**
///
/// `FUN_0044F6E7` is 2,147 bytes and it is the writer of all nine job records:
/// it splits the population into a farm half and an industry half by county
/// `+0x08`, gives each job `Pct(half, share[job])` people or as many as its
/// useful ceiling allows, walks the leftovers round the jobs that still have
/// room in an uneven rota, and drops whatever nobody could take into *Idle
/// townsfolk*.
///
/// The shipped save is the state immediately after that pass ran, so its own
/// worker counts are the answer. Rebuilding them from the population, the
/// industry share, the eight percentages and the eight ceilings —
/// **and getting all fourteen counties exactly right, including the 133 idle
/// in every unowned one and the nought idle in every owned one** — is not
/// something a wrong rota or a wrong quota order could do.
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
        // An owned county's wood ceiling is unbounded and takes everyone left;
        // an unowned county's is zero and they stand idle. It is the same pass
        // producing both.
        assert_eq!(idle > 0, c.owner == 0, "county {id} owner {}", c.owner);
        checked += 1;
    }
    assert_eq!(checked, 14);
}

/// And the two functions that write the allocator's own inputs back from what
/// it did: each half of the eight percentages comes out summing to exactly 100,
/// which is the invariant both of the binary's default setters satisfy.
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
        // Reallocating on the recomputed inputs still spends everybody.
        l2_kingdom::labour::allocate(&mut c);
        assert_eq!(c.labour.iter().sum::<i32>(), c.population, "county {id}");
    }
}

