//! The labour allocator — who works, and at what.
//!
//! `FUN_0044F6E7` (`0x0044F6E7`, 2,147 bytes), and the two small functions that
//! feed it. It is the writer of the nine job records, and it is called from
//! about fifteen places: whenever a field is repainted, whenever a county
//! changes hands, at new-game setup, and **twice** in the season pipeline —
//! once after `Castle_BuildTick` and before migration, and once again after the
//! armies have been recounted.
//!
//! # The shape of it
//!
//! ```text
//!    population
//!        │
//!        ├── Pct(pop, industryShare)  ─────────► industry pool
//!        └── the rest                ─────────► farm pool
//!
//!    farm pool     → cattle, grain, reclamation      each up to its ceiling
//!    industry pool → wood, stone, iron, smith, castle    ditto
//!    whatever neither could take → Idle townsfolk
//! ```
//!
//! Each half is spent twice. **First by quota**: every job gets
//! `Pct(pool, share[job])` people, or as many of them as its ceiling allows.
//! **Then by round robin**: whatever the quotas left over is walked round the
//! jobs that still have room, until either the pool is empty or every job is
//! full. The round is not even — the farm half offers grain two places and
//! cattle three before it offers reclamation one, and the industry half offers
//! wood, stone, iron and the smithy one each before castle building — so a
//! county with slack fills its cattle before its fields.
//!
//! # Why the ceiling is the interesting number
//!
//! [`County::labour_useful`] is what stops each loop, and it is the reason the
//! shipped save looks the way it does. An owned county's wood ceiling is
//! 100,000 and an unowned one's is 0, so the owned county's spare people become
//! foresters and the unowned county's become idle. That is the whole
//! difference between `[0, 218, 0, 0, 0, 0, 217, 0, 0]` and
//! `[0, 323, 0, 0, 0, 0, 0, 0, 133]`.
//!
//! # What is not here
//!
//! **The estimates that fill the floors and the ceilings.** `FUN_004485A5` runs
//! seven of them for one county — reclamation, grain, herd, four industries and
//! the castle — and this crate has none. Until it does, [`allocate`] must be
//! given ceilings from somewhere else, which today means the ones the scenario
//! imported. So this module is **not wired into
//! [`crate::phase`]**: an allocator running against a stale ceiling every
//! season would move people on evidence that stopped being true, which is worse
//! than the honest gap of not moving them at all. The two call sites are named
//! in [`SEASON_CALL_SITES`] so the next hand does not have to find them again.

use crate::county::County;
use crate::math::pct;
use crate::tables::{
    Commodity, JOB_CASTLE_BUILDING, JOB_CATTLE_FARMING, JOB_COUNT, JOB_FIELD_RECLAMATION,
    JOB_GRAIN_FARMING, JOB_IDLE_TOWNSFOLK, JOB_IRON_MINING, JOB_STONE_QUARRYING, JOB_BLACKSMITH,
    JOB_WOOD_CUTTING,
};

/// Where `Season_Advance` (`0x0044C1EE`) calls `FUN_0044F699`, which is
/// [`allocate`] for every county in index order.
///
/// Both are **after** the pass that changes how many people there are and
/// before anything that reads a worker count, which is what keeps the nine
/// records summing to the population.
pub const SEASON_CALL_SITES: [&str; 2] =
    ["after Castle_BuildTick, before Migration_UpdateAll", "after FUN_00448D16, before Panels_RefreshAll"];

/// The three farm jobs, in the order the allocator serves their quotas.
///
/// **Cattle is served first**, which is not the slot order and is not an
/// accident: the herd is the only thing in a county that dies for want of
/// staff.
const FARM_QUOTA_ORDER: [usize; 3] =
    [JOB_CATTLE_FARMING, JOB_GRAIN_FARMING, JOB_FIELD_RECLAMATION];

/// The five industry jobs, likewise: wood, stone, iron, blacksmith, castle.
const INDUSTRY_QUOTA_ORDER: [usize; 5] = [
    JOB_WOOD_CUTTING,
    JOB_STONE_QUARRYING,
    JOB_IRON_MINING,
    JOB_BLACKSMITH,
    JOB_CASTLE_BUILDING,
];

/// How many places each farm job gets in one turn of the round robin.
///
/// `FUN_0044F6E7`'s inner loop tests grain twice and cattle three times before
/// it lets reclamation have one, so the leftovers go five-to-one towards the
/// two jobs that feed people.
const FARM_ROUND: [(usize, usize); 2] = [(JOB_GRAIN_FARMING, 2), (JOB_CATTLE_FARMING, 3)];

/// And one each for the four industries, before castle building gets one.
const INDUSTRY_ROUND: [(usize, usize); 4] = [
    (JOB_WOOD_CUTTING, 1),
    (JOB_STONE_QUARRYING, 1),
    (JOB_IRON_MINING, 1),
    (JOB_BLACKSMITH, 1),
];

/// The job that gets the single place at the end of each round, per half.
const FARM_TAIL: usize = JOB_FIELD_RECLAMATION;
const INDUSTRY_TAIL: usize = JOB_CASTLE_BUILDING;

/// Each job's ceiling as the allocator sees it: the stored
/// [`County::labour_useful`], with [`crate::county::LABOUR_UNSET`] read as
/// zero and every job whose resource the county has not got forced to zero.
///
/// The gates are `FUN_0044F6E7`'s own, and they are the *enable* byte
/// (`+0x297 + c*0x18`) rather than the *has-resource* byte beside it — an
/// industry switched off allocates nobody even where the ore is.
///
/// **One gate is not modelled**: castle building is gated on `+0x1C3` *and* on
/// `+0x1B0`, and `+0x1B0` is a switch `FUN_00439CC2` throws the first time the
/// player drags builders onto the castle. This crate has no such field, so only
/// the first gate is applied and the second is treated as thrown.
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
    // `+0x1C3`: a castle is actually under construction. Our field is named
    // `castle_degraded`, which `docs/kingdom.md` admits was never traced; the
    // allocator and `Castle_BuildEstimate` both read it as "a build is in
    // progress", and `Tax_CollectAll` charging the *lower* castle while it is
    // set says the same thing.
    gate(JOB_CASTLE_BUILDING, county.castle_degraded, &mut out);
    // Idle townsfolk has no ceiling and takes the remainder.
    out[JOB_IDLE_TOWNSFOLK] = 0;
    out
}

/// One pass of `FUN_0044F6E7` over one county.
///
/// Clears all nine records and refills them. Returns the number of people left
/// idle, which is also what lands in [`crate::tables::JOB_IDLE_TOWNSFOLK`].
pub fn allocate(county: &mut County) -> i32 {
    let ceiling = ceilings(county);
    let pop = county.population;
    let mut industry_pool = pct(pop, county.industry_share);
    let mut farm_pool = pop - industry_pool;

    county.labour = [0; JOB_COUNT];

    spend(county, &ceiling, &mut farm_pool, &FARM_QUOTA_ORDER, &FARM_ROUND, FARM_TAIL);

    // Farm leftovers smaller than one icon are handed to industry rather than
    // left to become idle. `popBand` is the county's own icon size, so this is
    // "less than one peasant icon's worth".
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

/// One half of the allocator: quotas first, then the uneven round robin.
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

    // `do { ... } while (ceiling[tail] <= labour[tail]); labour[tail]++;`
    // wrapped in `while (0 < pool)`. Spelled out rather than transcribed,
    // because the original's four gotos do not translate.
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

/// `FUN_0044FF4A` — rewrite [`County::industry_share`] from what was actually
/// assigned.
///
/// **Half the idle count goes to industry**, which is the one surprising part:
/// a county with people doing nothing drifts towards a 50/50 split rather than
/// towards whichever half it last favoured.
pub fn recompute_industry_share(county: &mut County) {
    let pop = county.population;
    let farm = county.labour[JOB_GRAIN_FARMING]
        + county.labour[JOB_CATTLE_FARMING]
        + county.labour[JOB_FIELD_RECLAMATION];
    let idle = county.labour[JOB_IDLE_TOWNSFOLK];
    county.industry_share = crate::math::pct_of(pop - farm - idle + idle / 2, pop);
}

/// `FUN_00450000` — rewrite the eight percentages from the worker counts.
///
/// Two independent groups, each renormalised to exactly 100 by giving the
/// rounding remainder to whichever job in that group already has the largest
/// share. That is why the two defaults in the binary both close: they are what
/// this function produces.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::county::{LABOUR_NO_FLOOR, LABOUR_UNBOUNDED};

    /// A county of the shipped save, set up as the file has it.
    fn owned() -> County {
        let mut c = County::new();
        c.population = 435;
        c.pop_band = (435 - 1) / 25 + 1;
        c.industry_share = 50;
        c.labour_share = [0, 100, 0, 0, 0, 0, 100, 0];
        c.labour_wanted = [LABOUR_NO_FLOOR; JOB_COUNT];
        c.labour_wanted[JOB_CATTLE_FARMING] = 302;
        c.labour_useful = [0; JOB_COUNT];
        c.labour_useful[JOB_CATTLE_FARMING] = 302;
        c.labour_useful[JOB_WOOD_CUTTING] = LABOUR_UNBOUNDED;
        c
    }

    /// **The whole point of the pass**: every person in the county ends up in
    /// exactly one of the nine records, whatever the ceilings say.
    #[test]
    fn the_nine_records_always_sum_to_the_population() {
        for share in [0i32, 25, 50, 75, 100] {
            for pop in [1i32, 25, 100, 435, 456, 2000] {
                let mut c = owned();
                c.population = pop;
                c.pop_band = (pop - 1) / 25 + 1;
                c.industry_share = share;
                allocate(&mut c);
                assert_eq!(
                    c.labour.iter().sum::<i32>(),
                    pop,
                    "pop {pop} at {share}% industry: {:?}",
                    c.labour
                );
                assert!(c.labour.iter().all(|&n| n >= 0), "nobody is negative");
            }
        }
    }

    /// A ceiling of zero is why an unowned county's people stand idle: with no
    /// industry enabled and no wood ceiling, the whole industry half falls
    /// through to *Idle townsfolk*.
    #[test]
    fn a_job_with_no_ceiling_takes_nobody_and_the_rest_stand_idle() {
        let mut c = owned();
        c.labour_useful[JOB_WOOD_CUTTING] = 0;
        let idle = allocate(&mut c);
        assert_eq!(c.labour[JOB_WOOD_CUTTING], 0, "no forestry to be had");
        assert!(idle > 0, "and the industry half has nowhere to go");
        assert_eq!(c.labour.iter().sum::<i32>(), c.population);

        // Give it back an unbounded ceiling and nobody is idle at all.
        c.labour_useful[JOB_WOOD_CUTTING] = LABOUR_UNBOUNDED;
        let idle = allocate(&mut c);
        assert_eq!(idle, 0);
        assert!(c.labour[JOB_WOOD_CUTTING] > 0);
    }

    /// The herd's ceiling binds: 302 is all the dairy maids the herd can use,
    /// and the 303rd person goes somewhere else however big the farm share is.
    #[test]
    fn no_job_is_ever_filled_past_its_useful_ceiling() {
        let mut c = owned();
        c.industry_share = 0; // every one of the 435 offered to the farm
        allocate(&mut c);
        assert_eq!(c.labour[JOB_CATTLE_FARMING], 302, "the herd's own ceiling");
        assert_eq!(c.labour.iter().sum::<i32>(), 435);
    }

    /// The industry gates are the *enabled* byte, not the resource: an industry
    /// switched off allocates nobody even where the ore is.
    #[test]
    fn a_switched_off_industry_is_a_ceiling_of_zero() {
        let mut c = owned();
        c.labour_useful[JOB_IRON_MINING] = LABOUR_UNBOUNDED;
        c.industry[Commodity::Iron.index() as usize].has_resource = true;
        c.industry[Commodity::Iron.index() as usize].enabled = false;
        assert_eq!(ceilings(&c)[JOB_IRON_MINING], 0);
        c.industry[Commodity::Iron.index() as usize].enabled = true;
        assert_eq!(ceilings(&c)[JOB_IRON_MINING], LABOUR_UNBOUNDED);
    }

    /// The two shares the binary ships as defaults are what `FUN_00450000`
    /// produces, which is the check that this is the same arithmetic: each half
    /// comes back summing to exactly 100.
    #[test]
    fn recomputing_the_shares_leaves_each_half_summing_to_a_hundred() {
        let mut c = owned();
        c.labour = [100, 200, 35, 10, 20, 30, 40, 0, 0];
        recompute_shares(&mut c);
        assert_eq!(c.labour_share[0] + c.labour_share[1] + c.labour_share[2], 100);
        assert_eq!(c.labour_share[3..8].iter().sum::<i32>(), 100);

        // A county doing nothing at all still comes back at 100 apiece, because
        // the remainder is handed out whatever the counts were.
        c.labour = [0; JOB_COUNT];
        recompute_shares(&mut c);
        assert_eq!(c.labour_share[0] + c.labour_share[1] + c.labour_share[2], 100);
        assert_eq!(c.labour_share[3..8].iter().sum::<i32>(), 100);
        assert_eq!(c.labour_share[1], 100, "and it all lands on cattle and on wood");
        assert_eq!(c.labour_share[JOB_WOOD_CUTTING], 100);
    }

    /// Half the idle count counts as industry, so a county with people doing
    /// nothing drifts towards an even split.
    #[test]
    fn the_industry_share_counts_half_the_idle_as_industry() {
        let mut c = owned();
        c.population = 100;
        c.labour = [0; JOB_COUNT];
        c.labour[JOB_CATTLE_FARMING] = 40;
        c.labour[JOB_WOOD_CUTTING] = 20;
        c.labour[JOB_IDLE_TOWNSFOLK] = 40;
        recompute_industry_share(&mut c);
        assert_eq!(c.industry_share, 40, "20 working and 20 of the 40 idle");
    }
}
