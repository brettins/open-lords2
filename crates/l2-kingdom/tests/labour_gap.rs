//! **The labour allocator is not in the season pipeline, and here is the list
//! of what has to land before it can be.**
//!
//! ```text
//! cargo test -p l2-kingdom --test labour_gap
//! ```
//!
//! `crates/l2-kingdom/src/labour.rs` reproduces `Labour_Allocate`
//! (`FUN_0044F6E7`) exactly and rebuilds all fourteen counties' worker counts
//! from the England turn-one save. It is called from
//! [`l2_kingdom::field::set_type`] and [`l2_kingdom::Kingdom::toggle_industry`],
//! where the original calls it, and **not** from `Season_Advance`, where the
//! original also calls it — twice.
//!
//! # Why not, precisely
//!
//! `Season_Advance` (`0x00448440`) does not call `County_RefreshEstimates` at
//! all before its two `Labour_AllocateAll`s. It does not have to: **each
//! estimate is the tail call of the pass that invalidates it.**
//!
//! | ceiling | refreshed at the end of | this crate's pass |
//! |---|---|---|
//! | the five field counts | `FUN_00469B51`, its own pipeline entry | **missing** |
//! | reclamation | `Field_ReclaimEstimate`, after `Field_ReclaimTick` | `FieldReclaim` |
//! | grain | `Grain_LabourEstimate`, last line of `Grain_SeasonTick` | `GrainSeasonTick` |
//! | cattle | `Herd_LabourEstimate`, last line of `Herd_SeasonTick` | `HerdSeasonTick` |
//! | four industries | `Industry_LabourEstimate`, inside `Industry_ProduceAll` | `Industry(_)` |
//! | castle | `Castle_BuildEstimate`, both arms of `Castle_BuildTick` | `CastleBuildTick` |
//!
//! So wiring the allocator is not one function; it is six tail calls, and three
//! of the six cannot be written honestly yet:
//!
//! * **grain outside the sowing season.** `Grain_Grow` and `Grain_Harvest` are
//!   not this crate's [`l2_kingdom::land::grow`] and
//!   [`l2_kingdom::land::harvest`]: the original caps the crop at
//!   `labour * divisor` *every* season and applies fertility at the growing
//!   step, and it carries **one** crop word where `County::crop` carries three.
//! * **the four industries.** `Industry_LabourEstimate`'s weapons pass reads
//!   the owning **realm's** wood and iron, so `County_RefreshEstimates` is not
//!   a `&mut County` function at all.
//! * **the castle.** `Castle_BuildEstimate`'s ceiling is 0 until the build's
//!   materials have all been delivered, and this crate debits them up front and
//!   holds none of the six fields that track the delivery.
//!
//! And one thing that would make wiring it a **silent no-op** rather than a
//! wrong number, which is worse: [`l2_kingdom::land::reclaim_fields`] advances
//! every started field by a flat quarter whatever the county's reclamation
//! labour is. The original spends `labour[2]` as a budget. Put people on
//! reclamation today and nothing at all happens.
//!
//! # What this file is for
//!
//! `docs/decisions.md` C12: *a test that passes before and after the change is
//! not testing the thing its name claims.* These assert the gap itself, so the
//! first person to close half of it gets a red build and this list to read.

use l2_kingdom::phase::SEASON_PIPELINE;
use l2_kingdom::tables::{Season, Tables, JOB_CATTLE_FARMING, JOB_GRAIN_FARMING};
use l2_kingdom::{field, land, County, Kingdom};

/// The pipeline has no allocation pass and no field recount, and the original's
/// has both. **When you add either, this goes red — read the module docs above
/// before deleting it.**
#[test]
fn the_season_pipeline_still_has_neither_a_recount_nor_an_allocation() {
    let names: Vec<String> = SEASON_PIPELINE.iter().map(|p| format!("{p:?}")).collect();
    let joined = names.join(" ");
    assert!(
        !joined.contains("Labour"),
        "the allocator is in the pipeline now; six estimate tail calls have to be in it too — \
         see this file's module documentation.\n{joined}"
    );
    assert!(
        !joined.contains("Recount"),
        "`FUN_00469B51` is in the pipeline now, which is the first of the six.\n{joined}"
    );
    assert_eq!(SEASON_PIPELINE.len(), 25, "and the length is written down too");
}

/// Three of the nine ceilings are computed, six are not. Named one by one, so
/// that filling one in is a one-line edit here and not a rewrite.
#[test]
fn exactly_three_of_the_nine_ceilings_are_refreshed_when_a_field_is_painted() {
    let t = &Tables::DEFAULT;
    let mut c = County::new();
    c.population = 400;
    c.pop_band = c.compute_pop_band();
    c.herd = 60;
    c.fields_cattle = 8;
    c.fields_grain = 4;
    c.grain = 500;
    c.herd_crowding = land::herd_crowding(t, c.herd, c.fields_cattle);

    // Every ceiling starts at the "never estimated" sentinel.
    c.labour_useful = [l2_kingdom::county::LABOUR_UNSET; 9];
    let map = l2_kingdom::CampaignMap::empty();
    field::refresh_estimates(&mut c, &map, Season::Spring, t, false);

    let refreshed: Vec<usize> = (0..9)
        .filter(|&j| c.labour_useful[j] != l2_kingdom::county::LABOUR_UNSET)
        .collect();
    assert_eq!(
        refreshed,
        vec![
            JOB_GRAIN_FARMING,
            JOB_CATTLE_FARMING,
            l2_kingdom::tables::JOB_FIELD_RECLAMATION
        ],
        "grain, cattle and reclamation are the three `County_RefreshEstimates` passes \
         this crate can reproduce; the four industries and the castle are not"
    );
}

/// And the grain ceiling is only computed for the season the sowing happens in.
#[test]
fn the_grain_ceiling_is_the_sowing_seasons_and_the_other_three_are_absent() {
    let t = &Tables::DEFAULT;
    let mut c = County::new();
    c.population = 200;
    c.pop_band = c.compute_pop_band();
    c.fields_grain = 4;
    c.grain = 500;

    assert!(
        land::grain_labour_estimate(t, &c, Season::Spring, false).is_some(),
        "Grain_Sow is `sacks_per_field`, which this crate has"
    );
    for season in [Season::Summer, Season::Autumn, Season::Winter] {
        assert_eq!(
            land::grain_labour_estimate(t, &c, season, false),
            None,
            "{season:?} needs Grain_Grow / Grain_Harvest, which are not this crate's \
             `grow` and `harvest` — see the module documentation"
        );
    }
}

/// **The consequence, stated as a measurement rather than as a worry.**
///
/// Nothing redistributes labour between seasons, so a county whose population
/// grows leaves the extra people in no job at all and the nine records stop
/// summing to the population. That is the invariant that proved the `0x0C`
/// stride in the first place, and this records exactly how fast it breaks.
#[test]
fn the_nine_records_stop_summing_to_the_population_from_the_second_season() {
    let mut k = Kingdom::new(11);
    assert!(k.set_county_count(1));
    let c = &mut k.counties[1];
    c.owner = 1;
    c.population = 400;
    c.pop_band = c.compute_pop_band();
    c.herd = 60;
    c.fields_cattle = 8;
    c.labour_useful[JOB_CATTLE_FARMING] = 400;
    l2_kingdom::labour::allocate(c);
    assert_eq!(c.labour.iter().sum::<i32>(), c.population, "season one closes");

    let mut broke_at = None;
    for season in 1..=6 {
        k.advance_season();
        let c = &k.counties[1];
        if c.labour.iter().sum::<i32>() != c.population && broke_at.is_none() {
            broke_at = Some(season);
        }
    }
    assert!(
        broke_at.is_some(),
        "the sum still closes after six seasons, which would mean the allocator is \
         running — read this file's module documentation"
    );
}
