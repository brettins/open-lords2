#![allow(unused_imports)]
use super::*;
use super::allocation::*;
use crate::county::County;
use crate::math::pct;
use crate::tables::{
    Commodity, JOB_CASTLE_BUILDING, JOB_CATTLE_FARMING, JOB_COUNT, JOB_FIELD_RECLAMATION,
    JOB_GRAIN_FARMING, JOB_IDLE_TOWNSFOLK, JOB_IRON_MINING, JOB_STONE_QUARRYING, JOB_BLACKSMITH,
    JOB_WOOD_CUTTING,
};

/// `Labour_ToggleShare` (`FUN_00450639`, `0x00450639`, 677 bytes) — bring one
/// farm job into the split, or take it out.
///
/// It is not: it writes county `+0x130 + job*4`, the eight job percentages
/// (`docs/kingdom.md` §14), and its only caller is `Field_SetType`, which uses
/// it to give field reclamation a share of the farm the moment the county has
/// a field under reclamation, and to take it away again when it has none. The
/// two readings were both half-right — the *call* comes from painting a field,
/// the *effect* is on labour — and only the caller separates them.
///
/// **`[D]`**, and two details of it are worth stating because they look like
/// transcription errors and are not.
pub fn toggle_share(county: &mut County, job: usize, on: bool, divisor: i32) {
    toggle(county, job, on, divisor, &FARM_GROUP, FARM_SEED);
}

/// `FUN_004502CA` (`0x004502CA`, 879 bytes) — the same thing for the five
/// industry jobs
pub fn toggle_industry_share(county: &mut County, job: usize, on: bool) {
    toggle(county, job, on, 1, &INDUSTRY_GROUP, INDUSTRY_SEED);
}

fn toggle(
    county: &mut County,
    job: usize,
    on: bool,
    divisor: i32,
    group: &[usize],
    seed: usize,
) {
    if (county.labour_share[job] == 0) != on {
        return;
    }
    if on {
        let n = group.iter().filter(|&&j| county.labour_share[j] != 0).count();
        let give = SHARE_TABLE[n.min(SHARE_TABLE.len() - 1)] / divisor.max(1);
        let scale = 100 - give;
        for &j in group {
            county.labour_share[j] = crate::math::pct(scale, county.labour_share[j]);
        }
        county.labour_share[job] = give;
    } else {
        let scale = 100 - county.labour_share[job];
        county.labour_share[job] = 0;
        for &j in group {
            county.labour_share[j] = crate::math::pct_of(county.labour_share[j], scale);
        }
    }

    let short: i32 = 100 - group.iter().map(|&j| county.labour_share[j]).sum::<i32>();
    let mut best = group[seed];
    let mut best_share = county.labour_share[group[seed]];
    for &j in group {
        if best_share < county.labour_share[j] {
            best = j;
            best_share = county.labour_share[j];
        }
    }
    county.labour_share[best] += short;
}

/// `FUN_0044FF4A` — rewrite [`County::industry_share`] from what
/// assigned.
pub fn recompute_industry_share(county: &mut County) {
    let pop = county.population;
    let farm = county.labour[JOB_GRAIN_FARMING]
        + county.labour[JOB_CATTLE_FARMING]
        + county.labour[JOB_FIELD_RECLAMATION];
    let idle = county.labour[JOB_IDLE_TOWNSFOLK];
    county.industry_share = crate::math::pct_of(pop - farm - idle + idle / 2, pop);
}

/// `FUN_00450000` — rewrite the eight percentages from the worker counts.
pub fn recompute_shares(county: &mut County) {
    renormalise(county, &[JOB_GRAIN_FARMING, JOB_CATTLE_FARMING, JOB_FIELD_RECLAMATION], 1);
    renormalise(
        county,
        &[
            JOB_CASTLE_BUILDING,
            JOB_IRON_MINING,
            JOB_STONE_QUARRYING,
            JOB_WOOD_CUTTING,
            JOB_BLACKSMITH,
        ],
        JOB_WOOD_CUTTING,
    );
}

/// The remainder goes to `fallback` unless some job in the group has a bigger
/// share than the first one measured — `FUN_00450000` seeds its search with
/// cattle's share for the farm group and wood's for the industry group, which
/// is why those two are the defaults.
fn renormalise(county: &mut County, group: &[usize], fallback: usize) {
    let total: i32 = group.iter().map(|&j| county.labour[j]).sum();
    for &job in group {
        county.labour_share[job] = crate::math::pct_of(county.labour[job], total);
    }
    let mut best = fallback;
    let mut best_share = county.labour_share[fallback];
    for &job in group {
        if best_share < county.labour_share[job] {
            best = job;
            best_share = county.labour_share[job];
        }
    }
    let sum: i32 = group.iter().map(|&j| county.labour_share[j]).sum();
    county.labour_share[best] += 100 - sum;
}

