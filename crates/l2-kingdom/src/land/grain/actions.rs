#![allow(unused_imports)]
use super::*;
use super::factors::*;
use super::steps::*;
use super::bands::*;
use super::display::*;
use super::*;

pub fn grain_labour(t: &Tables, county: &County) -> i32 {
    county.labour[t.job.grain_farming]
}

pub fn sow(t: &Tables, county: &mut County, advanced_farming: bool) {
    let labour = grain_labour(t, county);
    let store = county.grain;
    let sown = sow_sacks(t, county, store, labour, advanced_farming);
    county.crop[0] = sow_factor(county.weather).apply(sown);
    county.grain_weather_change = county.crop[0] - sown;
    county.crop[1] = county.crop[0] * t.grain.yield_per_sack;
    county.fields_grain_sown =
        if county.sow_shortfall { 1 } else { county.fields_grain };
    // `+0x206`, written in the same two statements as `+0x202` and then left to
    // `County_DestroyField` to step down. The wheat picture divides by this one.
    county.fields_grain_standing = county.fields_grain_sown;
    county.grain -= county.crop[0];
}

pub fn grow(t: &Tables, county: &mut County, advanced_farming: bool) {
    let labour = grain_labour(t, county);
    let grown = grow_step(t, county, labour, county.crop[1], advanced_farming);
    county.crop[1] = grow_factor(county.weather).apply(grown);
    county.grain_weather_change = county.crop[1] - grown;
}

/// **The weather does not scale the harvest; it replaces it.** Four of the six
/// bands assign `crop[2]` from `crop[1]` — the *standing* crop —
/// from what `Grain_Harvest` just returned, so under *Sunny* a county reaps
/// three halves of everything it grew however few reapers it sent, and under
/// *Frost*, *Storms* or *Flooding* it reaps a fixed fraction of the same. Only
/// *Cloudy* and *Drought* leave the labour cap standing. That is the binary's,
/// it is four separate `if`s reading the wrong word, and it is reproduced
/// because it is what the game does. **`[V]`** on the reading — the four
/// assignments are `crop[1]`-sourced in the decompilation and the surrounding
/// two branches at sowing and growing are self-sourced.
pub fn harvest(t: &Tables, county: &mut County, advanced_farming: bool, quirks: Quirks) {
    let labour = grain_labour(t, county);
    let reaped = harvest_step(t, county, labour, county.crop[1], advanced_farming);
    let factor = harvest_factor(county.weather);
    let base = if quirks.reproduces(Quirk::HarvestIgnoresLabourCap) {
        county.crop[1]
    } else {
        reaped
    };
    county.crop[2] = if factor == Factor::NONE { reaped } else { factor.apply(base) };
    county.grain_weather_change = county.crop[2] - reaped;
    county.grain += county.crop[2];
}

/// `Grain_SeasonTick` (`0x0044C8AE`) — the random-event modifier on the store,
/// then sow, grow, grow or harvest, then next season's labour ceiling.
pub fn grain_season_tick(
    t: &Tables,
    county: &mut County,
    season: Season,
    advanced_farming: bool,
    quirks: Quirks,
) {
    // `grain = grain - +0x18C`, the shadow `Ration_ApplyAll` (`0x0044BF04`)
    // left. Before the event percentage
    // takes its cut of what the eaters left.
    county.grain -= county.grain_eaten_shadow;
    county.crop[2] = 0;
    county.grain_event_change = 0;
    if county.event_grain_pct != 0 {
        county.grain_event_change = pct(county.grain, county.event_grain_pct.abs());
        county.grain += pct(county.grain, county.event_grain_pct);
        county.event_grain_pct = 0;
    }
    county.grain = county.grain.max(0);
    match season {
        Season::Spring => sow(t, county, advanced_farming),
        Season::Summer | Season::Autumn => grow(t, county, advanced_farming),
        Season::Winter => harvest(t, county, advanced_farming, quirks),
    }
}


