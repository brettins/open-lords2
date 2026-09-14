#![allow(unused_imports)]
use super::*;
use super::industry_and_labor::*;
use super::forecasts_and_events::*;
use super::*;
use super::county::*;
use super::units::*;
use l2_formats::save::{Layout, Save, COUNTY_BASE, COUNTY_STRIDE};
use l2_scenario::{ImportError, Scenario, STARTING_HEALTH_METER};
use l2_testkit::{saves, SaveFile};

/// **The field counts we derive are the counts every reachable save stores.**
///
/// The importer no longer carries `+0x1FF`, `+0x200` and `+0x201` across: it
/// reads the twenty tiles in `g_countyFieldTiles`, applies
/// `County_RecountFields`' terrain ladder to the tile planes, and lets the
/// kingdom hold what that makes. `CountyState` still carries what the file
/// said, so the two are independent readings of the same thing and this diffs
/// them.
///
/// It runs over **every** save the machine can offer
/// named fixture, because `docs/decisions.md` C26 is what happens when a rule
/// is checked against one value of its input — and the battle saves are a
/// different map with four counties, which is exactly the second value.
#[test]
fn the_field_counts_are_derived_and_they_match_every_save_that_stores_them() {
    let saves = saves!();
    let mut counties_checked = 0;
    for SaveFile { name, save, .. } in &saves {
        let Ok(scenario) = Scenario::from_save(save) else { continue };
        let kingdom = scenario.kingdom(1);
        for id in scenario.county_ids() {
            let Some(stored) = &scenario.counties[id] else { continue };
            let ours = &kingdom.counties[id];
            counties_checked += 1;
            assert_eq!(
                (ours.fields_fallow, ours.fields_cattle, ours.fields_grain),
                (stored.fields_fallow, stored.fields_cattle, stored.fields_grain),
                "{name} county {id}: recounting its {} field tiles disagrees with the file",
                ours.field_slots_used()
            );
        }
    }
    assert!(counties_checked > 14, "only {counties_checked} counties reached");
}

// --- the campaign layer -----------------------------------------------------

/// `Pct` — `Tax_RecomputePreview`'s own rounding, which is truncation.
fn pct(v: i32, p: i32) -> i32 {
    v * p / 100
}

/// **`g_castleTaxBase`, written out.** The multiplier for
/// castle types 0 … 5, immediates in `Tax_CollectAll`'s instruction stream
/// (`docs/kingdom.md` §10). Spelled here so that the assertion below does not
/// compute its expected value from the table it is checking — the trap
/// `docs/agents.md` records as *ablating a constant while computing your probe
/// from that same constant*.
const CASTLE_TAX_BASE: [i32; 6] = [320, 480, 560, 640, 720, 800];

/// The one moment on this machine where `+0xC0` is **not** the current
/// population's answer, named with its reason.
///
/// It is the middle save of the battle triple
/// the explanation: `battle-during.sav` (the install calls the same game
/// `incombat.sav`) is taken with a battle open. County 2's population has
/// already fallen to 588 and the stored preview is still **245**
/// `Pct(Pct(638, 480), 8)` — the answer for the population the county had
/// before the fighting. `battle-after.sav` stores **225** for the same county,
/// which *is* `Pct(Pct(588, 480), 8)`.
///
/// recomputing it on load.** No recompute can produce 245; the original
/// restores a memory image, and `Tax_RecomputePreview` runs on a control or at
/// the end of a season, not on a load.
const PREVIEW_NOT_YET_REFRESHED: &[&str] = &["battle-during.sav", "incombat.sav"];

/// **`+0x0F` is `5 - taxRate` in every owned county of every save**, which is
/// `Tax_RecomputePreview` (`0x0044B80B`)'s second statement and is what says
/// the offset is the right one. It is a different field from `+0x0E`, which
/// carries the realm's empire term as well.
#[test]
fn the_local_tax_happiness_byte_is_five_minus_the_rate_in_every_save() {
    let mut checked = 0usize;
    for f in l2_testkit::saves!() {
        let s = &f.save;
        for id in 1..17usize {
            let base = COUNTY_BASE + (id * COUNTY_STRIDE) as u32;
            let Ok(owner) = s.u8_at(base + 0x05) else { continue };
            if owner == 0 {
                continue;
            }
            let rate = s.u8_at(base + 0xB9).unwrap() as i32;
            let local = s.i8_at(base + 0x0F).unwrap() as i32;
            assert_eq!(
                local,
                5 - rate,
                "{}: county {id} stores +0x0F = {local} at rate {rate}",
                f.label()
            );
            checked += 1;
        }
    }
    assert!(checked >= 5, "only {checked} counties were reached");
}

/// **The tax panel's *People pay* line, against the original's own answer.**
///
/// `+0xC0` is `Pct(Pct(population, castleBase), taxRate)` — the third statement
/// of `Tax_RecomputePreview` — and the saves on this machine carry rates 2, 3,
/// 6 and 8, so the arithmetic can be checked against a number the original
/// wrote. `docs/plan.md` §2.5 says every county in
/// every fixture sits at rate 0; that is true of the England fixture and false
/// of the turn pair and the six siege saves.
///
/// It is still true of any rate above 19, where `g_taxHappinessOther` starts to
/// bite — so this promotes the *preview*, not the empire term.
#[test]
fn the_tax_preview_byte_is_the_arithmetic_we_implement() {
    let mut agreed = 0usize;
    for f in l2_testkit::saves!() {
        if PREVIEW_NOT_YET_REFRESHED.contains(&f.name.as_str()) {
            continue;
        }
        let s = &f.save;
        for id in 1..17usize {
            let base = COUNTY_BASE + (id * COUNTY_STRIDE) as u32;
            let Ok(owner) = s.u8_at(base + 0x05) else { continue };
            if owner == 0 {
                continue;
            }
            let rate = s.u8_at(base + 0xB9).unwrap() as i32;
            if rate == 0 {
                continue;
            }
            let pop = s.i32_at(base + 0x24).unwrap();
            let castle = s.u8_at(base + 0x1C0).unwrap() as usize;
            let shown = s.i32_at(base + 0xC0).unwrap();
            let base_mult = CASTLE_TAX_BASE[castle.min(5)];
            assert_eq!(
                pct(pct(pop, base_mult), rate),
                shown,
                "{}: county {id}, {pop} people at rate {rate} behind castle {castle}",
                f.label()
            );
            agreed += 1;
        }
    }
    if agreed == 0 {
        l2_testkit::skip!(
            "no reachable save carries a county at a non-zero tax rate, so there is \
             nothing to check the preview against"
        );
    }
}

// ------------------------------------ what docs/stored-fields.json found dropped

/// **`+0x18` is `+0x1C / g_turnCount` in every county of every save** —
/// `Happiness_UpdateAll` (`0x0044BAEA`) banks the season's happiness into the sum
/// and divides by the turn count, into a signed byte.
///
/// The importer used to set both to this season's happiness, which this test
/// measures the cost of: it counts the counties where the stored average is not
/// the current happiness, which is every county past turn one whose mood has
/// moved — `siege-aftersie.sav` county 2 stores 54 and is at 95 today.
#[test]
fn the_happiness_average_is_the_running_sum_over_the_turn_count_in_every_save() {
    let (mut checked, mut differs) = (0usize, 0usize);
    for f in l2_testkit::saves!() {
        let s = &f.save;
        let turns = s.globals().unwrap().turn_count;
        assert!(turns > 0, "{}: turn count {turns}", f.label());
        for id in 1..17usize {
            let base = COUNTY_BASE + (id * COUNTY_STRIDE) as u32;
            if s.i32_at(base + 0x24).unwrap() == 0 {
                continue;
            }
            let avg = s.i8_at(base + 0x18).unwrap() as i32;
            let sum = s.i32_at(base + 0x1C).unwrap();
            assert_eq!(avg, (sum / turns) as i8 as i32, "{}: county {id}, sum {sum} over {turns} turns", f.label());
            checked += 1;
            differs += usize::from(avg != s.i8_at(base + 0x0C).unwrap() as i32);
        }
    }
    assert!(checked >= 14, "only {checked} counties reached");
    assert!(differs > 0, "the average equals the current happiness everywhere, so the old import passed too");
}

/// **`+0x5B` is set exactly where `+0x2C` reaches 6** — `Population_UpdateAll`
/// writes the change percentage and then attributes it only when it is at least
/// six (`docs/kingdom.md` §1.2). Both bytes are written in the same pass, so the
/// relation holds whatever happened to the population afterwards, so
/// this does not also check the percentage against the population.
///
/// The six is written as a literal
/// `l2_kingdom::county::CHANGE_REASON_MIN_PCT`: a probe computed from the constant
/// under test is `docs/agents.md`'s first way to ablate wrongly.
#[test]
fn a_population_change_reason_is_recorded_exactly_where_the_change_reaches_six_percent() {
    let (mut checked, mut reasons) = (0usize, 0usize);
    for f in l2_testkit::saves!() {
        let s = &f.save;
        for id in 1..17usize {
            let base = COUNTY_BASE + (id * COUNTY_STRIDE) as u32;
            if s.i32_at(base + 0x24).unwrap() == 0 {
                continue;
            }
            let pct = s.i32_at(base + 0x2C).unwrap();
            let reason = s.u8_at(base + 0x5B).unwrap();
            assert_eq!(reason != 0, pct >= 6, "{}: county {id}, change {pct}%, reason {reason}", f.label());
            assert!(reason <= 4, "{}: county {id}, reason {reason} names no L2.eng group 65 string", f.label());
            checked += 1;
            reasons += usize::from(reason != 0);
        }
    }
    assert!(checked >= 14 && reasons > 0, "{checked} counties, {reasons} with a reason");
}

/// **`+0x21` is set only in a county below thirty happiness**, which is what
/// identifies it as `Unrest_UpdateAll`'s warning latch (`0x0044AA41`: set when
/// happiness is under `0x1E` and the byte is clear) — the flag
/// `l2_kingdom::county::County::unrest_warned` described without an offset.
#[test]
fn the_unrest_warning_latch_is_set_only_in_a_county_below_thirty_happiness() {
    let mut set = 0usize;
    for f in l2_testkit::saves!() {
        let s = &f.save;
        for id in 1..17usize {
            let base = COUNTY_BASE + (id * COUNTY_STRIDE) as u32;
            if s.u8_at(base + 0x21).unwrap() == 0 {
                continue;
            }
            let happiness = s.i8_at(base + 0x0C).unwrap();
            assert!(happiness < 30, "{}: county {id} is warned at happiness {happiness}", f.label());
            set += 1;
        }
    }
    assert!(set > 0, "no save carries the latch, so nothing identified it");
}


