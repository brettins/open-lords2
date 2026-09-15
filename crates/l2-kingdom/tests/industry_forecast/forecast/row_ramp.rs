#![allow(unused_imports)]
use super::*;
use super::england_season::*;
use super::refresh::*;
use super::advanced_farming::*;
use super::*;
use l2_kingdom::industry::MapToggle;
use l2_kingdom::tables::Commodity;
use l2_scenario::Scenario;
use l2_testkit::england;

/// **Ablation.** Pass `county.labour[row.job]` to
/// `preview`'s `efficiency_ramp` — the repair a reader who thought `local_28`
/// was a slip would make — and this goes red at the equality.
#[test]
fn the_row_is_drawn_at_the_ramp_a_whole_population_would_earn_and_understates_because_of_it() {
    let save = england!();
    let scenario = Scenario::from_save(&save).expect("import");
    let mut kingdom = scenario.kingdom(1);
    kingdom.options.advanced_farming = true;

    const WOOD: usize = 0;
    const WOOD_BASE: i32 = 20;
    let Some(id) = (1..=kingdom.county_count).find(|&id| {
        let c = &kingdom.counties[id];
        c.owner != 0
            && c.industry[WOOD].has_resource
            && c.industry[WOOD].enabled
            && c.labour[JOB[WOOD]] > 0
    }) else {
        panic!("no county on the England map is cutting wood");
    };

    let (population, workers) = {
        let c = &kingdom.counties[id];
        (c.population, c.labour[JOB[WOOD]])
    };
    let capacity = (workers + population) / 2;
    let last = 50;
    {
        let c = &mut kingdom.counties[id];
        c.industry[WOOD].capacity = capacity;
        c.industry[WOOD].efficiency = last;
        // `FUN_0044F248` ramps from `+0x29C`, not from the `+0x294` above.
        c.industry[WOOD].last_efficiency = last;
    }

    // `County_RefreshEstimates` (`0x004485A5`) — the same entry point the season
    // pass and `Industry_ToggleFromMap` use.
    kingdom.refresh_estimates(id);

    // `Industry_EfficiencyRamp` (`FUN_0044F248`), written out here
    // called, so that ablating the crate's copy cannot move this expectation
    // with it.
    let ramp = |n: i32| {
        let mut increment = WOOD_BASE;
        if capacity < n {
            increment = WOOD_BASE * (capacity * 100 / n) / 100;
        }
        let mut eff = last + increment;
        if eff > 100 {
            eff = 100;
        }
        if eff < WOOD_BASE {
            eff = WOOD_BASE;
        }
        eff
    };
    let drawn = (workers / DIVISOR[WOOD]) * ramp(population) / 100;
    let honest = (workers / DIVISOR[WOOD]) * ramp(workers) / 100;
    assert!(
        drawn < honest,
        "county {id}: {workers} of {population} cutting wood at capacity {capacity} did not \
         separate the two ramps ({} against {}), so this test asserts nothing",
        ramp(population),
        ramp(workers)
    );
    assert_eq!(
        kingdom.counties[id].industry[WOOD].next_season,
        drawn.min(UNLIMITED),
        "county {id}: the row draws the whole-population ramp's number ({drawn}), which is \
         `docs/bugs.md` B97, and not the workforce's own ({honest})"
    );
}

