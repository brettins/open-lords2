//! `crates/l2-kingdom/src/labour/mod.rs` reproduced `Labour_Allocate`
//! (`0x0044F6E7`) exactly and rebuilt all fourteen counties' worker counts from
//! the England turn-one save, and it ran only where a **click** reached it.
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
//! * **the four industries.** `Industry_LabourEstimate` reads the owning
//! *realm*, so [`l2_kingdom::field::refresh_estimates`] takes one — and the
//!   blacksmith's share of the stockpile is
//!   [`l2_kingdom::industry::weapon_shares`], `FUN_0044F15B`, which was `[D]`
//!   and is now traced.
//!
//! * **the castle.** `Castle_BuildEstimate`'s ceiling is 0 until the build's
//!   materials have been delivered. This file used to say the gate was
//!   permanently open because the crate debited the cost up front; it does not,
//! the six words at `+0x1CC … +0x1E0` are on [`County`] now, and the gate
//!   shuts. See [`l2_kingdom::industry::castle_labour_estimate`].
//!
//! The six words at county `+0x1CC … +0x1E0` that track a castle's material
//! *delivery*. They are in [`County`] now and
//! [`the_castle_ceiling_is_shut_until_the_wood_and_stone_have_arrived`] asserts
//! the gate they open and shut. Everything in `County_RefreshEstimates` is
//! reproduced.

use l2_kingdom::county::LABOUR_UNSET;
use l2_kingdom::phase::{Pass, SEASON_PIPELINE};
use l2_kingdom::tables::{
    Season, Tables, JOB_BLACKSMITH, JOB_CASTLE_BUILDING, JOB_CATTLE_FARMING, JOB_COUNT,
    JOB_FIELD_RECLAMATION, JOB_GRAIN_FARMING, JOB_IRON_MINING, JOB_STONE_QUARRYING,
    JOB_WOOD_CUTTING,
};
use l2_kingdom::{industry, land, County, Kingdom, Realm};

#[test]
fn the_season_pipeline_allocates_twice_and_recounts_the_fields() {
    let names: Vec<String> = SEASON_PIPELINE.iter().map(|p| format!("{p:?}")).collect();
    let joined = names.join(" ");
    assert!(joined.contains("LabourAllocate"), "{joined}");
    assert!(joined.contains("CountyRecountFields"), "{joined}");
    // 34 since `Pass::ArmyRecountTroops` — `Army_RecountCountyTroops`
    // (`0x004AD6C0`), whose tail re-prices the ration and the two farm ceilings
    // — went in before the second allocation.
    assert_eq!(SEASON_PIPELINE.len(), 34, "and the length is written down too");

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
    c.castle_type = 1;
    c.castle_degraded = l2_kingdom::siege::CASTLE_DEGRADED_BUILDING;
    c.castle_work_left = industry::castle_workforce(t, 1);
    c.castle_work_total = c.castle_work_left;

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
        let grain = land::grain_labour_estimate(t, &c, season, false)
            .unwrap_or_else(|| panic!("{season:?} still has no grain ceiling"));
        assert!(grain.useful > 0, "{season:?} should want somebody on the fields");
        assert_eq!(
            grain.wanted, grain.useful,
            "grain's floor and ceiling come from one loop variable, so they agree \
             whenever the search found anything at all"
        );
    }

    c.pop_band = 0;
    assert_eq!(land::grain_labour_estimate(t, &c, Season::Summer, false), None);
}

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

/// `Castle_BuildEstimate` computes `min(100 - Pct(woodOwed, woodTotal),
/// 100 - Pct(stoneOwed, stoneTotal))` from six words at county
/// `+0x1CC … +0x1E0` and returns a ceiling of **0** until that reaches 100.
///
/// The claim here was that `County` could not have those words because
/// `order_castle` took the whole cost up front — which was this crate's
/// invention, not `docs/kingdom.md`'s finding. `Castle_Order` (`0x00436D02`)
/// takes what the realm happens to have and leaves the rest owing, and
/// `Castle_DeliverMaterials` (`0x00450CCD`) carts the rest in season by season.
#[test]
fn the_castle_ceiling_is_shut_until_the_wood_and_stone_have_arrived() {
    let t = &Tables::DEFAULT;
    let mut c = County::new();
    c.owner = 1;
    c.population = 2_000;
    assert_eq!(
        industry::castle_labour_estimate(t, &c),
        (l2_kingdom::county::LABOUR_NO_FLOOR, 0),
        "no build, no ceiling"
    );

    let mut realm = Realm::new();
    assert!(industry::order_castle(t, &mut c, &mut realm, 1));
    assert_eq!((c.castle_wood_owed, c.castle_stone_owed), (400, 40), "all of it owed");
    assert_eq!(
        industry::castle_labour_estimate(t, &c).1,
        0,
        "nobody may lift a spade until the carts come"
    );
    assert_eq!(industry::castle_seasons_left(t, &c), 100, "and the panel says: for ever");

    // `PctOf(1, 400)` is 0,
    // delivered and the builders start. Reproduced
    // `FUN_00450FB4` is two `PctOf` calls and integer division, and a rule that
    // rounded the other way would idle a county over a rounding error.
    realm.wood = 399;
    realm.stone = 40;
    industry::deliver_castle_materials(&mut c, &mut realm);
    assert_eq!(c.castle_wood_owed, 1, "one stick short");
    assert_eq!(
        industry::castle_labour_estimate(t, &c).1,
        industry::castle_workforce(t, 1),
        "and one stick short is close enough for the original"
    );
    c.castle_wood_owed = 4;
    assert_eq!(industry::castle_labour_estimate(t, &c).1, 0, "1% short shuts it");

    realm.wood = 4;
    industry::deliver_castle_materials(&mut c, &mut realm);
    let (_, ceiling) = industry::castle_labour_estimate(t, &c);
    assert_eq!(ceiling, industry::castle_workforce(t, 1), "the whole workforce, now it is paid");

    c.castle_work_left = ceiling - ceiling / 4;
    assert_eq!(industry::castle_labour_estimate(t, &c).1, ceiling - ceiling / 4);
}
