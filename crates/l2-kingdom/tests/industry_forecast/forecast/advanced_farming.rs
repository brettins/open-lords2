#![allow(unused_imports)]
use super::*;
use super::england_season::*;
use super::row_ramp::*;
use super::refresh::*;
use super::*;
use l2_kingdom::industry::MapToggle;
use l2_kingdom::tables::Commodity;
use l2_scenario::Scenario;
use l2_testkit::england;

/// The same write with *Advanced Farming* **off**: the flat 80, whatever the
/// staffing, the capacity or the number of refreshes. `FUN_0044F248`'s first
/// statement is why the ramp is invisible in every save on this machine.
///
/// **Ablation.** The same deletion leaves the doctored base and this goes red.
#[test]
fn with_advanced_farming_off_the_write_back_is_the_flat_eighty() {
    let save = england!();
    let scenario = Scenario::from_save(&save).expect("import");
    let mut kingdom = scenario.kingdom(1);
    assert!(!kingdom.options.advanced_farming, "the fixture has the option off");

    const WOOD: usize = 0;
    let Some(id) = (1..=kingdom.county_count).find(|&id| {
        let c = &kingdom.counties[id];
        c.owner != 0 && c.industry[WOOD].enabled && c.labour[JOB[WOOD]] > 0
    }) else {
        panic!("no county on the England map is cutting wood");
    };
    // Doctored down from the 80 the file stores, so the assertion is on the
    // write and not on what the importer carried.
    kingdom.counties[id].industry[WOOD].efficiency = 20;
    kingdom.counties[id].industry[WOOD].last_efficiency = 20;
    kingdom.counties[id].industry[WOOD].capacity = kingdom.counties[id].labour[JOB[WOOD]] / 3;
    for _ in 0..4 {
        kingdom.refresh_estimates(id);
    }
    assert_eq!(
        kingdom.counties[id].industry[WOOD].efficiency, FLAT_EFFICIENCY,
        "county {id}: the flat 80 is what the option-off ramp returns for every staffing"
    );
}


