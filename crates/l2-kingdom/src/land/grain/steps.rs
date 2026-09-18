#![allow(unused_imports)]
use super::*;
use super::factors::*;
use super::actions::*;
use super::bands::*;
use super::display::*;
use super::*;

/// `Grain_Sow` (`0x0044CFE1`) chooses sacks-per-field by descending from 10 to
/// 1, taking the first value for which **both** `grainStore >= fields * sacks`
/// and `labour >= 12 * fields * sacks / divisor` hold.
pub fn sacks_per_field(
    t: &Tables,
    fields: i32,
    grain_store: i32,
    labour: i32,
    advanced_farming: bool,
) -> i32 {
    if fields <= 0 {
        return 0;
    }
    let divisor = sow_divisor(t, advanced_farming);
    let mut sacks = t.grain.max_sacks_per_field;
    while sacks >= 1 {
        let seed = fields * sacks;
        let work = t.grain.yield_per_sack * seed / divisor;
        if grain_store >= seed && labour >= work {
            return sacks;
        }
        sacks -= 1;
    }
    0
}

/// `Grain_Sow` in full (`0x0044CFE1`) — **the sacks that go into the
/// ground**, and the fallback that keeps a poor county from sowing nothing.
pub fn sow_sacks(
    t: &Tables,
    county: &mut County,
    grain_store: i32,
    labour: i32,
    advanced_farming: bool,
) -> i32 {
    let (sown, flag) =
        sow_seed(t, county.fields_grain, grain_store, labour, advanced_farming);
    if let Some(shortfall) = flag {
        county.sow_shortfall = shortfall;
    }
    sown
}

/// Returns the seed and what `Grain_Sow` writes to `+0x1A7` — `None` on the two
/// early exits, which leave the flag alone. `Grain_LabourEstimate` calls
/// `Grain_Sow` `population + 1` times a season and every one of those calls
/// moves the real flag in the original; nothing reads it between the sowing
/// that sets it and the next season's, so the search here is pure and the
/// difference is unobservable. **`[D]`**
pub fn sow_seed(
    t: &Tables,
    fields_grain: i32,
    grain_store: i32,
    labour: i32,
    advanced_farming: bool,
) -> (i32, Option<bool>) {
    if grain_store < 1 || labour < 1 {
        return (0, None);
    }
    let divisor = sow_divisor(t, advanced_farming);
    let max = t.grain.max_sacks_per_field;
    let affordable =
        |seed: i32| grain_store >= seed && labour >= t.grain.yield_per_sack * seed / divisor;

    let mut sacks = max;
    let mut sown = 0;
    let mut tried = 0;
    while tried < max {
        sown = fields_grain * sacks;
        if affordable(sown) {
            break;
        }
        tried += 1;
        sacks -= 1;
    }
    if sacks >= 1 {
        return (sown, Some(false));
    }

    sacks = max;
    for _ in 0..max {
        sown = sacks;
        if affordable(sacks) {
            break;
        }
        sacks -= 1;
    }
    if sacks < 1 {
        return (0, Some(false));
    }
    (sown, Some(true))
}

/// `Grain_Grow` (`0x0044D15A`) — one mid-season step, entering Summer and
/// Autumn.
pub fn grow_step(
    t: &Tables,
    county: &County,
    labour: i32,
    crop: i32,
    advanced_farming: bool,
) -> i32 {
    let capped = field_share(county, crop).min(labour * grow_per_worker(t, advanced_farming));
    fertility_bonus(county, capped).max(0)
}

/// `Grain_Harvest` (`0x0044D1E5`) — what the reapers bring in, entering Winter.
pub fn harvest_step(
    t: &Tables,
    county: &County,
    labour: i32,
    crop: i32,
    advanced_farming: bool,
) -> i32 {
    let crop = field_share(county, crop).max(0);
    let hands = if advanced_farming { labour / 2 } else { labour };
    crop.min(hands * harvest_per_worker(t, advanced_farming))
}

