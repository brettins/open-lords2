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
///
/// Returns 0 when even one sack a field cannot be afforded or worked.
///
/// This is the first half of [`sow_sacks`], which is the whole function; it is
/// kept separate because "sacks a field" is the number the county panel draws
/// and the number two published guides quote.
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
///
/// The normal answer is `fieldsGrain * sacksPerField`. If even one sack a field
/// cannot be afforded or worked, the function sets [`County::sow_shortfall`]
/// and retries against the **sack count alone**, ignoring the field count: the
/// largest `s <= 10` with `s <= grainStore` and `12 * s / divisor <= labour`,
/// sown as the county's whole seed. `Grain_SeasonTick` then records the field
/// usage as 1. If even that fails the flag is cleared again and nothing is
/// sown.
///
/// **Two early exits do not touch the flag.** An empty store or an empty
/// workforce returns 0 with [`County::sow_shortfall`] left as it was. That is
/// the original's and it is reproduced; nothing observable turns on it, because
/// a county that sowed no seed grows no crop either way.
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

/// The arithmetic of [`sow_sacks`], with no county to write to.
///
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

// The fallback: a token handful, measured in sacks.
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
///
/// ```c
/// crop = min(FieldShare(county, crop), labour * perWorker);
/// crop = FertilityBonus(county, crop);
/// return max(crop, 0);
/// ```
///
/// **This is not the crate's old `grow`**, which was one weather multiplier and
/// nothing else. The labour cap is the part that matters: a county that puts
/// nobody on the fields grows nothing whatever it sowed, and one that puts
/// half a workforce on grows half.
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
///
/// ```c
/// crop = max(FieldShare(county, crop), 0);
/// if (advancedFarming) labour /= 2;
/// return min(crop, labour * perWorker);
/// ```
///
/// **No fertility here** — it was spent at the two growing steps — and the
/// workforce is halved before the multiplier when *Advanced Farming* is on, so
/// the effective rate is one and a half sacks a reaper against two without it.
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

