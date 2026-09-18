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

pub fn grow_factor(weather: Weather) -> Factor {
    match weather {
        Weather::Sunny => Factor(3, 2),
        Weather::Drought | Weather::Flooding => Factor(1, 2),
        _ => Factor::NONE,
    }
}

pub fn harvest_factor(weather: Weather) -> Factor {
    match weather {
        Weather::Sunny => Factor(3, 2),
        Weather::Flooding => Factor(1, 4),
        Weather::Frost | Weather::Storms => Factor(1, 2),
        _ => Factor::NONE,
    }
}

pub(super) fn sow_divisor(t: &Tables, advanced_farming: bool) -> i32 {
    if advanced_farming {
        t.grain.labour_divisor_advanced
    } else {
        t.grain.labour_divisor_basic
    }
    .max(1)
}

pub(super) fn grow_per_worker(t: &Tables, advanced_farming: bool) -> i32 {
    if advanced_farming {
        t.grain.grow_per_worker_advanced
    } else {
        t.grain.labour_divisor_basic
    }
}

pub(super) fn harvest_per_worker(t: &Tables, advanced_farming: bool) -> i32 {
    if advanced_farming {
        t.grain.harvest_per_worker_advanced
    } else {
        t.grain.labour_divisor_basic
    }
}

