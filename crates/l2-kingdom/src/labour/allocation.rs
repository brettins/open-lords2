#![allow(unused_imports)]
use super::*;
use super::shares::*;
use crate::county::County;
use crate::math::pct;
use crate::tables::{
    Commodity, JOB_CASTLE_BUILDING, JOB_CATTLE_FARMING, JOB_COUNT, JOB_FIELD_RECLAMATION,
    JOB_GRAIN_FARMING, JOB_IDLE_TOWNSFOLK, JOB_IRON_MINING, JOB_STONE_QUARRYING, JOB_BLACKSMITH,
    JOB_WOOD_CUTTING,
};

/// The gates are `FUN_0044F6E7`'s own, and they are the *enable* byte
/// (`+0x297 + c*0x18`) — an
/// industry switched off allocates nobody even where the ore is.
///
/// **One gate is deliberately not applied**: castle building is gated on
/// `+0x1C3` *and* on `+0x1B0`, and `+0x1B0` is a switch the player throws by
/// clicking the castle on the map. The field exists now
/// ([`County::castle_switch`], and `Industry_ToggleFromMap` moves it)
/// gate is still not applied — the original has three UI writers for that
/// switch and **no AI writer at all**, so gating on it here would stop every AI
/// realm building a castle. That is a rule which is right for the original's
/// human player and wrong for everybody else in it
/// the switch has not been found.
pub fn ceilings(county: &County) -> [i32; JOB_COUNT] {
    let mut out = [0i32; JOB_COUNT];
    for (job, slot) in out.iter_mut().enumerate() {
        *slot = match county.labour_useful[job] {
            crate::county::LABOUR_UNSET => 0,
            v => v,
        };
    }
    let gate = |job: usize, on: bool, out: &mut [i32; JOB_COUNT]| {
        if !on {
            out[job] = 0;
        }
    };
    gate(JOB_WOOD_CUTTING, county.industry[Commodity::Wood.index() as usize].enabled, &mut out);
    gate(JOB_IRON_MINING, county.industry[Commodity::Iron.index() as usize].enabled, &mut out);
    gate(
        JOB_STONE_QUARRYING,
        county.industry[Commodity::Stone.index() as usize].enabled,
        &mut out,
    );
    gate(JOB_BLACKSMITH, county.industry[Commodity::Weapons.index() as usize].enabled, &mut out);
    // `+0x1C3`: a castle is. Our field is named
    // `castle_degraded`, and it is a byte with three values
    // — 1 for a build and 2 for a post-siege repair, both of which are work the
    // allocator and `Castle_BuildEstimate` both read it as "a build is in
    // progress", and `Tax_CollectAll` charging the *lower* castle while it is
    // set says the same thing.
    gate(JOB_CASTLE_BUILDING, county.castle_degraded != 0, &mut out);
    out[JOB_IDLE_TOWNSFOLK] = 0;
    out
}

/// One pass of `FUN_0044F6E7` over one county.
pub fn allocate(county: &mut County) -> i32 {
    let ceiling = ceilings(county);
    let pop = county.population;
    let mut industry_pool = pct(pop, county.industry_share);
    let mut farm_pool = pop - industry_pool;

    county.labour = [0; JOB_COUNT];

    spend(county, &ceiling, &mut farm_pool, &FARM_QUOTA_ORDER, &FARM_ROUND, FARM_TAIL);

    if farm_pool < county.pop_band {
        industry_pool += farm_pool;
        farm_pool = 0;
    }

    spend(
        county,
        &ceiling,
        &mut industry_pool,
        &INDUSTRY_QUOTA_ORDER,
        &INDUSTRY_ROUND,
        INDUSTRY_TAIL,
    );

    let idle = farm_pool + industry_pool;
    county.labour[JOB_IDLE_TOWNSFOLK] = idle;
    idle
}

fn spend(
    county: &mut County,
    ceiling: &[i32; JOB_COUNT],
    pool: &mut i32,
    quota_order: &[usize],
    round: &[(usize, usize)],
    tail: usize,
) {
    let start = *pool;
    for &job in quota_order {
        let mut quota = pct(start, county.labour_share[job]);
        while quota > 0 && county.labour[job] < ceiling[job] {
            county.labour[job] += 1;
            quota -= 1;
            *pool -= 1;
        }
    }

    let mut progress = *pool >= 1;
    'outer: while *pool > 0 {
        loop {
            if !progress {
                return;
            }
            progress = false;
            for &(job, places) in round {
                for _ in 0..places {
                    if county.labour[job] < ceiling[job] {
                        county.labour[job] += 1;
                        progress = true;
                        *pool -= 1;
                        if *pool < 1 {
                            return;
                        }
                    }
                }
            }
            if county.labour[tail] < ceiling[tail] {
                break;
            }
        }
        county.labour[tail] += 1;
        progress = true;
        *pool -= 1;
        if *pool <= 0 {
            break 'outer;
        }
    }
}

