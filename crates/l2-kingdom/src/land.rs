//! Land — fertility, fields, grain and livestock.
//! `docs/kingdom.md` §7.1 and §7.2.
//!
//! # The grain cycle
//!
//! One number moving through three fields, once round the year:
//!
//! ```text
//! end of Winter   Grain_Sow      sown = fields x sacksPerField ; store -= sown
//!                                crop = sown x g_grainYieldPerSack
//! end of Spring   Grain_Grow     crop adjusted
//! end of Summer   Grain_Grow     crop adjusted
//! end of Autumn   Grain_Harvest  store += crop
//! ```
//!
//! Remember §3.3's subtlety: `Season_Advance` sets `g_season` to the season
//! *about to begin* before running the economy, so "sows when `g_season == 1`"
//! means "sows at the end of Winter". The game's own FAQ page states all three
//! clauses: *"Grain is only planted at the end of the winter turn, and is
//! harvested at the end of the Autumn. You will see the extra grain in your
//! county at the start of each Winter turn."*
//!
//! # Fertility
//!
//! Three lines, and both of its consequences contradict the printed manual:
//! **one fallow field per two grain fields is exactly break-even**, and
//! **cattle fields do not enter the formula at all**.

use crate::county::{County, MAX_FIELDS};
use crate::math::{clamp, pct, pct_of, per_myriad};
use crate::tables::{HerdCrowdingRow, Season, Tables, Weather, HERD_CROWDING_COUNT};

// ---------------------------------------------------------------------------
// Fertility
// ---------------------------------------------------------------------------

/// The fertility scale's bounds. `L2.eng` group 22 names seven levels from
/// *"Infertile — almost no production"* to *"Excellent fertility — bumper
/// crop!"*, but `docs/kingdom.md` §7.2 could not trace the mapping from this
/// scalar to those seven, so this crate does not invent one.
pub const FERTILITY_MIN: i32 = -100;
pub const FERTILITY_MAX: i32 = 100;

/// A fallow field is worth this much fertility a season.
pub const FERTILITY_PER_FALLOW: i32 = 6;
/// A grain field costs this much.
pub const FERTILITY_PER_GRAIN: i32 = 3;

/// `Fertility_Update` (`0x0044BFD5`).
///
/// ```text
/// county.fertility += 6 * county.fieldsFallow - 3 * county.fieldsGrain;
/// clamp -100 .. 100;
/// if (!g_optAdvancedFarming) county.fertility = 0;
/// ```
pub fn update_fertility(county: &mut County, advanced_farming: bool) {
    county.fertility +=
        FERTILITY_PER_FALLOW * county.fields_fallow - FERTILITY_PER_GRAIN * county.fields_grain;
    county.fertility = clamp(county.fertility, FERTILITY_MIN, FERTILITY_MAX);
    if !advanced_farming {
        county.fertility = 0;
    }
}

/// County `+0x210` — **which field the reclamation gang is working on.**
///
/// `FUN_0044C53B` and `FUN_0044C5FC` are the same three lines twice: the slot,
/// among the county's fields whose *terrain* says reclamation, with the
/// **highest progress**, ties going to the lowest slot. So the gang finishes
/// the nearly-done field before it starts the next one, and a county with two
/// hundred workers completes one field a season rather than inching four along
/// together.
///
/// The two differ only in what they leave behind when nothing is being
/// reclaimed — `0` for the tick, `99` for the estimate, which is the estimate's
/// "is there any work at all" test. Here that is `None`.
pub fn reclaim_leader(county: &County, map: &crate::map::CampaignMap) -> Option<usize> {
    let mut best: Option<(usize, i32)> = None;
    for slot in 0..MAX_FIELDS {
        let Some(tile) = county.field_tile(slot) else { continue };
        if crate::field::classify(map.terrain[tile]) != crate::field::FieldType::Reclaiming {
            continue;
        }
        let progress = county.field_progress[slot] as i32;
        // Strictly greater, scanning upward: a tie keeps the earlier slot.
        if best.is_none_or(|(_, p)| p < progress) {
            best = Some((slot, progress));
        }
    }
    best.map(|(slot, _)| slot)
}

/// The terrain byte a field under reclamation shows at `progress`: the four
/// quarters `0x19 … 0x1C`, and [`crate::field::terrain::FALLOW`] the moment it
/// is finished. `Field_ReclaimTick` repaints the tile on every step, which is
/// what eventually moves the field out of [`County::fields_reclaiming`] and
/// into [`County::fields_fallow`] at the next recount.
pub fn reclaim_terrain(t: &Tables, progress: i32) -> u8 {
    let max = t.field.progress_max;
    for quarter in 1..=4 {
        if progress < max * quarter / 4 {
            return crate::field::terrain::RECLAIM_FIRST + (quarter as u8 - 1);
        }
    }
    crate::field::terrain::FALLOW
}

/// `Field_ReclaimTick` (`0x0044C093`) — **spend the county's reclamation
/// labour.**
///
/// This crate used to advance every started field by a flat
/// [`crate::tables::FIELD_RECLAIM_PER_SEASON`] whatever anybody was doing, so
/// [`crate::tables::JOB_FIELD_RECLAMATION`] was a job nobody had to hold. The
/// original spends the job's worker count as a **budget**, one unit of progress
/// a worker:
///
/// ```c
/// budget = labour[2].workers;
/// slot   = mostAdvancedReclaimingField(county);   /* or 0 */
/// for (twenty slots, from slot, wrapping) {
///     if (!reclaiming(slot)) continue;
///     take = min(budget, 200); progress += take; budget -= take;
///     if (progress > 800) budget += progress - 800;     /* the overshoot comes back */
///     repaint(tile, quarter(progress));
///     if (budget < 1) return;
/// }
/// ```
///
/// Three consequences, all the original's. **A county with nobody on
/// reclamation reclaims nothing** — the first field takes a budget of zero and
/// the walk returns. **Two hundred workers is one field a season**, the
/// manual's *"never more than a quarter of a field in a single season"*, and
/// eight hundred workers is a whole field. And a field finished with labour to
/// spare hands the **overshoot back to the budget**, so the gang moves straight
/// on to the next field in the rota rather than wasting the season.
///
/// The stored progress is deliberately **not** clamped to `progress_max`: the
/// original writes the overshooting value back even as it refunds the excess.
/// It is inert, because the tile has already been repainted as fallow and the
/// slot is no longer reclamation on the next pass.
pub fn reclaim_fields(t: &Tables, county: &mut County, map: &mut crate::map::CampaignMap) {
    let mut budget = county.labour[crate::tables::JOB_FIELD_RECLAMATION];
    let start = reclaim_leader(county, map).unwrap_or(0);
    for step in 0..MAX_FIELDS {
        let slot = (start + step) % MAX_FIELDS;
        let Some(tile) = county.field_tile(slot) else { continue };
        if crate::field::classify(map.terrain[tile]) != crate::field::FieldType::Reclaiming {
            continue;
        }
        let mut progress = county.field_progress[slot] as i32;
        if budget <= t.field.reclaim_per_season {
            progress += budget;
            budget = 0;
        } else {
            progress += t.field.reclaim_per_season;
            budget -= t.field.reclaim_per_season;
        }
        if progress > t.field.progress_max {
            budget += progress - t.field.progress_max;
        }
        crate::field::paint_tile(map, tile, reclaim_terrain(t, progress));
        county.field_progress[slot] = progress.clamp(0, u16::MAX as i32) as u16;
        if budget < 1 {
            return;
        }
    }
}

// ---------------------------------------------------------------------------
// Grain
// ---------------------------------------------------------------------------

/// A weather multiplier, as an exact `(numerator, denominator)` pair. Kept as a
/// ratio rather than a percentage so `3/2` is `3/2` and not `150%` rounded
/// twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Factor(pub i32, pub i32);

impl Factor {
    pub const NONE: Factor = Factor(1, 1);

    pub fn apply(self, value: i32) -> i32 {
        ((value as i64 * self.0 as i64) / self.1 as i64) as i32
    }
}

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

/// `Grain_Sow` in full (`0x0044CFE1`) — **the sacks that actually go into the
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

    // The fallback: a token handful, measured in sacks rather than in fields.
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

/// `FUN_0044D281` — **scale a standing crop by the grain fields still
/// standing.**
///
/// Runs at the top of both `Grain_Grow` and `Grain_Harvest`. If the county now
/// has *fewer* grain fields than it sowed, the crop is cut to
/// `PctOf(fieldsGrain, fieldsGrainSown)` percent of itself; if it has as many
/// or more, nothing happens. So ploughing a wheat field under in midsummer
/// costs a share of the year's crop, and painting new grain in midsummer buys
/// nothing until the next sowing.
pub fn field_share(county: &County, crop: i32) -> i32 {
    if county.fields_grain < county.fields_grain_sown {
        pct(crop, crate::math::pct_of(county.fields_grain, county.fields_grain_sown))
    } else {
        crop
    }
}

/// `FUN_0044D303` — **fertility, at half strength, once per growing season.**
///
/// `crop + Pct(crop, fertility / 2)`, so the −100…100 scalar
/// [`update_fertility`] keeps is worth −50 % … +50 % *per grow step* and there
/// are two of them a year: a perfectly fertile county reaps 2.25 times what a
/// neutral one does and a ruined one a quarter. The original writes the
/// division as an `if` whose two arms are identical, which is a compiler
/// artefact of a signed divide, not a rule.
///
/// This is the whole of fertility's effect on the crop. It is applied **after**
/// the labour cap, so fertility multiplies what the farmhands could actually
/// tend rather than what the field could have grown.
pub fn fertility_bonus(county: &County, crop: i32) -> i32 {
    crop + pct(crop, county.fertility / 2)
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
    county.crop[1] = county.crop[0] * t.grain.yield_per_sack;
    // What the crop is measured against for the rest of the year. A county
    // that fell back to a token handful records **one** field, not its real
    // count — so if it later loses a grain field the ratio still reads 1.
    county.fields_grain_sown =
        if county.sow_shortfall { 1 } else { county.fields_grain };
    county.grain -= county.crop[0];
}

/// Grow: cap the crop at what the farmhands can tend, apply fertility, then
/// this season's weather.
pub fn grow(t: &Tables, county: &mut County, advanced_farming: bool) {
    let labour = grain_labour(t, county);
    let grown = grow_step(t, county, labour, county.crop[1], advanced_farming);
    county.crop[1] = grow_factor(county.weather).apply(grown);
}

/// Harvest: what the reapers bring in lands in the store.
///
/// **The weather does not scale the harvest; it replaces it.** Four of the six
/// bands assign `crop[2]` from `crop[1]` — the *standing* crop — rather than
/// from what `Grain_Harvest` just returned, so under *Sunny* a county reaps
/// three halves of everything it grew however few reapers it sent, and under
/// *Frost*, *Storms* or *Flooding* it reaps a fixed fraction of the same. Only
/// *Cloudy* and *Drought* leave the labour cap standing. That is the binary's,
/// it is four separate `if`s reading the wrong word, and it is reproduced
/// because it is what the game does. **`[V]`** on the reading — the four
/// assignments are `crop[1]`-sourced in the decompilation and the surrounding
/// two branches at sowing and growing are self-sourced.
pub fn harvest(t: &Tables, county: &mut County, advanced_farming: bool) {
    let labour = grain_labour(t, county);
    let reaped = harvest_step(t, county, labour, county.crop[1], advanced_farming);
    let factor = harvest_factor(county.weather);
    county.crop[2] =
        if factor == Factor::NONE { reaped } else { factor.apply(county.crop[1]) };
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
pub fn grain_season_tick(t: &Tables, county: &mut County, season: Season, advanced_farming: bool) {
    county.crop[2] = 0;
    if county.event_grain_pct != 0 {
        county.grain += pct(county.grain, county.event_grain_pct);
        county.event_grain_pct = 0;
    }
    county.grain = county.grain.max(0);
    match season {
        Season::Spring => sow(t, county, advanced_farming),
        Season::Summer | Season::Autumn => grow(t, county, advanced_farming),
        Season::Winter => harvest(t, county, advanced_farming),
    }
}

// ---------------------------------------------------------------------------
// Livestock
// ---------------------------------------------------------------------------

/// The labour figure the herd is staffed from — county `+0xD0`, which is
/// labour record 1 of the nine at `+0xC4 + job * 0x0C`.
///
/// **`[V]`, three ways.** `Herd_SeasonTick` passes `+0xD0`; `0xD0 - 0xC4` is
/// exactly one 12-byte record; and the England turn-one fixture's own arithmetic closes —
/// county 1 holds 218 cattle farmers and 217 wood cutters against a population
/// of 435, and county 2 holds 323 and 133 against 456. Both sum to the
/// population exactly.
pub fn herd_labour(t: &Tables, county: &County) -> i32 {
    county.labour[t.job.cattle_farming]
}

/// `herd / fieldsCattle`, head per pasture field — the input to the crowding
/// bands. `FUN_0044D913`'s first three lines.
///
/// A county with cattle but no pasture gets [`crate::tables::HERD_NO_PASTURE_DENSITY`],
/// which is far above the top band; an empty herd gets 0, which is the bottom
/// of the bottom band.
pub fn herd_density(t: &Tables, herd: i32, fields_cattle: i32) -> i32 {
    if herd < 1 {
        0
    } else if fields_cattle == 0 {
        t.herd.no_pasture_density
    } else {
        herd / fields_cattle
    }
}

/// `FUN_0044D913` — the crowding level stored in county `+0x25C`, one of the
/// four values `L2.eng` group 77 names (`docs/kingdom.md` §13.1).
///
/// **A county with no pasture is at maximum crowding**, which the original
/// says twice: once through [`herd_density`]'s sentinel and again as an
/// explicit override after the bands. Both are reproduced, because they are
/// separable — a ruleset that lowered `no_pasture_density` below the top band
/// would find the override still holding, exactly as the binary does.
///
/// §13.1's pseudocode shows an `if (herd < 1)` arm before the bands, as though
/// an empty herd had no crowding at all. It does not: that guard is on the map
/// *graphic*, and the level is written unconditionally. An empty herd on
/// pasture is at density 0 and therefore in the *lowest* band.
pub fn herd_crowding(t: &Tables, herd: i32, fields_cattle: i32) -> i32 {
    let last = t.herd.crowding[HERD_CROWDING_COUNT - 1];
    if fields_cattle == 0 {
        return last.level;
    }
    let density = herd_density(t, herd, fields_cattle);
    for row in t.herd.crowding.iter() {
        if density <= row.density_max {
            return row.level;
        }
    }
    last.level
}

/// The crowding band a stored level names.
///
/// The original matches the level **exactly** — `if (crowding == 10) … else if
/// (crowding == 20) … else if (crowding == 30) … else 7` — so anything that is
/// not one of the three named values falls into the harshest band rather than
/// being interpolated. A save or a mod holding 15 gets the *"Massive
/// overcrowding!!"* rates, and that is the original's own behaviour rather
/// than a defensive choice of ours.
fn crowding_band(t: &Tables, crowding: i32) -> HerdCrowdingRow {
    let last = t.herd.crowding[HERD_CROWDING_COUNT - 1];
    for row in t.herd.crowding.iter().take(HERD_CROWDING_COUNT - 1) {
        if crowding == row.level {
            return *row;
        }
    }
    last
}

/// What one season does to a herd, before the weather and any random event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HerdGrowth {
    pub births: i32,
    pub deaths: i32,
}

impl HerdGrowth {
    pub fn net(self) -> i32 {
        self.births - self.deaths
    }
}

/// **`FUN_0044DA99` — the rule that a herd has to be tended.**
/// `docs/kingdom.md` §13.
///
/// ```text
/// staffing  = PctOf(labour, herd * 3), capped at 200
/// deathRate = crowding.death_rate + (staffing < 100 ? (100 - staffing) / 3 : 0)
/// birthRate = Pct(crowding.birth_rate, staffing) + smallHerdBonus
/// deaths    = per_myriad(herd * 100, deathRate)      [x 3/2 in Winter]
/// births    = per_myriad(herd, birthRate)            [x 3/2 in Spring]
/// ```
///
/// **Three labourers a head is full staffing**, and below it the shortfall is
/// added to the death rate at a third of a point per point missing — a herd
/// with nobody at all tending it loses `crowding + 33` per ten thousand a
/// season on top of everything else. Above 100% the benefit is the birth rate
/// only, and it stops at 200%.
///
/// **A county with no pasture at all loses half its herd, or all of it below
/// six head**, and nothing else in the function runs. That is the branch that
/// makes `fieldsCattle` load-bearing rather than decorative.
///
/// The two small rounding tells at the end are the original's and are kept:
/// a herd whose *rate* is non-zero but whose *count* rounds to zero is given
/// one animal either way, and deaths can never exceed the herd.
///
/// `season` is a `g_season` index; 0 is the original's *No Season* and matches
/// neither bonus.
pub fn herd_growth(
    t: &Tables,
    herd: i32,
    fields_cattle: i32,
    labour: i32,
    crowding: i32,
    season: u8,
) -> HerdGrowth {
    if herd == 0 {
        return HerdGrowth::default();
    }
    if fields_cattle == 0 {
        let deaths = if herd < t.herd.no_pasture_kill_all_below {
            herd
        } else {
            herd / t.herd.no_pasture_divisor
        };
        return HerdGrowth { births: 0, deaths };
    }

    let mut staffing = pct_of(labour, herd.saturating_mul(t.herd.labour_per_head));
    if staffing > t.herd.staffing_max - 1 {
        staffing = t.herd.staffing_max;
    }
    let band = crowding_band(t, crowding);

    // Positive, and added to the *deaths*. The binary writes it as
    // `-((staffing - 100) / 3)`; C truncates towards zero, so negating first
    // gives the same answer as `(100 - staffing) / 3` and this is not the place
    // to be clever about it.
    let understaffed =
        if staffing < 100 { -((staffing - 100) / t.herd.understaffing_divisor) } else { 0 };
    let death_rate = band.death_rate + understaffed;

    let mut birth_rate = pct(band.birth_rate, staffing);
    if staffing >= 100 {
        for &(below, bonus) in t.herd.small_bonus.iter() {
            if herd < below {
                birth_rate += bonus;
                break;
            }
        }
    }

    let (num, den) = t.herd.season_bonus;
    let mut deaths = per_myriad(herd.saturating_mul(100), death_rate);
    if season == t.herd.culling_season {
        deaths = deaths * num / den;
    }
    let mut births = per_myriad(herd, birth_rate);
    if season == t.herd.calving_season {
        births = births * num / den;
    }

    // A rate that rounds away to nothing still moves one animal.
    if births == 0 {
        if birth_rate == 0 {
            if deaths == 0 && death_rate != 0 {
                deaths = 1;
            }
        } else {
            births = 1;
        }
    }
    if herd < deaths {
        deaths = herd;
    }
    HerdGrowth { births, deaths }
}

/// `FUN_0044DD4D`'s second call — next season's *"Calf births expected"*,
/// *"Cow deaths expected"* and *"Change due to farming"* (`L2.eng` group 77).
///
/// The herd it forecasts from is `herd - herdEaten`: the ration pass has
/// already taken this season's animals, and the panel assumes next season will
/// take as many again. That double subtraction is the original's, and it is
/// what makes `change` the number a player sees rather than `births - deaths`.
///
/// Guarded on `popBand`, which is the original's guard — an empty county
/// forecasts nothing. The labour search the same function performs, which
/// fills the two spare words of the labour record with a suggested and a
/// growth-maximising worker count, is **not** reproduced: `County::labour` is
/// one integer a job and has nowhere to put them (`docs/screens-county.md` §8).
pub fn herd_preview(t: &Tables, county: &mut County, season_next: u8) {
    county.herd_change_expected = 0;
    if county.pop_band == 0 {
        return;
    }
    let head = county.herd - county.herd_eaten;
    let g = herd_growth(
        t,
        head,
        county.fields_cattle,
        herd_labour(t, county),
        county.herd_crowding,
        season_next,
    );
    county.herd_births_expected = g.births;
    county.herd_deaths_expected = g.deaths;
    county.herd_change_expected = g.net() - county.herd_eaten;
}

/// `Herd_SeasonTick` (`0x0044D60D`) — births and deaths from
/// [`herd_growth`], then the weather's percentage swing and the random-event
/// modifier, then a fresh [`herd_crowding`] and next season's forecast.
///
/// The event modifier is tested for the sentinel **before** it is tested for
/// its sign: `if (mod == 99) { change = 0; births = 0; }`. So 99 is not
/// "+99%", it is *"Cattle will not reproduce this season"* — the *"No bull"*
/// event, `L2.eng` group 314. See [`crate::event::HERD_NO_GROWTH`].
///
/// The order matters twice over. The sentinel suppresses the **weather** swing
/// as well as the event's own, so a sunny season and a dead prize bull cancel
/// out — and it zeroes only the *births*, so **an understaffed herd still dies
/// during a No Bull season.** Both are the binary's.
///
/// `season` and `season_next` are `g_season` indices; the herd's own tick is
/// run on the season now beginning and the forecast on the one after it,
/// exactly as `Herd_SeasonTick` passes `g_season` and `g_seasonNext`.
pub fn herd_season_tick(t: &Tables, county: &mut County, season: u8, season_next: u8) {
    let growth = herd_growth(
        t,
        county.herd,
        county.fields_cattle,
        herd_labour(t, county),
        county.herd_crowding,
        season,
    );
    let mut births = growth.births;
    let mut deaths = growth.deaths;

    let mut weather_change = pct(county.herd, t.weather[county.weather.index() as usize].herd_pct);
    if county.event_herd_pct == crate::event::HERD_NO_GROWTH {
        weather_change = 0;
        births = 0;
    } else if county.event_herd_pct < 0 {
        deaths += pct(county.herd, -county.event_herd_pct);
    } else if county.event_herd_pct > 0 {
        births += pct(county.herd, county.event_herd_pct);
    }
    county.event_herd_pct = 0;

    if weather_change < 0 {
        deaths -= weather_change;
    } else {
        births += weather_change;
    }

    county.herd += births - deaths;
    county.herd = county.herd.max(0);
    county.herd_crowding = herd_crowding(t, county.herd, county.fields_cattle);
    herd_preview(t, county, season_next);
}

// ---------------------------------------------------------------------------
// The labour ceilings
// ---------------------------------------------------------------------------

/// `Grain_LabourEstimate` (`0x0044D374`) — the **grain** ceiling, for the
/// season the sowing happens in.
///
/// The original is not the inversion of [`sacks_per_field`] it looks like; it
/// is a plain forward scan:
///
/// ```c
/// best = 1; ceiling = 0;
/// for (workers = 0; workers < population; workers++) {
///     got = Grain_Sow(county, workers, grain - grainEaten);   /* season 1 */
///     if (best < got) { ceiling = workers; best = got; }
/// }
/// wanted[0] = (nothing beat 1) ? -1 : ceiling;
/// useful[0] = ceiling;
/// ```
///
/// Because `Grain_Sow` is monotone in labour and the test is strict, the answer
/// *is* "the fewest farmers that reach the best yield" — but by scanning, and
/// the loop stops one short of the population, so a county can never want every
/// one of its people on the fields. **`[D]`**, and reproduced by search here
/// rather than by a closed form, for the same reason the original does: the
/// integer truncation inside `Grain_Sow` is part of the answer.
///
/// Two details that are the original's and look like slips:
///
/// * the search subtracts **`grainEaten`** from the store and the panel
///   forecast one line below it does not;
/// * `wanted` and `useful` are written from the same variable, so for grain
///   alone the floor and the ceiling are the same number. `Labour_Allocate`
///   reads only the ceiling — **verified by exhaustion over its 2,147 bytes:
///   it reads `+0xCC + slot*0x0C` eight times and `+0xC8 + slot*0x0C` not
///   once** — so the floor is a display value and nothing here depends on it.
///
/// # All four seasons
///
/// The search runs whichever of the three grain functions next season will
/// use — `Grain_Sow` entering Spring, `Grain_Harvest` entering Winter, and
/// `Grain_Grow` for the two in between, all against the crop the county is
/// carrying now. Summer, Autumn and Winter used to return `None` because
/// [`grow`] and [`harvest`] were a weather multiplier and nothing more; they
/// are now [`grow_step`] and [`harvest_step`], which are the original's, so
/// the ceiling is a real number in every season.
///
/// Returns `None` only where the original writes nothing: `popBand == 0`, an
/// empty county, whose ceiling keeps whatever it had — which for a county that
/// has never been estimated is [`crate::county::LABOUR_UNSET`], and
/// `Labour_Allocate` reads that back as zero.
pub fn grain_labour_estimate(
    t: &Tables,
    county: &County,
    season_next: Season,
    advanced_farming: bool,
) -> Option<i32> {
    if county.pop_band == 0 {
        return None;
    }
    let store = county.grain - county.grain_eaten;
    let mut best = 1;
    let mut ceiling = 0;
    for workers in 0..county.population {
        let got = match season_next {
            Season::Spring => sow_seed(t, county.fields_grain, store, workers, advanced_farming).0,
            Season::Winter => {
                harvest_step(t, county, workers, county.crop[1], advanced_farming)
            }
            _ => grow_step(t, county, workers, county.crop[1], advanced_farming),
        };
        if best < got {
            ceiling = workers;
            best = got;
        }
    }
    Some(ceiling)
}

/// `Herd_LabourEstimate` (`0x0044DD4D`) — the **cattle** ceiling.
///
/// A search too, over the same `births − deaths` the season's own tick
/// computes:
///
/// ```c
/// for (workers = 0; workers < population; workers++) {
///     Herd_BirthsAndDeaths(county, herd - herdEaten, workers, crowding, season);
///     net = births - deaths;
///     if (bestNet < net) { ceiling = workers; bestNet = net; }
/// }
/// useful[1] = ceiling;                 /* 999999 when the loop never ran */
/// ```
///
/// **`[D]`.** Three things about it are worth keeping.
///
/// The herd is the post-ration one (`herd − herdEaten`) and the crowding is the
/// **stored** band, not a freshly derived one — which is why `Field_SetType`
/// calls `Herd_UpdateCrowding` before every refresh.
///
/// The answer is *not* `herd * 3`. Staffing is
/// `PctOf(labour, herd * 3)` capped at 200, deaths flatten at 100 % and births
/// keep rising to 200 %, so the argmax is the first labour figure reaching
/// **200 %** — about `6 * herd`, twice [`crate::tables::HERD_LABOUR_PER_HEAD`].
/// **`[I]`** on that closed form and `[D]` on the loop; truncation inside
/// `Pct(birthRate, staffing)` can land it a little below, which is exactly why
/// the original searches.
///
/// A county with no people leaves the ceiling at
/// [`crate::county::LABOUR_UNSET`] — the loop never runs and 999,999 is its
/// initial value. That is where the sentinel comes from, and `Labour_Allocate`
/// reads it back as 0.
pub fn herd_labour_estimate(t: &Tables, county: &County, season: u8) -> i32 {
    let herd = county.herd - county.herd_eaten;
    let mut best = -1_000_000;
    let mut ceiling = crate::county::LABOUR_UNSET;
    for workers in 0..county.population {
        let g = herd_growth(t, herd, county.fields_cattle, workers, county.herd_crowding, season);
        let net = g.births - g.deaths;
        if best < net {
            ceiling = workers;
            best = net;
        }
    }
    ceiling
}

/// `Field_ReclaimEstimate` (`0x0044C278`) — the **reclamation** ceiling: the
/// work outstanding in every field under reclamation, at most a season's worth
/// each.
///
/// ```c
/// total = 0;
/// for (slot = 0; slot < 20; slot++)
///     if (fieldTile[slot] != 0 && terrain[fieldTile[slot]] > 0x18)
///         total += min(800 - progress[slot], 200);
/// wanted[2] = -1; useful[2] = total;
/// ```
///
/// **`[D]`.** The `> 0x18` test is the same one that puts a field in
/// [`County::fields_reclaiming`], so the two agree by construction.
///
/// **This ceiling is honoured and then ignored**, and that is worth saying
/// plainly. [`reclaim_fields`] advances every started field by a flat
/// [`crate::tables::FIELD_RECLAIM_PER_SEASON`] whatever the county's
/// reclamation labour is; the original spends `labour[2]` as a *budget*,
/// starting at the most advanced field and carrying the remainder on. Until
/// that is fixed, putting people on reclamation changes nothing — a silent
/// no-op rather than a wrong number, but a gap all the same.
pub fn reclaim_labour_estimate(
    t: &Tables,
    county: &County,
    map: &crate::map::CampaignMap,
) -> i32 {
    let mut total = 0;
    for slot in 0..MAX_FIELDS {
        let Some(tile) = county.field_tile(slot) else { continue };
        if crate::field::classify(map.terrain[tile]) != crate::field::FieldType::Reclaiming {
            continue;
        }
        let left = t.field.progress_max - county.field_progress[slot] as i32;
        total += left.clamp(0, t.field.reclaim_per_season);
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The stock ruleset. Every rule below takes it as an argument now.
    const T: &Tables = &Tables::DEFAULT;

    /// **One fallow field per two grain fields is exactly break-even**, and
    /// **cattle fields do not enter the formula at all.** Both contradict the
    /// printed manual (`docs/decisions.md` C10).
    #[test]
    fn one_fallow_field_per_two_grain_fields_is_break_even() {
        for pairs in 1..=8 {
            let mut c = County::new();
            c.fields_fallow = pairs;
            c.fields_grain = pairs * 2;
            c.fields_cattle = 99; // deliberately absurd
            update_fertility(&mut c, true);
            assert_eq!(c.fertility, 0, "{pairs} fallow, {} grain", pairs * 2);
        }
    }

    #[test]
    fn cattle_fields_are_invisible_to_fertility() {
        let mut with = County::new();
        with.fields_fallow = 4;
        with.fields_grain = 4;
        with.fields_cattle = 8;
        update_fertility(&mut with, true);

        let mut without = County::new();
        without.fields_fallow = 4;
        without.fields_grain = 4;
        update_fertility(&mut without, true);

        assert_eq!(with.fertility, without.fertility);
        assert_eq!(with.fertility, 12, "6*4 - 3*4");
    }

    #[test]
    fn fertility_is_clamped_to_its_hundred_point_scale() {
        let mut c = County::new();
        c.fields_fallow = 20;
        for _ in 0..20 {
            update_fertility(&mut c, true);
        }
        assert_eq!(c.fertility, FERTILITY_MAX);

        let mut c = County::new();
        c.fields_grain = 20;
        for _ in 0..20 {
            update_fertility(&mut c, true);
        }
        assert_eq!(c.fertility, FERTILITY_MIN);
    }

    /// **`docs/kingdom.md` §9 point 3.** With Advanced Farming off, every
    /// county's fertility in the England turn-one fixture is 0.
    #[test]
    fn basic_farming_pins_fertility_at_zero() {
        let mut c = County::new();
        c.fields_fallow = 10;
        c.fields_grain = 1;
        update_fertility(&mut c, false);
        assert_eq!(c.fertility, 0);
    }

    /// A county with fields under reclamation and a workforce to spend on
    /// them. `slots` are the field slots that carry a reclaiming tile; every
    /// other slot is fallow, so the walk skips it.
    fn reclaiming(slots: &[(usize, u16)], workers: i32) -> (County, crate::map::CampaignMap) {
        let mut map = crate::map::CampaignMap::empty();
        let mut c = County::new();
        c.labour[crate::tables::JOB_FIELD_RECLAMATION] = workers;
        for slot in 0..MAX_FIELDS {
            // Tile 0 is "no field"; start at 1.
            c.field_tiles[slot] = slot as u16 + 1;
            map.terrain[slot + 1] = crate::field::terrain::FALLOW;
        }
        for &(slot, progress) in slots {
            c.field_progress[slot] = progress;
            map.terrain[slot + 1] = reclaim_terrain(T, progress as i32);
        }
        (c, map)
    }

    /// Only a field whose *tile* says reclamation is worked on, and the
    /// progress word alone does not say so.
    #[test]
    fn only_fields_already_started_are_reclaimed() {
        let (mut c, mut map) = reclaiming(&[(1, 100)], 10_000);
        c.field_progress[0] = 0; // fallow tile: never started
        c.field_progress[2] = 800; // fallow tile: finished long ago
        reclaim_fields(T, &mut c, &mut map);
        assert_eq!(c.field_progress[0], 0);
        assert_eq!(c.field_progress[1], 300, "a quarter, and no more");
        assert_eq!(c.field_progress[2], 800);
    }

    /// **The manual's rule, and the reason it is a rule:** the per-field step
    /// is capped at a quarter however large the workforce is.
    #[test]
    fn a_field_under_way_finishes_in_four_seasons_at_most() {
        let (mut c, mut map) = reclaiming(&[(5, 1)], 10_000);
        for _ in 0..4 {
            reclaim_fields(T, &mut c, &mut map);
        }
        assert!(c.field_progress[5] >= T.field.progress_max as u16);
        assert_eq!(
            map.terrain[6],
            crate::field::terrain::FALLOW,
            "and the tile becomes fallow, which is the reward"
        );
    }

    /// **The labour is a budget, and this is what closes the gap
    /// `crates/l2-kingdom/tests/labour_gap.rs` recorded.** Nobody on
    /// reclamation used to mean a field advanced anyway.
    #[test]
    fn a_county_with_nobody_on_reclamation_reclaims_nothing() {
        let (mut c, mut map) = reclaiming(&[(0, 100), (3, 400)], 0);
        reclaim_fields(T, &mut c, &mut map);
        assert_eq!(c.field_progress[0], 100);
        assert_eq!(c.field_progress[3], 400);

        // Fifty workers buy fifty units, all of it on the leading field.
        c.labour[crate::tables::JOB_FIELD_RECLAMATION] = 50;
        reclaim_fields(T, &mut c, &mut map);
        assert_eq!(c.field_progress[3], 450, "the most advanced field goes first");
        assert_eq!(c.field_progress[0], 100, "and the other gets nothing");
    }

    /// The gang starts on the **most advanced** field, spends at most a
    /// quarter there, and carries the rest round the rota — including the
    /// overshoot from a field it has just finished.
    #[test]
    fn the_budget_walks_the_rota_from_the_leading_field() {
        let (mut c, mut map) = reclaiming(&[(0, 0), (7, 700)], 500);
        reclaim_fields(T, &mut c, &mut map);
        // Slot 7 leads, is offered its quarter, and hands the 100 units of
        // overshoot straight back to the budget — so 400 wraps round to slot
        // 0, which can only take its own quarter.
        //
        // **The stored progress overshoots to 900 and is not clamped.** The
        // original refunds the excess to the budget and writes the raw sum
        // back anyway. It is inert: the tile is already fallow, so the slot is
        // never offered work again.
        assert_eq!(c.field_progress[7], 900);
        assert_eq!(map.terrain[8], crate::field::terrain::FALLOW);
        assert_eq!(c.field_progress[0], 200);
    }

    /// **The manual says 5 sacks a field, twice, and it is wrong.** Two
    /// independent players measured 6 fields sowing 60 sacks and 9 fields
    /// sowing 90.
    #[test]
    fn a_well_supplied_county_sows_ten_sacks_a_field_not_five() {
        // Plenty of everything.
        assert_eq!(sacks_per_field(T, 6, 10_000, 10_000, true), 10);
        assert_eq!(sacks_per_field(T, 9, 10_000, 10_000, true), 10);
        assert_eq!(6 * sacks_per_field(T, 6, 10_000, 10_000, true), 60);
        assert_eq!(9 * sacks_per_field(T, 9, 10_000, 10_000, true), 90);
    }

    #[test]
    fn the_seed_store_is_the_binding_constraint_when_labour_is_plentiful() {
        // 6 fields, 30 sacks in store: 5 a field fits, 6 does not.
        assert_eq!(sacks_per_field(T, 6, 30, 100_000, true), 5);
        assert_eq!(sacks_per_field(T, 6, 29, 100_000, true), 4);
        assert_eq!(sacks_per_field(T, 6, 5, 100_000, true), 0, "not even one a field");
    }

    /// The labour test is `labour >= 12 * fields * sacks / divisor`, and the
    /// divisor is *larger* with Advanced Farming on — so the option makes
    /// sowing cheaper in labour, not dearer.
    #[test]
    fn advanced_farming_needs_less_labour_for_the_same_sowing() {
        for fields in 1..=16 {
            let advanced = sacks_per_field(T, fields, 10_000, 200, true);
            let basic = sacks_per_field(T, fields, 10_000, 200, false);
            assert!(advanced >= basic, "{fields} fields: {advanced} vs {basic}");
        }
        // 8 fields x 10 sacks needs 12*80/5 = 192 workers advanced, 480 basic.
        assert_eq!(sacks_per_field(T, 8, 10_000, 192, true), 10);
        assert_eq!(sacks_per_field(T, 8, 10_000, 191, true), 9);
        assert_eq!(sacks_per_field(T, 8, 10_000, 192, false), 4);
    }

    #[test]
    fn a_county_with_no_grain_fields_sows_nothing() {
        assert_eq!(sacks_per_field(T, 0, 10_000, 10_000, true), 0);
        let mut c = County::new();
        c.grain = 500;
        sow(T, &mut c, true);
        assert_eq!(c.grain, 500);
        assert_eq!(c.crop, [0; 3]);
    }

    /// The whole year, in Cloudy weather where every factor is 1: 6 fields at
    /// 10 sacks is 60 sacks of seed, becoming 720 sacks at harvest.
    ///
    /// **And the three crop words are seed, standing crop and harvest** — not
    /// three growth stages. `crop[0]` is written once, at sowing, and never
    /// moves again; `crop[1]` is rewritten in place by every grow; `crop[2]` is
    /// cleared at the top of every season and filled at the harvest.
    #[test]
    fn a_full_year_of_grain_turns_each_sack_into_twelve() {
        let mut c = County::new();
        c.fields_grain = 6;
        c.grain = 200;
        c.labour[T.job.grain_farming] = 10_000;
        c.weather = Weather::Cloudy;

        grain_season_tick(T, &mut c, Season::Spring, true);
        assert_eq!(c.grain, 140, "60 sacks of seed spent");
        assert_eq!(c.crop[0], 60, "the seed, not the crop");
        assert_eq!(c.crop[1], 720, "and the crop is the seed times twelve");
        assert_eq!(c.fields_grain_sown, 6);

        grain_season_tick(T, &mut c, Season::Summer, true);
        assert_eq!(c.crop[1], 720);
        assert_eq!(c.crop[2], 0, "nothing is harvested in summer");
        grain_season_tick(T, &mut c, Season::Autumn, true);
        assert_eq!(c.crop[1], 720);
        grain_season_tick(T, &mut c, Season::Winter, true);
        assert_eq!(c.crop[2], 720, "and this is the harvest");
        assert_eq!(c.grain, 860, "140 + 720");
    }

    /// **The labour cap, which is the whole reason the grain ceiling exists.**
    /// A county that sows a full crop and then puts nobody on the fields grows
    /// and reaps nothing at all; one that sends half the hands reaps half.
    #[test]
    fn a_crop_nobody_tends_comes_to_nothing() {
        let sow_and_run = |grow_hands: i32| {
            let mut c = County::new();
            c.fields_grain = 6;
            c.grain = 200;
            c.weather = Weather::Cloudy;
            c.labour[T.job.grain_farming] = 10_000;
            grain_season_tick(T, &mut c, Season::Spring, true);
            assert_eq!(c.crop[1], 720);
            c.labour[T.job.grain_farming] = grow_hands;
            for season in [Season::Summer, Season::Autumn, Season::Winter] {
                grain_season_tick(T, &mut c, season, true);
            }
            c.crop[2]
        };
        assert_eq!(sow_and_run(0), 0, "nobody tends it, nobody reaps it");
        // 10 hands tend 100 sacks a season and 5 of them reap 15, so the cap
        // that binds at the end is the harvest's, not the growing's.
        assert_eq!(sow_and_run(10), 15);
        assert_eq!(sow_and_run(10_000), 720, "and a full workforce loses nothing");
    }

    /// **Fertility is the crop's multiplier, once per growing season**, and it
    /// was doing nothing at all in this crate before: `Grain_Grow` applies
    /// `crop + Pct(crop, fertility / 2)` after the labour cap, and there are
    /// two grow steps a year.
    #[test]
    fn fertility_multiplies_the_crop_twice_a_year() {
        let year = |fertility: i32| {
            let mut c = County::new();
            c.fields_grain = 6;
            c.grain = 200;
            c.weather = Weather::Cloudy;
            c.labour[T.job.grain_farming] = 10_000;
            grain_season_tick(T, &mut c, Season::Spring, true);
            c.fertility = fertility;
            for season in [Season::Summer, Season::Autumn, Season::Winter] {
                grain_season_tick(T, &mut c, season, true);
            }
            c.crop[2]
        };
        assert_eq!(year(0), 720);
        // +100 is +50% a step: 720 -> 1080 -> 1620.
        assert_eq!(year(100), 1620);
        // -100 is -50% a step: 720 -> 360 -> 180.
        assert_eq!(year(-100), 180);
    }

    /// **Ploughing a wheat field under in midsummer costs a share of the
    /// year's crop.** `FUN_0044D281` scales the standing crop by
    /// `fieldsGrain / fieldsGrainSown` whenever the county has fewer grain
    /// fields than it sowed — and painting *more* grain buys nothing until the
    /// next sowing.
    #[test]
    fn losing_a_grain_field_mid_year_cuts_the_standing_crop() {
        let mut c = County::new();
        c.fields_grain = 6;
        c.grain = 200;
        c.weather = Weather::Cloudy;
        c.labour[T.job.grain_farming] = 10_000;
        grain_season_tick(T, &mut c, Season::Spring, true);
        assert_eq!(c.crop[1], 720);

        c.fields_grain = 3; // half the fields turned over to pasture
        grain_season_tick(T, &mut c, Season::Summer, true);
        assert_eq!(c.crop[1], 360);

        // And back the other way: twelve fields on a crop sown on six is still
        // a crop sown on six.
        c.fields_grain = 12;
        grain_season_tick(T, &mut c, Season::Autumn, true);
        assert_eq!(c.crop[1], 360);
    }

    #[test]
    fn a_sunny_year_beats_a_stormy_one_by_a_wide_margin() {
        let year = |weather: Weather| {
            let mut c = County::new();
            c.fields_grain = 6;
            c.grain = 200;
            c.labour[T.job.grain_farming] = 10_000;
            c.weather = weather;
            for season in Season::ALL {
                grain_season_tick(T, &mut c, season, true);
            }
            c.grain
        };
        let sunny = year(Weather::Sunny);
        let cloudy = year(Weather::Cloudy);
        let stormy = year(Weather::Storms);
        let flooded = year(Weather::Flooding);
        assert!(sunny > cloudy, "{sunny} vs {cloudy}");
        assert!(cloudy > stormy, "{cloudy} vs {stormy}");
        assert!(stormy > flooded, "{stormy} vs {flooded}");
    }

    #[test]
    fn the_grain_event_modifier_is_applied_and_consumed() {
        let mut c = County::new();
        c.grain = 1000;
        c.weather = Weather::Cloudy;
        c.event_grain_pct = -30; // "eaten by rats"
        grain_season_tick(T, &mut c, Season::Winter, true);
        assert_eq!(c.grain, 700);
        assert_eq!(c.event_grain_pct, 0);
    }

    #[test]
    fn the_grain_store_never_goes_negative() {
        let mut c = County::new();
        c.grain = 10;
        c.event_grain_pct = -200;
        grain_season_tick(T, &mut c, Season::Winter, true);
        assert_eq!(c.grain, 0);
    }

    // -----------------------------------------------------------------------
    // The herd
    //
    // Every test below has to say what its county's pasture and labour are,
    // and that is the whole lesson. `County::new()` has no cattle fields and
    // nobody working, which the old tests took as a neutral background: a
    // county in that state now loses half its herd a season, and every one of
    // them was measuring the weather against a slaughterhouse without knowing.
    // -----------------------------------------------------------------------

    /// A county whose herd is pastured and staffed, so a rule can be measured
    /// on its own. `labour` is stated rather than derived — three a head is
    /// full staffing, and a test that wants "well staffed" should have to write
    /// the number down.
    fn grazing(herd: i32, fields_cattle: i32, labour: i32) -> County {
        let mut c = County::new();
        c.herd = herd;
        c.fields_cattle = fields_cattle;
        c.labour[T.job.cattle_farming] = labour;
        c.herd_crowding = herd_crowding(T, herd, fields_cattle);
        c.weather = Weather::Cloudy;
        c
    }

    const SPRING: u8 = Season::Spring as u8;
    const SUMMER: u8 = Season::Summer as u8;
    const AUTUMN: u8 = Season::Autumn as u8;
    const WINTER: u8 = Season::Winter as u8;

    /// Only *Sunny* grows the herd and only *Cloudy* leaves it alone — and the
    /// weather is now a term **on top of** the births and deaths rather than
    /// the only thing that happens.
    #[test]
    fn the_herd_follows_the_weather_table_exactly() {
        for w in Weather::ALL {
            let mut c = grazing(1000, 100, 3000);
            c.weather = w;
            let farming = herd_growth(T, 1000, 100, 3000, c.herd_crowding, SUMMER);
            herd_season_tick(T, &mut c, SUMMER, AUTUMN);
            let weather = pct(1000, T.weather[w.index() as usize].herd_pct);
            assert_eq!(c.herd, 1000 + farming.net() + weather, "{}", w.name());
        }
        // ... and the ordering the table encodes is still the ordering.
        let after = |w: Weather| {
            let mut c = grazing(1000, 100, 3000);
            c.weather = w;
            herd_season_tick(T, &mut c, SUMMER, AUTUMN);
            c.herd
        };
        assert!(after(Weather::Sunny) > after(Weather::Cloudy));
        assert!(after(Weather::Cloudy) > after(Weather::Frost));
    }

    #[test]
    fn wolves_take_their_percentage_and_the_modifier_is_consumed() {
        let mut c = grazing(200, 20, 600);
        c.event_herd_pct = -25; // "taken by wolves"
        let farming = herd_growth(T, 200, 20, 600, c.herd_crowding, SUMMER);
        herd_season_tick(T, &mut c, SUMMER, AUTUMN);
        assert_eq!(c.herd, 200 + farming.net() - 50, "a quarter of the herd, on top of farming");
        assert_eq!(c.event_herd_pct, 0);
    }

    /// **The rule the crate did not have.** `docs/kingdom.md` §13.
    ///
    /// Walks the whole labour domain rather than one comfortable value,
    /// because the rule that was here before — none — passed every test at
    /// every one of them.
    #[test]
    fn three_labourers_a_head_is_full_staffing_and_below_it_cattle_die() {
        let herd = 100;
        let fields = 10; // density 10, the mildest crowding band
        let crowding = herd_crowding(T, herd, fields);
        assert_eq!(crowding, 10);

        let full = herd * T.herd.labour_per_head;
        assert_eq!(full, 300, "three a head");

        let mut previous_deaths = i32::MAX;
        let mut previous_births = -1;
        for labour in 0..=(full * 3) {
            let g = herd_growth(T, herd, fields, labour, crowding, SUMMER);
            assert!(g.deaths <= previous_deaths, "labour {labour} killed more than {}", labour - 1);
            assert!(g.births >= previous_births, "labour {labour} bred less than {}", labour - 1);
            previous_deaths = g.deaths;
            previous_births = g.births;
        }

        // The boundary itself. Deaths are `herd * rate / 100` because the
        // per-ten-thousand rate is applied to a hundred times the herd.
        let staffed = herd_growth(T, herd, fields, full, crowding, SUMMER);
        let abandoned = herd_growth(T, herd, fields, 0, crowding, SUMMER);
        assert_eq!(staffed.deaths, 1, "1 per 10,000 of 100 head");
        assert_eq!(abandoned.deaths, 34, "1 + (100 - 0) / 3 per 10,000, which is a third of it");
        assert_eq!(staffed.births, 14, "1,400 per 10,000");
        assert_eq!(abandoned.births, 0, "and a staffing of zero is a birth rate of zero");
        assert!(staffed.net() > 0 && abandoned.net() < 0, "the sign of the season flips");

        // One worker short of full staffing is not yet a penalty - the
        // shortfall is divided by three and truncated - and a third of the
        // workforce missing is.
        assert_eq!(herd_growth(T, herd, fields, full - 1, crowding, SUMMER).deaths, 1);
        assert_eq!(herd_growth(T, herd, fields, full / 3, crowding, SUMMER).deaths, 23);
    }

    /// The cap: staffing stops paying at 200%, and the comparison really is
    /// against 199 rather than 200.
    #[test]
    fn the_staffing_benefit_stops_at_twice_the_workers() {
        let (herd, fields) = (10_000, 1_000);
        let crowding = herd_crowding(T, herd, fields);
        let at = |labour: i32| herd_growth(T, herd, fields, labour, crowding, SUMMER).births;
        let full = herd * T.herd.labour_per_head;
        assert!(at(2 * full) > at(full), "twice the workers is worth having");
        assert_eq!(at(2 * full), at(10 * full), "ten times over is not");
        // 199% is not rounded up to 200%: the guard is `199 < staffing`.
        let ninety_nine = full * 199 / 100;
        assert!(at(ninety_nine) < at(2 * full));
    }

    /// A small herd breeds faster — **only when it is fully staffed.** The
    /// bonus sits inside the `staffing >= 100` arm, which is what stops it
    /// rescuing a herd nobody is looking after.
    #[test]
    fn a_small_herd_breeds_faster_but_only_if_somebody_is_tending_it() {
        for &(below, bonus) in T.herd.small_bonus.iter() {
            let herd = below - 1;
            let fields = 10;
            let crowding = herd_crowding(T, herd, fields);
            let staffed = herd_growth(T, herd, fields, herd * 3, crowding, SUMMER);
            let base = pct(T.herd.crowding[0].birth_rate, 100);
            assert_eq!(
                staffed.births,
                per_myriad(herd, base + bonus).max(1),
                "a herd of {herd} should get the {bonus} bonus"
            );
            let idle = herd_growth(T, herd, fields, 0, crowding, SUMMER);
            assert!(idle.births <= 1, "and an unstaffed herd of {herd} gets none of it");
        }
        // Twenty-five head and up is not a small herd.
        let crowding = herd_crowding(T, 25, 10);
        let plain = herd_growth(T, 25, 10, 75, crowding, SUMMER);
        assert_eq!(plain.births, per_myriad(25, 1400).max(1));
    }

    /// **No pasture is not a small penalty.** `docs/kingdom.md` §13's second
    /// branch, walked over the whole herd domain it splits.
    #[test]
    fn a_county_with_no_pasture_loses_half_its_herd_or_all_of_it() {
        for herd in 0..=200 {
            for labour in [0, herd * 3, herd * 30] {
                let g = herd_growth(T, herd, 0, labour, 40, SUMMER);
                let expect = if herd == 0 {
                    0
                } else if herd < T.herd.no_pasture_kill_all_below {
                    herd
                } else {
                    herd / 2
                };
                assert_eq!(g.deaths, expect, "herd {herd}, labour {labour}");
                assert_eq!(g.births, 0, "and nothing is born");
            }
        }
        assert_eq!(herd_growth(T, 5, 0, 999, 10, SPRING).deaths, 5, "five head, all of them");
        assert_eq!(herd_growth(T, 6, 0, 999, 10, SPRING).deaths, 3, "six head, half of them");
    }

    /// `FUN_0044D913`, over the whole density domain — `docs/kingdom.md` §13.1.
    #[test]
    fn crowding_bands_break_at_eleven_twentyone_and_thirtyone_head_a_field() {
        for fields in 1..=20 {
            for herd in 0..=(fields * 45) {
                let density = herd / fields;
                let expect = if density < 11 {
                    10
                } else if density < 21 {
                    20
                } else if density < 31 {
                    30
                } else {
                    40
                };
                assert_eq!(
                    herd_crowding(T, herd, fields),
                    expect,
                    "{herd} head on {fields} fields is {density} a field"
                );
            }
        }
        // The four bands of `L2.eng` group 77, and no fifth.
        let seen: Vec<i32> = (0..=200).map(|h| herd_crowding(T, h, 5)).collect();
        let mut levels: Vec<i32> = seen.clone();
        levels.dedup();
        assert_eq!(levels, vec![10, 20, 30, 40], "in that order, and only those");
    }

    /// A county with no pasture is at maximum crowding whatever its herd —
    /// which the binary says twice, and which is why the last band's rates are
    /// what an unpastured county would be judged by if the harsher branch above
    /// it ever stopped firing.
    #[test]
    fn no_pasture_is_maximum_crowding_at_every_herd_size() {
        for herd in 0..=500 {
            assert_eq!(herd_crowding(T, herd, 0), 40, "herd {herd} on no pasture");
        }
        // An *empty* herd on real pasture is the other extreme.
        assert_eq!(herd_crowding(T, 0, 8), 10);
    }

    /// Crowding is a stored field, not a derived one, and the difference shows:
    /// the season is worked out at the crowding the herd *had*.
    #[test]
    fn a_season_is_judged_at_last_seasons_crowding_and_then_recomputed() {
        let mut c = grazing(300, 10, 900); // density 30 -> band 30
        assert_eq!(c.herd_crowding, 30);
        c.fields_cattle = 100; // pasture bought: density would now be 3
        herd_season_tick(T, &mut c, SUMMER, AUTUMN);
        assert_eq!(c.herd_crowding, 10, "and the new pasture counts from next season");
    }

    /// Every crowding band, against every staffing level. Two monotonicities
    /// that hold across the whole grid, which is the shape of the table rather
    /// than a sample of it: **more crowding is always worse on both counts.**
    #[test]
    fn a_more_crowded_herd_always_dies_faster_and_breeds_slower() {
        let (herd, fields) = (10_000, 1_000);
        for labour in (0..=(herd * 6)).step_by(1_000) {
            let mut last: Option<HerdGrowth> = None;
            for band in T.herd.crowding.iter() {
                let g = herd_growth(T, herd, fields, labour, band.level, SUMMER);
                if let Some(previous) = last {
                    assert!(g.deaths >= previous.deaths, "band {} at labour {labour}", band.level);
                    assert!(g.births <= previous.births, "band {} at labour {labour}", band.level);
                }
                last = Some(g);
            }
        }
    }

    /// A crowding value the bands do not name falls into the harshest one,
    /// which is the original's `else` and not a choice of ours.
    #[test]
    fn an_unnamed_crowding_value_is_treated_as_the_worst_one() {
        let (herd, fields, labour) = (10_000, 1_000, 30_000);
        let worst = herd_growth(T, herd, fields, labour, 40, SUMMER);
        for odd in [0, 5, 15, 25, 35, 41, 1_000, -7] {
            assert_eq!(herd_growth(T, herd, fields, labour, odd, SUMMER), worst, "crowding {odd}");
        }
    }

    /// Spring brings calves and Winter kills, both by exactly `3 / 2`.
    #[test]
    fn spring_is_worth_half_again_in_calves_and_winter_half_again_in_losses() {
        let (herd, fields) = (10_000, 1_000);
        let crowding = herd_crowding(T, herd, fields);
        let plain = herd_growth(T, herd, fields, herd * 3, crowding, SUMMER);
        let spring = herd_growth(T, herd, fields, herd * 3, crowding, SPRING);
        let winter = herd_growth(T, herd, fields, herd * 3, crowding, WINTER);
        assert_eq!(spring.births, plain.births * 3 / 2);
        assert_eq!(spring.deaths, plain.deaths, "Spring does not kill");
        assert_eq!(winter.deaths, plain.deaths * 3 / 2);
        assert_eq!(winter.births, plain.births, "and Winter does not calve");
        // Autumn and Summer are neither, and so is the original's "No Season".
        for season in [SUMMER, AUTUMN, 0] {
            assert_eq!(herd_growth(T, herd, fields, herd * 3, crowding, season), plain);
        }
    }

    /// **A rate that rounds away to nothing still moves one animal**, and the
    /// direction depends on which rate it was. The last cow in a county nobody
    /// is farming dies; the last cow in a county that is, calves.
    #[test]
    fn the_last_cow_calves_if_it_is_tended_and_dies_if_it_is_not() {
        let mut kept = grazing(1, 1, 3);
        kept.weather = Weather::Drought;
        herd_season_tick(T, &mut kept, SUMMER, AUTUMN);
        assert_eq!(kept.herd, 2, "a doubled birth rate for a tiny herd, and Pct(1, -10) is 0");

        let mut abandoned = grazing(1, 1, 0);
        abandoned.weather = Weather::Drought;
        herd_season_tick(T, &mut abandoned, SUMMER, AUTUMN);
        assert_eq!(abandoned.herd, 0, "a birth rate of zero and a death rate that is not");
    }

    /// The herd can never go negative, and deaths are capped at the herd.
    #[test]
    fn deaths_never_exceed_the_herd_and_the_herd_never_goes_negative() {
        for herd in 0..=120 {
            for fields in [0, 1, 4] {
                let g = herd_growth(T, herd, fields, 0, 40, WINTER);
                assert!(g.deaths <= herd, "herd {herd} on {fields} fields lost {}", g.deaths);
            }
        }
        let mut c = grazing(4, 1, 0);
        c.event_herd_pct = -500;
        herd_season_tick(T, &mut c, WINTER, SPRING);
        assert_eq!(c.herd, 0);
    }

    /// The rule as a player meets it: a county that stops assigning cattle
    /// farmers watches the herd go, and one that keeps them watches it grow.
    #[test]
    fn an_untended_herd_dwindles_away_over_a_few_years_and_a_tended_one_does_not() {
        let seasons = [SPRING, SUMMER, AUTUMN, WINTER];
        let run = |labour: i32| {
            let mut c = grazing(200, 20, labour);
            c.pop_band = 8;
            for year in 0..8 {
                for (i, &s) in seasons.iter().enumerate() {
                    let next = seasons[(i + 1) % 4];
                    herd_season_tick(T, &mut c, s, next);
                    let _ = year;
                }
            }
            c.herd
        };
        let tended = run(600);
        let abandoned = run(0);
        assert!(tended > 200, "a staffed herd grows: {tended}");
        assert!(abandoned < 200, "an unstaffed one does not: {abandoned}");
        assert!(abandoned < tended / 4, "{abandoned} against {tended}");
    }

    /// The forecast the county panel draws — group 77's *"Calf births
    /// expected"*, *"Cow deaths expected"* and *"Change due to farming"*.
    #[test]
    fn the_forecast_is_next_seasons_growth_less_what_the_people_will_eat() {
        let mut c = grazing(100, 10, 300);
        c.pop_band = 4;
        c.herd_eaten = 13;
        herd_preview(T, &mut c, SPRING);
        let g = herd_growth(T, 100 - 13, 10, 300, c.herd_crowding, SPRING);
        assert_eq!(c.herd_births_expected, g.births);
        assert_eq!(c.herd_deaths_expected, g.deaths);
        assert_eq!(c.herd_change_expected, g.net() - 13, "and the eating is counted again");

        // An empty county forecasts nothing at all: the original guards on
        // popBand, which is zero only where there are no people.
        let mut empty = grazing(100, 10, 300);
        empty.pop_band = 0;
        herd_preview(T, &mut empty, SPRING);
        assert_eq!(empty.herd_change_expected, 0);
    }

    #[test]
    fn a_factor_is_an_exact_ratio_rather_than_a_rounded_percentage() {
        assert_eq!(Factor(3, 2).apply(7), 10, "not 7 * 150% rounded twice");
        assert_eq!(Factor(1, 4).apply(7), 1);
        assert_eq!(Factor::NONE.apply(12345), 12345);
    }
}
