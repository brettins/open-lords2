#![allow(unused_imports)]
use super::*;
use super::migration::*;
use super::tests_part::*;
use crate::county::{ChangeReason, County, CHANGE_REASON_MIN_PCT, MAX_INFLOW_SOURCES};
use crate::math::pct;
use crate::tables::{Season, Tables};
use l2_net::{Quirk, Quirks};

pub fn update_one(t: &Tables, county: &mut County, season: Season, quirks: Quirks) {
    county.pop_last = county.population;

    let cap = pct(county.population, t.event.population_cap_pct);
    let base = t.birth_rate(county.population);
    let death = t.health[(county.health_band as usize).min(t.health.len() - 1)].death_rate
        + t.season[season.index() as usize].death_rate;
    let factor = t.happiness_birth_factor(county.happiness);
    // `local_c` in the decompilation: the ladder's rate **already scaled by
    // happiness**. It is this, not the ladder's own `base`, that both tests
    // below compare — `if ((births == 0) && (local_c != 0))` and
    // `if (local_c < iVar3)` — and C170 is the season of births
    // and deaths we got one wrong in either direction by comparing `base`.
    let rate = pct(base, factor);

    let mut births = pct(county.population, rate);
    let mut deaths = pct(county.population, death);

    if births == 0 && rate != 0 {
        births = 1;
    }
    if deaths == 0 && death != 0 {
        deaths = 1;
    }
    if county.health_band == 0 {
        deaths += 2;
    }
    if rate < death {
        deaths += 1;
    } else {
        births += 1;
    }

    // The random-event swing — `Population_UpdateAll` (`0x00449EF3`), the
    // eighteen lines after the `+1` above. Only two handlers write `+0x1FB`:
    //
    // *Plague* (`FUN_00448F6F`, −20 … −40) and *Wedding fever* (`FUN_004491F0`,
    // +30 … +60). `[V]` — every write to the byte in the binary is one of those
    // eight or one of the two clears.
    //
    // **A percentage of the deaths or the births, not of the county.** This
    // read `Pct(population, pct)` until C169, which made a Winter
    // plague on 1,000 people in health band 2 at happiness 50 kill 200 extra
    //
    // ```c
    // county.+0x2F8 = 0;
    // if (pct < 0)      county.+0x2F8 = Pct(deaths, -pct) + 10;
    // else if (pct > 0) county.+0x2F8 = Pct(births,  pct) + 10;
    // if (cap < county.+0x2F8) county.+0x2F8 = cap;
    // if (pct < 0) deaths += county.+0x2F8; else if (pct > 0) births += county.+0x2F8;
    // ```
    let p = county.event_population_pct;
    county.event_population_swing = 0;
    if p < 0 {
        county.event_population_swing = pct(deaths, -p) + EVENT_SWING_FLOOR;
    } else if p > 0 {
        county.event_population_swing = pct(births, p) + EVENT_SWING_FLOOR;
    }
    if cap < county.event_population_swing {
        county.event_population_swing = cap;
    }
    if p < 0 {
        deaths += county.event_population_swing;
    } else if p > 0 {
        births += county.event_population_swing;
    }
    county.event_population_pct = 0;

    county.population += births - deaths;
    if county.population < 1 {
        births = 0;
        deaths = if quirks.reproduces(Quirk::ExtinctCountyRecordsNegativeDeaths) {
            county.population
        } else {
            county.pop_last
        };
        county.population = 0;
    }

    county.army = 0;

    county.population -= county.emigrants;
    county.population += county.immigrants;

    county.births = births;
    county.deaths = deaths;
    county.pop_band = county.compute_pop_band();
    county.pop_change_pct = if county.pop_last != 0 {
        (county.population - county.pop_last).abs() * 100 / county.pop_last
    } else {
        0
    };
    county.change_reason = change_reason(county);
}

/// `L2.eng` group 65's *"Births / Deaths / Emigration / Immigration"* line,
/// suppressed below a 6% change (`docs/kingdom.md` §1.2, county `+0x5B`).
///
/// **`[I]` on the selection.** The document gives the five values, the ordering
/// and the 6% suppression, and does not say how one of the four causes is
/// picked when several moved. The largest contributor, ties broken by the
/// enumeration order, is the only choice that makes the field mean anything.
fn change_reason(county: &County) -> ChangeReason {
    if county.pop_change_pct < CHANGE_REASON_MIN_PCT {
        return ChangeReason::None;
    }
    let candidates = [
        (county.births, ChangeReason::Births),
        (county.deaths.abs(), ChangeReason::Deaths),
        (county.emigrants, ChangeReason::Emigration),
        (county.immigrants, ChangeReason::Immigration),
    ];
    let mut best = ChangeReason::None;
    let mut best_size = 0;
    for (size, reason) in candidates {
        if size > best_size {
            best_size = size;
            best = reason;
        }
    }
    best
}

pub fn update_all(
    t: &Tables,
    counties: &mut [County],
    county_count: usize,
    season: Season,
    quirks: Quirks,
) {
    for id in 1..=county_count {
        update_one(t, &mut counties[id], season, quirks);
    }
}

