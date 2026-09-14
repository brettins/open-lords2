//! The labour allocator — who works, and at what.
//!
//! `FUN_0044F6E7` (`0x0044F6E7`, 2,147 bytes)
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
//! cattle three before it offers reclamation one
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
//! # Where it runs
//!
//! Wherever a **click** reaches it — `Field_SetType` and
//! `Industry_ToggleFromMap` both allocate straight afterwards, and so do
//! [`crate::field::set_type`] and [`crate::Kingdom::toggle_industry`] — and
//! **twice in the season pipeline**, [`crate::phase::Pass::LabourAllocate`] and
//! [`crate::phase::Pass::LabourAllocateAgain`], at the two points
//! [`SEASON_CALL_SITES`] names.
//!
//! It was not in the pipeline for a long time
//! because it is the shape of every "why is this pass not wired in" question.
//! `Season_Advance` does not call `County_RefreshEstimates` before either
//! allocation at all: **each of the nine ceilings is refreshed by the pass that
//! invalidates it, as that pass's tail call**, and `County_RefreshEstimates`
//! itself runs at the very end of the season as the middle statement of
//! `Panels_RefreshAll` ([`crate::phase::Pass::RefreshEstimates`]). Wiring the
//! allocator meant writing six tail calls, three of which needed real work
//! first — `Grain_Grow` and `Grain_Harvest` for the grain ceiling outside
//! sowing, the owning **realm** for the four industries, and a materials model
//! for the castle. `crates/l2-kingdom/tests/labour_gap.rs` is the record of it,
//! including the one piece still inferred.
//!
//! One input that turned out not to be needed: **`Labour_Allocate` never reads
//! [`County::labour_wanted`]** — verified by exhaustion over its 2,147 bytes,
//! which read `+0xCC + slot*0x0C` eight times and `+0xC8 + slot*0x0C` not once.
//! The floors are what the county panel draws a worker count red against, and
//! nothing here depends on them.

mod allocation;
pub use allocation::*;
mod shares;
pub use shares::*;

use crate::county::County;
use crate::math::pct;
use crate::tables::{
    Commodity, JOB_CASTLE_BUILDING, JOB_CATTLE_FARMING, JOB_COUNT, JOB_FIELD_RECLAMATION,
    JOB_GRAIN_FARMING, JOB_IDLE_TOWNSFOLK, JOB_IRON_MINING, JOB_STONE_QUARRYING, JOB_BLACKSMITH,
    JOB_WOOD_CUTTING,
};

/// Where `Season_Advance` (`0x00448440`) calls `FUN_0044F699`, which is
/// [`allocate`] for every county in index order.
///
/// Both are **after** the pass that changes how many people there are and
/// before anything that reads a worker count, which is what keeps the nine
/// records summing to the population.
///
/// **Corrected:** the enclosing function is `Season_Advance` at **`0x00448440`**,
/// 261 bytes. This constant's documentation used to give `0x0044C1EE`, which is
/// an address *inside* `Field_ReclaimTick` (`0x0044C093` + 485). The two call
/// sites themselves were right.
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

/// `g_shareTable` (`0x004D6768`) — `{100, 50, 33, 25, 20, 0, 5, 0}`.
///
/// **`[V]`**, byte for byte out of `Lords2.exe`. The first five entries are
/// `100 / (n + 1)` for `n` = 0 … 4 — an even split once one more job joins the
/// `n` that already have a share. The last three are not that sequence and are
/// never indexed: the two callers count non-zero shares among **three** and
/// **five** jobs, so the largest `n` either can reach is 4.
pub const SHARE_TABLE: [i32; 8] = [100, 50, 33, 25, 20, 0, 5, 0];

/// The divisor [`toggle_share`]'s caller passes: the farm group has three
/// members. `Field_SetType` calls `Labour_ToggleShare(county, 2, on, 3)`.
///
/// **The industry twin has no divisor at all** — `FUN_004502CA` uses
/// `g_shareTable[n]` neat. That asymmetry is the original's; a job joining the
/// farm gets a *third* of an even split and a job joining industry gets the
/// whole of one.
pub const FARM_GROUP_DIVISOR: i32 = 3;

/// The three jobs [`toggle_share`] renormalises
/// search is seeded with — cattle, so a tie goes to the herd.
const FARM_GROUP: [usize; 3] =
    [JOB_GRAIN_FARMING, JOB_CATTLE_FARMING, JOB_FIELD_RECLAMATION];
const FARM_SEED: usize = 1;

/// The five [`toggle_industry_share`] renormalises, seeded with wood.
///
/// The order is the original's own read order in `FUN_004502CA`
/// (`+0x148, +0x144, +0x140, +0x14C, +0x13C`), which is wood, stone, iron,
/// blacksmith, castle. Only the seed depends on it — the sum does not — and the
/// seed is what decides a tie.
const INDUSTRY_GROUP: [usize; 5] = [
    JOB_WOOD_CUTTING,
    JOB_STONE_QUARRYING,
    JOB_IRON_MINING,
    JOB_BLACKSMITH,
    JOB_CASTLE_BUILDING,
];
const INDUSTRY_SEED: usize = 0;

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

