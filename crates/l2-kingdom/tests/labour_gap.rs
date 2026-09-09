//! **The labour allocator is in the season pipeline now, and this is what it
//! took.** The file that used to record the gap records the closing of it.
//!
//! ```text
//! cargo test -p l2-kingdom --test labour_gap
//! ```
//!
//! # What the gap was
//!
//! `crates/l2-kingdom/src/labour.rs` reproduced `Labour_Allocate`
//! (`0x0044F6E7`) exactly and rebuilt all fourteen counties' worker counts from
//! the England turn-one save, and it ran only where a **click** reached it.
//! `Season_Advance` calls it twice and this crate called it never, so from turn
//! 2 a county's nine job records stopped summing to its population — the very
//! invariant that established the `0x0C` labour-record stride.
//!
//! The reason was not the allocator. It was that
//! **`Season_Advance` never calls `County_RefreshEstimates`**, so each of the
//! nine ceilings the allocator reads has to be refreshed by the pass that
//! invalidates it, as that pass's **tail call**:
//!
//! | ceiling | refreshed at the end of | this crate's pass |
//! |---|---|---|
//! | the five field counts | `FUN_00469B51`, its own pipeline entry | [`Pass::CountyRecountFields`] |
//! | reclamation | `Field_ReclaimEstimate`, after `Field_ReclaimTick` | [`Pass::FieldReclaim`] |
//! | grain | `Grain_LabourEstimate`, last line of `Grain_SeasonTick` | [`Pass::GrainSeasonTick`] |
//! | cattle | `Herd_LabourEstimate`, last line of `Herd_SeasonTick` | [`Pass::HerdSeasonTick`] |
//! | four industries | `Industry_LabourEstimate`, inside `Industry_ProduceAll` | [`Pass::RefreshEstimates`] |
//! | castle | `Castle_BuildEstimate`, both arms of `Castle_BuildTick` | [`Pass::RefreshEstimates`] |
//!
//! …plus the one that made the whole thing possible: **`County_RefreshEstimates`
//! *does* run every season**, once per county, as the middle statement of
//! `Panels_RefreshAll`, which is `Season_Advance`'s *last* call. It is
//! [`Pass::RefreshEstimates`]. `docs/kingdom.md` §3.4.
//!
//! # The three that could not be written, and what each needed
//!
//! * **grain outside the sowing season.** `Grain_Grow` and `Grain_Harvest` are
//!   now [`l2_kingdom::land::grow_step`] and
//!   [`l2_kingdom::land::harvest_step`], which cap the crop at
//!   `labour * multiplier` and apply fertility at the growing step. The crop
//!   model changed with them: [`County::crop`] is **seed, standing crop,
//!   harvest** and not three growth stages.
//! * **the four industries.** `Industry_LabourEstimate` reads the owning
//!   *realm*, so [`l2_kingdom::field::refresh_estimates`] takes one — and the
//!   blacksmith's share of the stockpile is
//!   [`l2_kingdom::industry::weapon_shares`], `FUN_0044F15B`, which was `[D]`
//!   and is now traced.
//! * **the castle.** `Castle_BuildEstimate`'s ceiling is 0 until the build's
//!   materials have been delivered; this crate debits them **up front**, so
//!   the gate is permanently open and the ceiling is the work outstanding.
//!   `[I]` on the model, not on the arithmetic — see
//!   [`l2_kingdom::industry::castle_labour_estimate`].
//!
//! And the one that would have made wiring the allocator a **silent no-op**:
//! [`l2_kingdom::land::reclaim_fields`] now spends `labour[2]` as a budget, the
//! way `Field_ReclaimTick` does, instead of advancing every started field by a
//! flat quarter. Putting people on reclamation now does something.
//!
//! # What is still open
//!
//! One thing, and it is named in [`the_castle_ceiling_still_rests_on_an_inferred_model`]:
//! the six words at county `+0x1CC … +0x1E0` that track a castle's material
//! *delivery* are not in [`County`], because this crate has no delivery to
//! track. Everything else in `County_RefreshEstimates` is reproduced.

use l2_kingdom::county::LABOUR_UNSET;
use l2_kingdom::phase::{Pass, SEASON_PIPELINE};
use l2_kingdom::tables::{
    Season, Tables, JOB_BLACKSMITH, JOB_CASTLE_BUILDING, JOB_CATTLE_FARMING, JOB_COUNT,
    JOB_FIELD_RECLAMATION, JOB_GRAIN_FARMING, JOB_IRON_MINING, JOB_STONE_QUARRYING,
    JOB_WOOD_CUTTING,
};
use l2_kingdom::{industry, land, County, Kingdom, Realm};

/// The pipeline has an allocation pass — twice — and a field recount, and the
/// original's has both.
#[test]
fn the_season_pipeline_allocates_twice_and_recounts_the_fields() {
    let names: Vec<String> = SEASON_PIPELINE.iter().map(|p| format!("{p:?}")).collect();
    let joined = names.join(" ");
    assert!(joined.contains("LabourAllocate"), "{joined}");
    assert!(joined.contains("CountyRecountFields"), "{joined}");
    assert_eq!(SEASON_PIPELINE.len(), 30, "and the length is written down too");

    // The order is the rule, and it is this: everything that moves a ceiling
    // runs before the first allocation.
    let at = |p: Pass| p.order();
    for pass in [
        Pass::CountyRecountFields,
        Pass::FieldReclaim,
        Pass::GrainSeasonTick,
        Pass::HerdSeasonTick,
        Pass::CastleBuildTick,
    ] {
        assert!(at(pass) < at(Pass::LabourAllocate), "{pass:?} must refresh before allocation");
    }
    assert!(
        at(Pass::PopulationUpdate) < at(Pass::LabourAllocateAgain),
        "and the second allocation is what counts the newborns"
    );
    assert!(
        at(Pass::LabourAllocateAgain) < at(Pass::RefreshEstimates),
        "Panels_RefreshAll is the last call of all"
    );
}

/// **All nine ceilings are refreshed now**, where three were before. Named one
/// by one, because the identity of the missing ones was the whole content of
/// this file's first version.
#[test]
fn every_one_of_the_nine_ceilings_is_refreshed() {
    let t = &Tables::DEFAULT;
    let mut c = County::new();
    c.owner = 1;
    c.population = 400;
    c.pop_band = c.compute_pop_band();
    c.herd = 60;
    c.fields_cattle = 8;
    c.fields_grain = 4;
    c.grain = 500;
    c.herd_crowding = land::herd_crowding(t, c.herd, c.fields_cattle);
    // A castle under construction, so the ninth ceiling has something to say.
    c.castle_building = 1;
    c.castle_degraded = true;

    c.labour_useful = [LABOUR_UNSET; JOB_COUNT];
    let map = l2_kingdom::CampaignMap::empty();
    let realm = Realm::new();
    l2_kingdom::field::refresh_estimates(
        &mut c,
        &map,
        Season::Spring,
        t,
        false,
        &realm,
        industry::WeaponShare::UNSHARED,
    );

    let refreshed: Vec<usize> =
        (0..JOB_COUNT).filter(|&j| c.labour_useful[j] != LABOUR_UNSET).collect();
    assert_eq!(
        refreshed,
        vec![
            JOB_GRAIN_FARMING,
            JOB_CATTLE_FARMING,
            JOB_FIELD_RECLAMATION,
            JOB_CASTLE_BUILDING,
            JOB_IRON_MINING,
            JOB_STONE_QUARRYING,
            JOB_WOOD_CUTTING,
            JOB_BLACKSMITH,
        ],
        "eight of the nine; the ninth is Idle townsfolk, which has no ceiling"
    );
    assert!(c.labour_useful[JOB_CASTLE_BUILDING] > 0, "a castle is being built");
    assert!(c.labour_useful[JOB_WOOD_CUTTING] > 0, "an owned county can cut wood");
}

/// And the grain ceiling is a real number in **every** season, where it used to
/// be the sowing season's alone.
#[test]
fn the_grain_ceiling_is_computed_in_all_four_seasons() {
    let t = &Tables::DEFAULT;
    let mut c = County::new();
    c.population = 200;
    c.pop_band = c.compute_pop_band();
    c.fields_grain = 4;
    c.fields_grain_sown = 4;
    c.grain = 500;
    c.crop[1] = 480; // a standing crop for the two growing steps and the harvest

    for season in [Season::Spring, Season::Summer, Season::Autumn, Season::Winter] {
        let ceiling = land::grain_labour_estimate(t, &c, season, false)
            .unwrap_or_else(|| panic!("{season:?} still has no grain ceiling"));
        assert!(ceiling > 0, "{season:?} should want somebody on the fields");
    }

    // The only `None` left is the original's own guard: a county with nobody
    // in it writes no estimate at all.
    c.pop_band = 0;
    assert_eq!(land::grain_labour_estimate(t, &c, Season::Summer, false), None);
}

/// **The invariant, as a measurement.** The nine job records sum to the
/// population, exactly, in every season — which is what proved the `0x0C`
/// stride in the first place, and what this crate could not hold from turn 2.
#[test]
fn the_nine_records_sum_to_the_population_every_season() {
    let mut k = Kingdom::new(11);
    assert!(k.set_county_count(1));
    k.realms[1].strength = 3;
    let c = &mut k.counties[1];
    c.owner = 1;
    c.population = 400;
    c.pop_band = c.compute_pop_band();
    c.herd = 60;
    c.fields_cattle = 8;
    c.grain = 2000;
    c.labour_useful[JOB_CATTLE_FARMING] = 400;
    l2_kingdom::labour::allocate(c);
    assert_eq!(c.labour.iter().sum::<i32>(), c.population, "season one closes");

    for season in 1..=12 {
        k.advance_season();
        let c = &k.counties[1];
        assert_eq!(
            c.labour.iter().sum::<i32>(),
            c.population,
            "season {season}: {:?} against a population of {}",
            c.labour,
            c.population
        );
        assert!(c.labour.iter().all(|&n| n >= 0), "season {season}: nobody is negative");
    }
}

/// **The one thing still inferred**, stated so that closing it is a change to
/// this test and not a rediscovery.
///
/// `Castle_BuildEstimate` computes `min(100 - Pct(woodDelivered, woodNeeded),
/// 100 - Pct(stoneDelivered, stoneNeeded))` from six words at county
/// `+0x1CC … +0x1E0` and returns a ceiling of **0** until that reaches 100.
/// [`County`] has none of them, because [`industry::order_castle`] takes the
/// whole cost out of the realm the moment the castle is ordered — which is
/// `docs/kingdom.md` §7.5's reading and is why the gate is permanently open
/// here. The arithmetic given a complete delivery is reproduced; the delivery
/// is not modelled.
#[test]
fn the_castle_ceiling_still_rests_on_an_inferred_model() {
    let t = &Tables::DEFAULT;
    let mut c = County::new();
    assert_eq!(
        industry::castle_labour_estimate(t, &c),
        (l2_kingdom::county::LABOUR_NO_FLOOR, 0),
        "no build, no ceiling"
    );

    let mut realm = Realm::new();
    realm.wood = 100_000;
    realm.stone = 100_000;
    assert!(industry::order_castle(t, &mut c, &mut realm, 1));
    let (_, ceiling) = industry::castle_labour_estimate(t, &c);
    assert_eq!(
        ceiling,
        industry::castle_workforce(t, 1),
        "the whole workforce is outstanding the season it is ordered — and in the original \
         it would be 0 until the wood and stone had been carted in"
    );

    // …and it counts down as the work is done, which is the half that is the
    // original's arithmetic rather than this crate's model.
    c.castle_progress = ceiling / 4;
    assert_eq!(industry::castle_labour_estimate(t, &c).1, ceiling - ceiling / 4);
}
