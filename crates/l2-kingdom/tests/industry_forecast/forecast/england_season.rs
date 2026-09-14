#![allow(unused_imports)]
use super::*;
use super::row_ramp::*;
use super::refresh::*;
use super::advanced_farming::*;
use super::*;
use l2_kingdom::industry::MapToggle;
use l2_kingdom::tables::Commodity;
use l2_scenario::Scenario;
use l2_testkit::england;

/// **The forecast a real county carries after a real season.**
///
/// Four claims:
///
/// 1. every county's forecast is exactly `min(999, (workers / divisor) * 80 /
///    100)` for the three unmetered commodities, or zero when one of the four
///    guards fails;
/// 2. the guards are the original's, not ours — a county that is unowned, has
///    no `pop_band`, is switched off, has no seam or is counting down a
/// trampling forecasts **nothing**; statement so a stale number cannot survive;
/// the value arrived;
/// 3. at least one county forecasts a **non-zero** number, which is what says
/// the value arrived;
/// 4. and a forecast that *was* non-zero goes back to zero when the player
///    switches the industry off — because
///    `*(undefined4 *)(county * 0x300 + 0x53fc58 + industry * 0x18) = 0;` is
///    `Industry_LabourEstimate`'s **first** statement, outside every guard, so
/// a county that fails one of them forecasts nothing
///    last season's number.
///
/// **Ablation.** Deleting the
/// `crate::industry::preview(…)` call at the foot of
/// `field::refresh_estimates`'s industry loop turns claims 1 and 3 red — the
/// forecast stays at `Industry::new()`'s zero for every county on the map.
///
/// Deleting the `next_season = 0` line inside `preview` was **green** against
/// the first three claims: on England
/// turn one every guarded county starts at zero and stays there, so nothing in
/// the position can tell "written to zero" from "never written". Claim 4 is
/// what makes that ablation red, and it needs a county whose forecast is
/// non-zero *first* —
/// [`l2_kingdom::Kingdom::toggle_industry`], the map click's own road, rather
/// than clearing a flag by hand.
#[test]
fn a_season_of_england_writes_every_county_s_industry_forecast() {
    let save = england!();
    let scenario = Scenario::from_save(&save).expect("import");
    let mut kingdom = scenario.kingdom(1);

    // The fixture's own setting. Asserted
    // with *Advanced Farming* on, the ramp is a compounding
    // number and none of the arithmetic below would be right.
    assert!(
        !kingdom.options.advanced_farming,
        "England turn one has Advanced Farming off; the flat 80 is not the rule here"
    );

    kingdom.advance_season();

    let mut non_zero = 0;
    let mut checked = 0;
    for id in 1..=kingdom.county_count {
        let county = &kingdom.counties[id];
        for c in 0..4 {
            // Weapons is the one commodity whose `resourceLimit` is a real
            // quantity — the realm's wood and iron divided by a denominator
            // summed across every blacksmith in the realm — so its expected
            // value cannot be written without reproducing `FUN_0044F15B` here,
            // and reproducing it here is exactly the thing this file refuses to
            // do. It is covered by the crate's own weapon-share tests instead;
            // this asserts the three that take the literal 999.
            if c == 2 {
                continue;
            }
            let record = &county.industry[c];
            let workers = county.labour[JOB[c]].max(0);
            let expected = if county.owner == 0
                || county.pop_band == 0
                || !record.enabled
                || !record.has_resource
                || record.disabled_seasons != 0
            {
                0
            } else {
                let made = (workers / DIVISOR[c]) * FLAT_EFFICIENCY / 100;
                made.min(UNLIMITED)
            };
            assert_eq!(
                record.next_season, expected,
                "county {id} commodity {c}: {workers} workers in job {}, owner {}, \
                 enabled {}, seam {}, countdown {}",
                JOB[c], county.owner, record.enabled, record.has_resource, record.disabled_seasons
            );
            checked += 1;
            if record.next_season != 0 {
                non_zero += 1;
            }
        }
    }

    assert!(checked >= 3 * 14, "fourteen counties, three commodities each: {checked} checked");
    assert!(
        non_zero > 0,
        "every one of {checked} forecasts is zero — nothing wrote the tail, or every \
         county failed a guard"
    );

    // Claim 4. Wood is the industry the England position has switched on, so it
    // is the only one that can be switched *off* from a non-zero forecast.
    let id = (1..=kingdom.county_count)
        .find(|&id| kingdom.counties[id].industry[0].next_season != 0)
        .expect("`non_zero` above says at least one county forecasts something");
    let before = kingdom.counties[id].industry[0].next_season;
    let on = kingdom.toggle_industry(id, MapToggle::Industry(Commodity::Wood));
    assert!(!on, "county {id}'s forest was already off, so the toggle proved nothing");
    assert_eq!(
        kingdom.counties[id].industry[0].next_season, 0,
        "county {id} was forecasting {before} and the player switched the forest off; \
         `Industry_LabourEstimate` zeroes the word before its first guard, so a stale \
         number cannot survive a switch"
    );
}


