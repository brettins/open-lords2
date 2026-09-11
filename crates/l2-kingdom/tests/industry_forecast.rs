//! **The industry rows' forecast, by the road the game travels.**
//!
//! ```text
//! LORDS2_FIXTURES="E:\dev\lords2-fixtures" cargo test -p l2-kingdom --test industry_forecast
//! ```
//!
//! `l2_kingdom::county::Industry::next_season` is county `+0x2A8 + c*0x18` —
//! `Industry_LabourEstimate`'s (`0x0044F318`) tail, the four bytes the sidebar's
//! industry rows draw a `Ui_DrawDelta` of. It is written by
//! [`l2_kingdom::industry::preview`], and the only thing that calls that is
//! [`l2_kingdom::field::refresh_estimates`], which is the estimate round the
//! season pass runs.
//!
//! # Why the road and not a hand-built county
//!
//! `docs/decisions.md` C30 and `docs/agents.md`'s *a test that drives the
//! picture from the wrong field passes for ever*: **a field is only tested if
//! something a test reads was written by something the game runs.** The village
//! screen had eleven green tests over four industry records that no importer
//! had ever filled. So this loads the England turn-one fixture through
//! `l2_scenario`, ends a season with [`l2_kingdom::Kingdom::advance_season`],
//! and reads the number back out of the counties the importer built.
//!
//! # Why every constant here is a literal
//!
//! `docs/agents.md`, *how to ablate wrongly*, one: **ablating a constant while
//! computing your probe from that same constant tests nothing at all.** So the
//! expected value below is not `pct(workers / t.commodity[c].divisor, …)` —
//! nothing in this file reads `Tables`, `efficiency_ramp`, `resource_limit` or
//! `pct`. The four divisors, the four job slots, the flat efficiency and the
//! unlimited limit are typed out as the numbers the decompilation has, and the
//! arithmetic is written out longhand.

use l2_kingdom::industry::MapToggle;
use l2_kingdom::tables::Commodity;
use l2_scenario::Scenario;
use l2_testkit::england;

/// `Industry_Produce`'s divisor column, in commodity order — wood, iron,
/// weapons, stone. `docs/kingdom.md` §7.4, and `l2_kingdom::industry`'s module
/// table. **Weapons is 4 and stone is 2**, which is the whole reason a divisor
/// column exists.
const DIVISOR: [i32; 4] = [1, 1, 4, 2];

/// The labour ladder slot each industry draws its workers from — 6 wood
/// cutting, 4 iron mining, 7 blacksmith, 5 stone quarrying. County
/// `+0xC4 + job*0x0C`.
const JOB: [usize; 4] = [6, 4, 7, 5];

/// `if (g_optAdvancedFarming == 0) return 80;` — `FUN_0044F248`'s first
/// statement, and **the whole of the ramp in the shipped game**: England turn
/// one has the option off, so every industry sits at a flat 80 whatever its
/// staffing or its base.
const FLAT_EFFICIENCY: i32 = 80;

/// `FUN_0044EF4E`'s literal for wood, iron and stone once the three flags hold.
const UNLIMITED: i32 = 999;

/// **The forecast a real county carries after a real season.**
///
/// Four claims, and the last two are the ones that needed the road:
///
/// 1. every county's forecast is exactly `min(999, (workers / divisor) * 80 /
///    100)` for the three unmetered commodities, or zero when one of the four
///    guards fails;
/// 2. the guards are the original's, not ours — a county that is unowned, has
///    no `pop_band`, is switched off, has no seam or is counting down a
///    trampling forecasts **nothing**, and the write is the function's *first*
///    statement so a stale number cannot survive;
/// 3. at least one county forecasts a **non-zero** number, which is what says
///    the value arrived rather than that every branch returned early;
/// 4. and a forecast that *was* non-zero goes back to zero when the player
///    switches the industry off — because
///    `*(undefined4 *)(county * 0x300 + 0x53fc58 + industry * 0x18) = 0;` is
///    `Industry_LabourEstimate`'s **first** statement, outside every guard, so
///    a county that fails one of them forecasts nothing rather than keeping
///    last season's number.
///
/// **Ablation, and the reason claim 4 exists.** Deleting the
/// `crate::industry::preview(…)` call at the foot of
/// `field::refresh_estimates`'s industry loop turns claims 1 and 3 red — the
/// forecast stays at `Industry::new()`'s zero for every county on the map.
///
/// Deleting the `next_season = 0` line inside `preview` was **green** against
/// the first three claims, and that is not a fact about the line: on England
/// turn one every guarded county starts at zero and stays there, so nothing in
/// the position can tell "written to zero" from "never written". Claim 4 is
/// what makes that ablation red, and it needs a county whose forecast is
/// non-zero *first* — which is why it goes through
/// [`l2_kingdom::Kingdom::toggle_industry`], the map click's own road, rather
/// than clearing a flag by hand.
#[test]
fn a_season_of_england_writes_every_county_s_industry_forecast() {
    let save = england!();
    let scenario = Scenario::from_save(&save).expect("import");
    let mut kingdom = scenario.kingdom(1);

    // The fixture's own setting, and the premise of `FLAT_EFFICIENCY`. Asserted
    // rather than assumed: with *Advanced Farming* on, the ramp is a compounding
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


/// **`B97`: the row is multiplied by the ramp at the county's whole
/// population and not at the workforce actually digging.** The defect, asserted.
///
/// `Industry_LabourEstimate` computes the ramp twice and uses the wrong one:
///
/// ```c
/// for (w = 0; w < population + popBand; w += popBand) {
///     n = min(w, population);
///     local_28 = Industry_EfficiencyRamp(county, industry, n, base);
///     …
/// }
/// iVar3 = Industry_EfficiencyRamp(county, industry, labour[slot].workers, base);
/// county.industry[industry].efficiency = (char)iVar3;      /* the real one, written */
/// made = Pct(labour[slot].workers / divisor, local_28);    /* the loop's, drawn */
/// ```
///
/// `local_28` survives the loop, and the loop's last trial is `n == population`
/// for every population and every band — the step is `popBand` and the guard is
/// `w < population + popBand`, so the final `w` is at least `population` and
/// `min` pins it there.
///
/// # The fixture cannot show it, and that is why the county is doctored
///
/// Three things all have to hold before the two ramps differ at all:
/// *Advanced Farming* on (England turn one has it off, and the ramp is then a
/// flat 80 whatever `n` is), a **non-zero** `capacity` (at zero the scaled
/// increment is zero for every `n` and the floor pins both to `base`), and
/// `capacity` between the staffing and the population. The England position has
/// `capacity == 0` in all fourteen counties, so this sets the three fields and
/// says so rather than pretending a fixture reached them. Everything else —
/// the population, the miners, the seam, the switch — is the importer's.
///
/// # The direction, which the first draft of `docs/bugs.md` had backwards
///
/// `Industry_EfficiencyRamp` scales its increment by `capacity * 100 / n`, so it
/// is **non-increasing in `n`**: a larger trial earns a *smaller* efficiency.
/// `workers <= population` always, so `ramp(population) <= ramp(workers)` and
/// the row **understates**. `docs/bugs.md` B97 called it optimistic;
/// the assertion below is `<`, and it would be red if the word were right.
///
/// **Ablation.** Pass `county.labour[row.job]` instead of `county.population` to
/// `preview`'s `efficiency_ramp` — the repair a reader who thought `local_28`
/// was a slip would make — and this goes red at the equality.
#[test]
fn the_row_is_drawn_at_the_ramp_a_whole_population_would_earn_and_understates_because_of_it() {
    let save = england!();
    let scenario = Scenario::from_save(&save).expect("import");
    let mut kingdom = scenario.kingdom(1);
    kingdom.options.advanced_farming = true;

    // Wood: job 6, divisor 1, base efficiency 20. It is the one industry the
    // England position has switched on anywhere, which is why the defect is
    // demonstrated on a forest and not on a mine.
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
    // Between the two, so `capacity < n` holds for the population's trial and
    // not for the workforce's — the only window in which the ramps differ.
    let capacity = (workers + population) / 2;
    let last = 50;
    {
        let c = &mut kingdom.counties[id];
        c.industry[WOOD].capacity = capacity;
        c.industry[WOOD].efficiency = last;
    }

    // `County_RefreshEstimates` (`0x004485A5`) — the same entry point the season
    // pass and `Industry_ToggleFromMap` use.
    kingdom.refresh_estimates(id);

    // `Industry_EfficiencyRamp` (`FUN_0044F248`), written out here rather than
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
