#![allow(unused_imports)]
use super::*;
use super::factors::*;
use super::steps::*;
use super::bands::*;
use super::display::*;
use super::*;

/// The labour figure `Grain_Sow` tests against.
///
/// **Not established** — see [`JOB_GRAIN_FARMING`]. `docs/kingdom.md` §7.1
/// gives the inequality and never says which of the ten job slots supplies the
/// left-hand side.
pub fn grain_labour(t: &Tables, county: &County) -> i32 {
    county.labour[t.job.grain_farming]
}

/// Sow: spend the seed, and start the year's crop.
///
/// `crop[0]` is the seed that went in — **after** the weather has cut it, which
/// is what the store is debited — and `crop[1]` is that seed times
/// `g_grainYieldPerSack`, the number every later step works on.
pub fn sow(t: &Tables, county: &mut County, advanced_farming: bool) {
    let labour = grain_labour(t, county);
    let store = county.grain;
    let sown = sow_sacks(t, county, store, labour, advanced_farming);
    county.crop[0] = sow_factor(county.weather).apply(sown);
    // `field_0x24c = crop[0] - iVar1`: the band's effect, after less before.
    county.grain_weather_change = county.crop[0] - sown;
    county.crop[1] = county.crop[0] * t.grain.yield_per_sack;
    // What the crop is measured against for the rest of the year. A county
    // that fell back to a token handful records **one** field, not its real
    // count — so if it later loses a grain field the ratio still reads 1.
    county.fields_grain_sown =
        if county.sow_shortfall { 1 } else { county.fields_grain };
    // `+0x206`, written in the same two statements as `+0x202` and then left to
    // `County_DestroyField` to step down. The wheat picture divides by this one.
    county.fields_grain_standing = county.fields_grain_sown;
    county.grain -= county.crop[0];
}

/// Grow: cap the crop at what the farmhands can tend, apply fertility, then
/// this season's weather.
pub fn grow(t: &Tables, county: &mut County, advanced_farming: bool) {
    let labour = grain_labour(t, county);
    let grown = grow_step(t, county, labour, county.crop[1], advanced_farming);
    county.crop[1] = grow_factor(county.weather).apply(grown);
    county.grain_weather_change = county.crop[1] - grown;
}

/// Harvest: what the reapers bring in lands in the store.
///
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
///
/// **Switchable** — [`Quirk::HarvestIgnoresLabourCap`], `docs/bugs.md` B1. With
/// the quirk fixed the weather *scales what the reapers brought in*, which is
/// what the labour cap was computed for and what the two neighbouring branches
/// already do. The switch is inside the one expression, so the bug and the fix
/// are readable together.
pub fn harvest(t: &Tables, county: &mut County, advanced_farming: bool, quirks: Quirks) {
    let labour = grain_labour(t, county);
    let reaped = harvest_step(t, county, labour, county.crop[1], advanced_farming);
    let factor = harvest_factor(county.weather);
    // The base the band multiplies: the *standing* crop as the original reads
// it, or what the reapers could carry.
    let base = if quirks.reproduces(Quirk::HarvestIgnoresLabourCap) {
        county.crop[1]
    } else {
        reaped
    };
    county.crop[2] = if factor == Factor::NONE { reaped } else { factor.apply(base) };
    // After the band less what `Grain_Harvest` returned — which under the
    // reproduced quirk is a difference of two different bases, and is what the
    // original prints as weather's gain or loss.
    county.grain_weather_change = county.crop[2] - reaped;
    county.grain += county.crop[2];
}

/// `Grain_SeasonTick` (`0x0044C8AE`) — the random-event modifier on the store,
/// then sow, grow, grow or harvest, then next season's labour ceiling.
///
/// `season` is `g_season`, the season now *beginning*, so the sowing clause
/// fires at the **end of Winter** — `docs/kingdom.md` §3.3.
///
/// The order inside is the original's: `crop[2]` is cleared for every season
/// before anything else, and the event percentage is applied to the store
/// *before* the seed comes out of it, so a *"rats in the granary"* season eats
/// the seed corn too.
pub fn grain_season_tick(
    t: &Tables,
    county: &mut County,
    season: Season,
    advanced_farming: bool,
    quirks: Quirks,
) {
    county.crop[2] = 0;
    // `field_0x278 = 0`, then `Pct(grain, |p|)` in either arm. The store moves
    // by the same number with the event's sign; `pct` truncates toward zero, so
    // `pct(grain, p)` is exactly that signed amount and the arithmetic below is
    // unchanged by storing the figure.
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

// ---------------------------------------------------------------------------
// Livestock
// ---------------------------------------------------------------------------

