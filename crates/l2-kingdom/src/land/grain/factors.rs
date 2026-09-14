#![allow(unused_imports)]
use super::*;
use super::steps::*;
use super::actions::*;
use super::bands::*;
use super::display::*;
use super::*;

/// The sowing multiplier: *Flooding* quarters the sowing, *Frost* or *Storms*
/// halve it. **`[D]`** — `docs/kingdom.md` §7.1 says the branch structure is
/// unambiguous but that nothing outside the binary confirms the factors.
pub fn sow_factor(weather: Weather) -> Factor {
    match weather {
        Weather::Flooding => Factor(1, 4),
        Weather::Frost | Weather::Storms => Factor(1, 2),
        _ => Factor::NONE,
    }
}

/// The growing multiplier: *Sunny* multiplies the crop by 3/2 while *Drought*
/// and *Flooding* halve it.
pub fn grow_factor(weather: Weather) -> Factor {
    match weather {
        Weather::Sunny => Factor(3, 2),
        Weather::Drought | Weather::Flooding => Factor(1, 2),
        _ => Factor::NONE,
    }
}

/// The harvest multiplier: *Sunny* again gives 3/2, *Flooding* quarters,
/// *Frost* or *Storms* halve.
pub fn harvest_factor(weather: Weather) -> Factor {
    match weather {
        Weather::Sunny => Factor(3, 2),
        Weather::Flooding => Factor(1, 4),
        Weather::Frost | Weather::Storms => Factor(1, 2),
        _ => Factor::NONE,
    }
}

/// The divisor `Grain_Sow` tests labour against: 5 with *Advanced Farming* on
/// and 2 with it off. **The smaller divisor demands more labour**, so turning
/// the option off makes sowing harder.
fn sow_divisor(t: &Tables, advanced_farming: bool) -> i32 {
    if advanced_farming {
        t.grain.labour_divisor_advanced
    } else {
        t.grain.labour_divisor_basic
    }
    .max(1)
}

/// `Grain_Grow`'s crop cap per worker: 10 with *Advanced Farming* on, and the
/// **sowing divisor's** 2 with it off, because both read the same global.
fn grow_per_worker(t: &Tables, advanced_farming: bool) -> i32 {
    if advanced_farming {
        t.grain.grow_per_worker_advanced
    } else {
        t.grain.labour_divisor_basic
    }
}

/// `Grain_Harvest`'s: **3** with *Advanced Farming* on — and it halves the
/// reapers first, so the effective rate is 1.5 sacks a head — and 2 with it
/// off, on the full workforce.
fn harvest_per_worker(t: &Tables, advanced_farming: bool) -> i32 {
    if advanced_farming {
        t.grain.harvest_per_worker_advanced
    } else {
        t.grain.labour_divisor_basic
    }
}

