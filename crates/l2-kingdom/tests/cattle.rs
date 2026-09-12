//! **The cattle labour record's first word — the break-even floor**, against
//! the number the original wrote into every save it ever made.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" \
//! LORDS2_FIXTURES="E:\dev\lords2-fixtures" cargo test -p l2-kingdom --test cattle
//! ```
//!
//! # The report
//!
//! > *"I don't know why 16 cows are being lost this season."*
//!
//! His county held 80 head on eight pastures with 114 milkmaids and a
//! population of 150. `Herd_BirthsAndDeaths` wants three a head — 240 people —
//! so the county was at 47 % staffing, and [`l2_kingdom::land::herd_growth`]
//! adds `(100 − staffing) / 3` to the death rate for the shortfall: seventeen
//! of the eighteen points that killed those cows were *nobody tending them*.
//! There is no arrangement of 150 people that would have saved the herd.
//!
//! **The game has exactly one way of saying that, and it is a number we never
//! computed.** `Herd_LabourEstimate` (`0x0044DD4D`) fills both spare words of
//! labour record 1 out of one loop: `+0xD8`, the growth-maximising staffing,
//! which the allocator fills up to, and `+0xD4`, the **first staffing at which
//! births stop trailing deaths**. Four things in the interface read `+0xD4` and
//! every one of them was reading a zero:
//!
//! * `Panel_JobDetail` colours the worker count red — `screens/info.rs`
//!   `workers_colour`, `screens/job.rs`;
//! * the county strip's produce icon switches to its *short* frame —
//!   `screens/county.rs`;
//! * `Village_RebuildIcons` draws the shortfall as extra unselectable icons in
//!   the cluster — `screens/village.rs`;
//! * the minimap's labour overlay drops to band 0 —
//!   [`l2_kingdom::county::County::minimap_bands`].
//!
//! So a player watching a sixth of his herd die every season was shown a black
//! number, a full cluster and a quiet map. `docs/decisions.md` CNEW-cattle-floor.
//!
//! # Why this is an oracle and not a model
//!
//! Every original save stores `+0xD4` for all fourteen counties, so the search
//! can be run against the game's own answer with nothing recovered and nothing
//! inverted — the same standing as
//! `tests/reproduction.rs::the_herds_own_forecast_reproduces_for_every_county`.
//! [`the_cattle_floor_is_the_break_even_staffing_every_original_save_stored`]
//! sweeps every `.sav` this machine can open rather than one fixture, because
//! one save agreeing proves nothing (`docs/decisions.md` C1) — and because the
//! fallback arm only appears in some of them.

use l2_formats::save::Save;
use l2_kingdom::county::{County, LABOUR_NO_FLOOR};
use l2_kingdom::land::{herd_growth, herd_labour_estimate};
use l2_kingdom::tables::{Tables, JOB_CATTLE_FARMING};
use l2_scenario::Scenario;

const SEED: u64 = 0x10D_52;
const T: &Tables = &Tables::DEFAULT;

/// One county field straight out of the file, at the offset `docs/kingdom.md`
/// §1 names it at. The same reader `tests/reproduction.rs` uses.
fn county_i32(save: &Save, id: usize, offset: u32) -> i32 {
    save.i32_at(0x0053_F9B0 + (id as u32) * 0x300 + offset).expect("a saved county address")
}

/// **The whole of `Herd_LabourEstimate`, against every save on this machine.**
///
/// Both words, every county, every `.sav` the install and the fixture directory
/// hold. The counts printed on failure are deliberate: a run that checks two
/// numbers is not the same evidence as a run that checks six hundred, and the
/// difference is invisible from a green tick.
///
/// **The fallback arm is exercised and is not a corner.** A county whose herd
/// cannot break even at any staffing its population could supply stores the
/// least-bad count instead — which is the argmax, so floor and ceiling come out
/// equal. `lastturn.sav`'s county 1 stores 302 and 302; two other positions
/// store 153/153 and 173/173. If the fallback were anything else — −1, zero,
/// the population — those three would part company and this would say so.
#[test]
fn the_cattle_floor_is_the_break_even_staffing_every_original_save_stored() {
    let saves = l2_testkit::every_available_save();
    if saves.is_empty() {
        eprintln!("no game install or fixture directory configured; skipping");
        return;
    }

    let mut checked = 0;
    let mut fallbacks = 0;
    let mut files = 0;
    for f in &saves {
        let Ok(s) = Scenario::from_save(&f.save) else { continue };
        files += 1;
        let k = s.kingdom(SEED);
        let next = s.clock.season_next;
        for id in s.county_ids() {
            let c = &k.counties[id];
            let ours = herd_labour_estimate(T, c, next);
            let stored_floor = county_i32(&f.save, id, 0xD4);
            let stored_ceiling = county_i32(&f.save, id, 0xD8);
            assert_eq!(
                ours.wanted, stored_floor,
                "{} county {id}: break-even staffing (+0xD4) for {} head on {} pasture, \
                 crowding {}, population {}",
                f.name, c.herd, c.fields_cattle, c.herd_crowding, c.population
            );
            assert_eq!(
                ours.useful, stored_ceiling,
                "{} county {id}: growth-maximising staffing (+0xD8)",
                f.name
            );
            if ours.wanted == ours.useful {
                fallbacks += 1;
            }
            checked += 2;
        }
    }

    assert!(files >= 2, "only {files} saves imported; the sweep needs more than one position");
    assert!(checked >= 100, "only {checked} stored numbers checked");
    assert!(
        fallbacks > 0,
        "no county in {files} saves took the cannot-break-even arm, so it is untested"
    );
}

/// **The county the player reported**, built from the numbers his save holds:
/// 80 head, eight pastures, crowding 10, 114 milkmaids, 150 people, entering
/// Winter.
///
/// Three assertions and they are three different claims. The herd really is
/// shrinking; the shrinkage really is understaffing rather than crowding or the
/// season; and the floor the original would have drawn red really is above the
/// staffing he had. The third is the one that was missing, and before the fix
/// `labour_wanted[1]` was 0 and `is_short` was false.
#[test]
fn the_herd_that_lost_sixteen_head_was_below_its_break_even_floor() {
    let mut c = County::new();
    c.owner = 1;
    c.population = 150;
    c.pop_band = 6;
    c.herd = 80;
    c.herd_eaten = 0;
    c.fields_cattle = 8;
    c.herd_crowding = 10;
    c.labour = [0; l2_kingdom::tables::JOB_COUNT];
    c.labour[JOB_CATTLE_FARMING] = 114;

    const WINTER: u8 = 4;
    let g = herd_growth(T, c.herd, c.fields_cattle, 114, c.herd_crowding, WINTER);
    assert_eq!((g.births, g.deaths), (5, 21), "the season he was looking at");
    assert_eq!(g.net(), -16, "and the number he could not account for");

    // Fully tended, the same herd in the same season grows. So the loss is the
    // staffing and not the pasture, the crowding or the winter cull.
    let tended = herd_growth(T, c.herd, c.fields_cattle, 80 * 3, c.herd_crowding, WINTER);
    assert!(tended.net() > g.net(), "three a head turns it round: {tended:?} against {g:?}");

    let e = herd_labour_estimate(T, &c, WINTER);
    assert!(
        e.wanted > c.labour[JOB_CATTLE_FARMING],
        "break-even wants {} milkmaids and he had {}",
        e.wanted,
        c.labour[JOB_CATTLE_FARMING]
    );
    assert!(
        e.wanted <= c.population,
        "and the search never asks for more people than the county has"
    );

    // What the interface does with it, which is the whole point of the number.
    c.labour_wanted[JOB_CATTLE_FARMING] = e.wanted;
    c.labour_useful[JOB_CATTLE_FARMING] = e.useful;
    assert!(
        c.labour[JOB_CATTLE_FARMING] < c.labour_wanted[JOB_CATTLE_FARMING],
        "which is the test `Panel_JobDetail`, the produce icon and Village_RebuildIcons make"
    );
    assert_eq!(
        c.minimap_bands().labour,
        0,
        "and the minimap's labour overlay paints the shortage band"
    );
}

/// **`Herd_SeasonTick`'s own tail writes both words**, which is a separate call
/// site from `County_RefreshEstimates` and was a separate omission.
///
/// It needs its own test and the reason is worth stating, because the obvious
/// test does not work: `Panels_RefreshAll` runs later in the same season and
/// rewrites the record, so deleting this write and running a whole season
/// leaves every number unchanged — measured, that ablation is **green** against
/// [`the_season_writes_the_break_even_floor_into_the_labour_record`]. Nothing in
/// the simulation reads the floor, so there is no downstream effect to catch it
/// either; `Labour_Allocate` reads the ceiling and only the ceiling.
///
/// So the pass is run **alone**. That is not a weaker test, it is the only one
/// that distinguishes the two call sites, and the write stays because
/// `Herd_LabourEstimate` is one function filling two words of one record and
/// writing half of it is the deviation.
///
/// **Ablation, run:** delete the `labour_wanted` line from
/// `Kingdom::herd_season_tick` — red here, green everywhere else in the suite.
#[test]
fn the_herds_own_season_tick_writes_the_break_even_floor() {
    let saves = l2_testkit::every_available_save();
    let Some(f) = saves.iter().find(|f| f.name == "england-turn1.sav") else {
        eprintln!("no england-turn1 fixture; skipping");
        return;
    };
    let s = Scenario::from_save(&f.save).expect("import");
    let mut k = s.kingdom(SEED);
    for id in 1..=k.county_count {
        k.counties[id].labour_wanted[JOB_CATTLE_FARMING] = LABOUR_NO_FLOOR;
    }

    let mut report = l2_kingdom::report::SeasonReport::new();
    k.run_pass(l2_kingdom::phase::Pass::HerdSeasonTick, &mut report);

    let mut written = 0;
    for id in 1..=k.county_count {
        if k.counties[id].pop_band == 0 {
            continue;
        }
        assert_ne!(
            k.counties[id].labour_wanted[JOB_CATTLE_FARMING], LABOUR_NO_FLOOR,
            "county {id}: `Herd_SeasonTick`'s tail left the floor unwritten"
        );
        assert_eq!(
            k.counties[id].labour_wanted[JOB_CATTLE_FARMING],
            herd_labour_estimate(T, &k.counties[id], k.season_next).wanted,
            "county {id}"
        );
        written += 1;
    }
    assert!(written >= 10, "only {written} counties had a floor written");
}

/// **And the end of the season leaves the right number in the record**, through
/// `Panels_RefreshAll`'s `County_RefreshEstimates`.
///
/// **Ablation, run:** delete the `labour_wanted` write from
/// `l2_kingdom::field::refresh_estimates` — red here.
#[test]
fn the_season_writes_the_break_even_floor_into_the_labour_record() {
    let saves = l2_testkit::every_available_save();
    if saves.is_empty() {
        eprintln!("no game install or fixture directory configured; skipping");
        return;
    }
    let Some(f) = saves.iter().find(|f| f.name == "england-turn1.sav") else {
        eprintln!("no england-turn1 fixture; skipping");
        return;
    };
    let s = Scenario::from_save(&f.save).expect("import");
    let mut k = s.kingdom(SEED);

    // Before anything runs, the file's own floors are already in the record —
    // the importer reads `+0xD4`. Clear them, so that what is asserted below is
    // this crate's arithmetic and not the file's bytes surviving.
    for id in 1..=k.county_count {
        k.counties[id].labour_wanted[JOB_CATTLE_FARMING] = LABOUR_NO_FLOOR;
    }
    k.advance_season();

    let mut written = 0;
    for id in 1..=k.county_count {
        let c = &k.counties[id];
        if c.pop_band == 0 {
            continue;
        }
        let expected = herd_labour_estimate(T, c, k.season_next).wanted;
        assert_eq!(
            c.labour_wanted[JOB_CATTLE_FARMING], expected,
            "county {id}'s floor after a season"
        );
        assert_ne!(
            c.labour_wanted[JOB_CATTLE_FARMING], LABOUR_NO_FLOOR,
            "county {id} kept the cleared sentinel, so no pass wrote it"
        );
        written += 1;
    }
    assert!(written >= 10, "only {written} counties had a floor written");
}

/// **An empty county asks for nobody**, which is the `[I]` arm of
/// [`herd_labour_estimate`]: the search loop never runs, so neither word is
/// ever assigned and the floor is whatever it was initialised to.
///
/// No save holds a county with no people, so the original's initial value is
/// unobserved. [`LABOUR_NO_FLOOR`] is chosen because it is what every other
/// job's estimate writes for *no requirement*, and because the alternative
/// paints a dead county's zero milkmaids red for ever.
#[test]
fn a_county_with_nobody_in_it_asks_for_no_milkmaids() {
    let mut c = County::new();
    c.population = 0;
    c.herd = 40;
    c.fields_cattle = 4;
    let e = herd_labour_estimate(T, &c, 1);
    assert_eq!(e.wanted, LABOUR_NO_FLOOR);
    assert_eq!(e.useful, l2_kingdom::county::LABOUR_UNSET, "the ceiling's own sentinel is the file's");
}
