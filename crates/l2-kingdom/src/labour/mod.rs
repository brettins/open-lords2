//! `FUN_0044F6E7` (`0x0044F6E7`, 2,147 bytes)
//! feed it. It is the writer of the nine job records, and it is called from
//! about fifteen places: whenever a field is repainted, whenever a county
//! changes hands, at new-game setup, and **twice** in the season pipeline —
//! once after `Castle_BuildTick` and before migration, and once again after the
//! armies have been recounted.
//!
//! One input that turned out not to be needed: **`Labour_Allocate` never reads
//! [`County::labour_wanted`]** — verified by exhaustion over its 2,147 bytes,
//! which read `+0xCC + slot*0x0C` eight times and `+0xC8 + slot*0x0C` not once.

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
/// **Corrected:** the enclosing function is `Season_Advance` at **`0x00448440`**,
/// 261 bytes. This constant's documentation used to give `0x0044C1EE`, which is
/// an address *inside* `Field_ReclaimTick` (`0x0044C093` + 485). The two call
/// sites themselves were right.
pub const SEASON_CALL_SITES: [&str; 2] =
    ["after Castle_BuildTick, before Migration_UpdateAll", "after FUN_00448D16, before Panels_RefreshAll"];

const FARM_QUOTA_ORDER: [usize; 3] =
    [JOB_CATTLE_FARMING, JOB_GRAIN_FARMING, JOB_FIELD_RECLAMATION];

const INDUSTRY_QUOTA_ORDER: [usize; 5] = [
    JOB_WOOD_CUTTING,
    JOB_STONE_QUARRYING,
    JOB_IRON_MINING,
    JOB_BLACKSMITH,
    JOB_CASTLE_BUILDING,
];

/// `FUN_0044F6E7`'s inner loop tests grain twice and cattle three times before
/// it lets reclamation have one, so the leftovers go five-to-one towards the
/// two jobs that feed people.
const FARM_ROUND: [(usize, usize); 2] = [(JOB_GRAIN_FARMING, 2), (JOB_CATTLE_FARMING, 3)];

const INDUSTRY_ROUND: [(usize, usize); 4] = [
    (JOB_WOOD_CUTTING, 1),
    (JOB_STONE_QUARRYING, 1),
    (JOB_IRON_MINING, 1),
    (JOB_BLACKSMITH, 1),
];

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

/// **The industry twin has no divisor at all** — `FUN_004502CA` uses
/// `g_shareTable[n]` neat. That asymmetry is the original's; a job joining the
/// farm gets a *third* of an even split and a job joining industry gets the
/// whole of one.
pub const FARM_GROUP_DIVISOR: i32 = 3;

const FARM_GROUP: [usize; 3] =
    [JOB_GRAIN_FARMING, JOB_CATTLE_FARMING, JOB_FIELD_RECLAMATION];
const FARM_SEED: usize = 1;

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

    #[test]
    fn a_job_with_no_ceiling_takes_nobody_and_the_rest_stand_idle() {
        let mut c = owned();
        c.labour_useful[JOB_WOOD_CUTTING] = 0;
        let idle = allocate(&mut c);
        assert_eq!(c.labour[JOB_WOOD_CUTTING], 0, "no forestry to be had");
        assert!(idle > 0, "and the industry half has nowhere to go");
        assert_eq!(c.labour.iter().sum::<i32>(), c.population);

        c.labour_useful[JOB_WOOD_CUTTING] = LABOUR_UNBOUNDED;
        let idle = allocate(&mut c);
        assert_eq!(idle, 0);
        assert!(c.labour[JOB_WOOD_CUTTING] > 0);
    }

    #[test]
    fn no_job_is_ever_filled_past_its_useful_ceiling() {
        let mut c = owned();
        c.industry_share = 0; // every one of the 435 offered to the farm
        allocate(&mut c);
        assert_eq!(c.labour[JOB_CATTLE_FARMING], 302, "the herd's own ceiling");
        assert_eq!(c.labour.iter().sum::<i32>(), 435);
    }

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

        c.labour = [0; JOB_COUNT];
        recompute_shares(&mut c);
        assert_eq!(c.labour_share[0] + c.labour_share[1] + c.labour_share[2], 100);
        assert_eq!(c.labour_share[3..8].iter().sum::<i32>(), 100);
        assert_eq!(c.labour_share[1], 100, "and it all lands on cattle and on wood");
        assert_eq!(c.labour_share[JOB_WOOD_CUTTING], 100);
    }

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

