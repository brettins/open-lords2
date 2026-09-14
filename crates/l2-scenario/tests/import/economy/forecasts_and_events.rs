#![allow(unused_imports)]
use super::*;
use super::industry_and_labor::*;
use super::tax_and_happiness::*;
use super::*;
use super::county::*;
use super::units::*;
use l2_formats::save::{Layout, Save, COUNTY_BASE, COUNTY_STRIDE};
use l2_scenario::{ImportError, Scenario, STARTING_HEALTH_METER};
use l2_testkit::{saves, SaveFile};

/// **County `+0x2F8` — the figure *Plague* and *Wedding fever*'s letters print —
/// reaches a loaded game**, measured on a patched copy of a real save because
/// the corpus cannot measure it.
///
/// Every county of every save on this machine stores **zero** here, so
/// `tests/stored_fields.rs` compares the row with zero and nothing else — and
/// `County::new()` is zero too, so deleting the importer's assignment leaves that
/// check green. Ablated: it did. The saves are not silent about events: county 3
/// holds Wedding fever's id `0x8E` in four consecutive siege saves, turns 12 to
/// 14, beside a zero figure. That id is **stale**, not this season's —
/// `Event_RollAll` (`0x00448819`) clears the three swing bytes and the tax gate
/// every season and never `eventId` — and the saved births reproduce with no swing
/// in them (`docs/decisions.md` C169).
///
/// So this writes a figure into one county's bytes, reopens the file through the
/// executable's own block table, and asks the import for it back.
#[test]
fn the_plague_letters_figure_survives_a_load() {
    let Some(exe) = l2_testkit::executable() else {
        l2_testkit::skip!("no Lords2.exe to take the save layout from");
    };
    let saves = l2_testkit::saves!();
    let f = saves.first().expect("saves! skips when there are none");
    let mut bytes = std::fs::read(&f.path).expect("the save that just opened");

    let county = 1usize;
    let va = COUNTY_BASE + (county * COUNTY_STRIDE) as u32 + 0x2F8;
    let stored = f.save.i32_at(va).unwrap();
    let figure = stored + 74; // a Winter plague on 1,000 people in band 2
    let at = f.save.layout().offset_of(va).expect("+0x2F8 is inside the county block");
    bytes[at..at + 4].copy_from_slice(&figure.to_le_bytes());

    let patched = l2_formats::save::Save::open(&exe, &bytes).expect("the patched save opens");
    assert_eq!(patched.i32_at(va).unwrap(), figure, "the patch landed on +0x2F8");
    let k = l2_scenario::Scenario::from_save(&patched)
        .unwrap_or_else(|e| panic!("{}: the import refused: {e}", f.label()))
        .kingdom(1);
    assert_eq!(
        k.counties[county].event_population_swing, figure,
        "{}: county {county}'s +0x2F8 holds {figure} and the loaded game does not carry it",
        f.label()
    );
}

/// **`+0x258` is `+0x268 − +0x26C − herdEaten` in every county of every save** —
/// the cattle row's *"Overall change"*, calf births expected, cow deaths expected,
/// and what the people ate (`docs/decisions.md` C128 for why the eating is in it).
///
/// Three fields that were dropped together by the importer, and one relation
/// that pins all three offsets at once: a wrong offset for any of them would have
/// to land on a word that happens to close this sum in every county of every
/// save. It is not vacuous — the England turn-one fixture's ten neutral counties
/// eat thirteen head each, so the eating term is exercised, and births differ
/// from deaths almost everywhere.
///
/// `tests/stored_fields.rs` is what holds the kingdom to these bytes; this is
/// what says the bytes are the fields.
#[test]
fn every_saved_cattle_forecast_is_births_less_deaths_less_what_was_eaten() {
    let (mut checked, mut eaten_counted) = (0usize, 0usize);
    for f in l2_testkit::saves!() {
        let s = &f.save;
        for id in 1..17usize {
            let base = COUNTY_BASE + (id * COUNTY_STRIDE) as u32;
            if s.i32_at(base + 0x24).unwrap() == 0 {
                continue;
            }
            let change = s.i32_at(base + 0x258).unwrap();
            let births = s.i32_at(base + 0x268).unwrap();
            let deaths = s.i32_at(base + 0x26C).unwrap();
            let eaten = s.i32_at(base + 0x17C).unwrap();
            assert_eq!(
                change,
                births - deaths - eaten,
                "{}: county {id} stores +0x258 {change}, and +0x268 {births} − +0x26C {deaths} − \
                 +0x17C {eaten} is {}",
                f.label(),
                births - deaths - eaten
            );
            checked += 1;
            eaten_counted += usize::from(eaten != 0);
        }
    }
    assert!(checked >= 14, "only {checked} counties reached");
    assert!(eaten_counted > 0, "no county anywhere ate cattle, so the eating term was never tested");
}

/// **`+0x22C` is `Grain_LabourEstimate`'s tail in every county of every save**:
/// `−sown − eaten` when the season is Spring, `harvest − eaten` in Winter,
/// `−eaten` otherwise, with `+0x230` the sowing and `crop[2]` the harvest.
///
/// **What the corpus can and cannot settle, said beside the assertion.** Every
/// save on this machine stores `+0x230 == 0` and `crop[2] == 0` in every county,
/// so the sowing and harvest arms are only checked at zero
/// `season` argument — `l2_kingdom::land::grain_preview` reads it as next season
/// — cannot be told from this season here. What *is* settled is that `+0x22C`
/// is minus the county's grain eaten wherever those arms are empty, in five
/// counties where that is not zero (`old_turn.sav` county 2 stores −73).
#[test]
fn every_saved_grain_forecast_is_its_seasons_tail() {
    let (mut checked, mut non_zero, mut arms) = (0usize, 0usize, 0usize);
    for f in l2_testkit::saves!() {
        let s = &f.save;
        let season_next = s.globals().unwrap().season_next;
        for id in 1..17usize {
            let base = COUNTY_BASE + (id * COUNTY_STRIDE) as u32;
            if s.i32_at(base + 0x24).unwrap() == 0 {
                continue;
            }
            let change = s.i32_at(base + 0x22C).unwrap();
            let sown = s.i32_at(base + 0x230).unwrap();
            let harvest = s.i32_at(base + 0x248).unwrap();
            let eaten = s.i32_at(base + 0x178).unwrap();
            let tail = match season_next {
                1 => -sown - eaten,
                4 => harvest - eaten,
                _ => -eaten,
            };
            assert_eq!(
                change,
                tail,
                "{}: county {id} stores +0x22C {change}; next season {season_next}, sown {sown}, \
                 harvest {harvest}, eaten {eaten}",
                f.label()
            );
            checked += 1;
            non_zero += usize::from(change != 0);
            arms += usize::from(sown != 0 || harvest != 0);
        }
    }
    assert!(checked >= 14, "only {checked} counties reached");
    assert!(non_zero > 0, "every stored grain forecast is zero, so nothing was compared");
    eprintln!("{checked} grain forecasts, {non_zero} non-zero, {arms} with a sowing or a harvest");
}

