#![allow(unused_imports)]
use super::*;
use super::england_season::*;
use super::row_ramp::*;
use super::advanced_farming::*;
use super::*;
use l2_kingdom::industry::MapToggle;
use l2_kingdom::tables::Commodity;
use l2_scenario::Scenario;
use l2_testkit::england;

/// **`Industry_LabourEstimate`'s efficiency write-back** (`0x0044F318`), the
/// ramp C136 left unported and C180 named as its first open item.
///
/// ```c
/// iVar3 = Industry_EfficiencyRamp(county, industry, labour[slot].workers, base);
/// (&DAT_0053fc44)[industry * 0x18 + county * 0x300] = (char)iVar3;   /* +0x294 */
/// ```
///
/// C136 held the write back because `County_RefreshEstimates` runs four times
/// inside `Industry_ToggleFromMap` alone, and a compounding ramp would climb
/// four steps on one click. It does not compound: `Industry_EfficiencyRamp`
/// (`0x0044F248`) reads county **`+0x29C`** and the write-back writes
/// **`+0x294`**, and only `Industry_Produce` (`0x0044EA92`) copies the one into
/// the other — `(&DAT_0053fc4c)[...] = (&DAT_0053fc44)[...];`, the line after
/// its own ramp.
///
/// **Ablation.** Delete the write-back in `industry::preview` and the first
/// assertion goes red at the base against the ramped number.
#[test]
fn the_refresh_ramps_the_efficiency_and_four_refreshes_land_on_one_answer() {
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

    let workers = kingdom.counties[id].labour[JOB[WOOD]];
    let capacity = workers / 3;
    {
        let c = &mut kingdom.counties[id];
        c.industry[WOOD].capacity = capacity;
        c.industry[WOOD].efficiency = WOOD_BASE;
        c.industry[WOOD].last_efficiency = WOOD_BASE;
    }
    // `FUN_0044F248` written out, not called: the base scaled by
    // capacity/workers, added to `+0x29C`, capped at 100, floored at the base.
    let increment = WOOD_BASE * (capacity * 100 / workers) / 100;
    let ramped = (WOOD_BASE + increment).clamp(WOOD_BASE, 100);
    assert!(
        ramped > WOOD_BASE && ramped < 100,
        "county {id}: {workers} cutting wood at capacity {capacity} ramps to {ramped}, which \
         separates nothing"
    );

    kingdom.refresh_estimates(id);
    assert_eq!(
        kingdom.counties[id].industry[WOOD].efficiency, ramped,
        "county {id}: the estimate pass did not write `+0x294`"
    );
    for _ in 0..3 {
        kingdom.refresh_estimates(id);
    }
    assert_eq!(
        kingdom.counties[id].industry[WOOD].efficiency, ramped,
        "county {id}: four refreshes compounded, so the ramp is reading `+0x294`, not `+0x29C`"
    );
    assert_eq!(
        kingdom.counties[id].industry[WOOD].last_efficiency, WOOD_BASE,
        "county {id}: the estimate pass wrote the ramp's own input"
    );

    // The season is the one pass that advances `+0x29C`, and it ramps from the
    // same 20 the refreshes did — so it lands on the same number and copies it.
    kingdom.advance_season();
    let after = kingdom.counties[id].industry[WOOD];
    assert_eq!(
        after.last_efficiency, ramped,
        "county {id}: `Industry_Produce` (`0x0044EA92`) copies its own ramp into `+0x29C`"
    );
    // `Industry_ProduceAll` (`0x0044E852`) writes the capacity from the job's
    // workers and *then* runs the four estimates, so the season closes with
    // `+0x294` a full increment above `+0x29C`.
    assert!(
        after.efficiency > after.last_efficiency,
        "county {id}: the estimate pass that closes the season did not re-ramp at the capacity \
         the season had just written ({} against {})",
        after.efficiency,
        after.last_efficiency
    );
}

