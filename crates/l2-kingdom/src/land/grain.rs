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

/// **`FUN_0044CF6F` — the crop's density band**, and the only producer of a
/// non-zero `Terrain_Set` variant in the game.
///
/// ```c
/// if (crop < 1 || fieldsGrain < 1)      return 2;
/// if (crop / fieldsGrain < 0x29)        return 3;
/// if (crop / fieldsGrain < 0x51)        return 7;
///                                       return 11;
/// ```
///
/// Four bands at 41 and 81 sacks a field, and the value **is** the terrain byte
/// `Grain_SeasonTick` then writes onto every grain tile of the county. `[D]`
///
/// `fields` is the second argument as the original passes it, which is **not**
/// `fieldsGrain` — see [`grain_stage_band`].
pub fn grain_crop_band(crop: i32, fields: i32) -> u8 {
    if crop < 1 || fields < 1 {
        return 2;
    }
    match crop / fields {
        d if d < 0x29 => 3,
        d if d < 0x51 => 7,
        _ => 11,
    }
}

/// **Which band a county's wheat is drawn at this season** — the three
/// `FUN_0044CF6F` calls in `Grain_SeasonTick` (`0x0044C8AE`), one per arm.
///
/// ```c
/// if (g_season == 1) { … sow …;    band = FUN_0044CF6F(crop[1], (byte)+0x206); }
/// else if (2 or 3)   { … grow …;   band = FUN_0044CF6F(crop[1], (byte)+0x206); }
/// else if (4)        { … harvest …; band = FUN_0044CF6F(crop[2], (byte)+0x206); }
/// ```
///
/// **This is the second time the wheat was fixed, and the first fix read
/// neither argument.** C124 transcribed the call as
/// `FUN_0044CF6F(county.crop[2], county.fieldsGrain)` — one line, stated for all
/// four seasons — and it is the Winter arm's first argument with a divisor
/// none of the three arms use. `crop[2]` is cleared at the top of every
/// season and filled only by the harvest, so in Spring, Summer and Autumn it
/// is always `0`, `FUN_0044CF6F` returns `2` for a zero crop, and every grain
/// field on the map was drawn at variant 0 for three seasons in four. A player,
/// on the build that carried that fix: *"wheat fields still not showing the
/// different stages of wheat growth."*
///
/// The divisor is `+0x206`, [`County::fields_grain_standing`]: the fields
/// sown this year less those since destroyed. `docs/decisions.md`
/// C195. `[D]`
pub fn grain_stage_band(county: &County, season: Season) -> u8 {
    let crop = match season {
        Season::Spring | Season::Summer | Season::Autumn => county.crop[1],
        Season::Winter => county.crop[2],
    };
    // `(uint)(byte)county.field_0x206` — a byte in the original.
    grain_crop_band(crop, county.fields_grain_standing & 0xFF)
}

/// **`Grain_SeasonTick`'s last two lines (`0x0044C8AE`), which had no
/// counterpart at all.**
///
/// A player: *"The wheat fields don't show the wheat growing."* This is half the
/// answer — the other half is [`l2_view::campaign::field_variant`], and
/// **neither half alone changes a pixel**, so the bug survived a
/// season pass this project believes it has read.
///
/// ```c
/// band = FUN_0044CF6F(<this season's crop word>, (byte)county.field_0x206);
/// variant = band < 3 ? 0 : (band - 3) / 4 + 1;
/// FUN_00469D21(county, band, variant, 2, 0xE);
/// ```
///
/// **The first argument changes with the season** — [`grain_stage_band`].
///
/// `FUN_00469D21(county, terrain, variant, lo, hi)` sweeps the whole tile array
/// in index order and calls `Terrain_Set(tile, terrain, variant)` on every tile
/// of that county carrying plane-0 bit `0x20` whose current `content` is in
/// `lo ..= hi`. So the repaint is bounded to tiles that are *already* growing
/// grain — a fallow or a pasture tile in the same county is untouched — and the
/// crop's stage is written onto the map.
///
/// **The one wrinkle, reproduced:** its shortfall arm.
///
/// ```c
/// if (lo == 2 && county.sowShortfall && notTheFirstTileThisSweep)
///     Terrain_Set(tile, 2, '\0');       /* this one gets nothing */
/// else
///     Terrain_Set(tile, terrain, variant);
/// ```
///
/// `sowShortfall` is `Grain_Sow`'s *"could not afford one sack a field and fell
/// back to a token handful"* flag. When it is set, **the first grain tile in
/// index order takes the whole crop's appearance and every other one is
/// repainted bare.** That is the game showing a failed sowing as one green field
/// among the empty ones, and it is a picture that is a rule — the same shape as
/// the pasture herd and the mine's animation rate. `[D]`
///
/// `season` is `g_season`, the season whose clause just ran, because that is
/// what chose the crop word the band is read from.
pub fn grain_repaint_fields(
    county_id: usize,
    county: &County,
    season: Season,
    map: &mut crate::map::CampaignMap,
) {
    // The variant `band < 3 ? 0 : (band - 3) / 4 + 1` is not stored: it is
    // recoverable from the band, which is the terrain byte, and
    // `l2_view::campaign::field_variant` recovers it.
    let band = grain_stage_band(county, season);
    let mut seen = false;
    for tile in 0..map.terrain.len() {
        if map.county[tile] as usize != county_id {
            continue;
        }
        if map.flags[tile] & crate::map::flags::FARMLAND == 0 {
            continue;
        }
        if !(2..=0x0E).contains(&map.terrain[tile]) {
            continue;
        }
        let painted = if county.sow_shortfall && seen { 2 } else { band };
        crate::field::paint_tile(map, tile, painted);
        seen = true;
    }
}

/// **`Grain_LabourEstimate`'s tail (`0x0044D374`) — the grain row's forecast.**
///
/// A player: *"Sidebar doesn't show grain being planted as a negative number."*
/// This is the number that would have said so, and until this function existed
/// nothing in the workspace computed it. `docs/decisions.md` C123
/// has the whole of it; the short version is that
/// [`grain_labour_estimate`] ports the **search loop** of `Grain_LabourEstimate`
/// and the original writes four more things after the loop ends:
///
/// ```c
/// staff = county.labour[0].workers;
/// county.field_0x230 = Grain_Sow(county, staff, county.grain);
/// if (season == 4)                county.crop[2]     = Grain_Harvest(county, staff, crop[1]);
/// if (season == 2 || season == 3) county.field_0x2FC = Grain_Grow   (county, staff, crop[1]);
///
/// if      (season == 1) county.field_0x22C = -county.field_0x230 - county.grainEaten;
/// else if (season == 4) county.field_0x22C =  county.crop[2]     - county.grainEaten;
/// else                  county.field_0x22C = -county.grainEaten;
/// ```
///
/// **In Spring the answer is `−sown − eaten` and cannot be positive**, which is
/// the player's sentence with the arithmetic under it: sowing spends grain, so
/// the store's forecast for the season you are about to enter is a loss twice
/// over.
///
/// # Three things that are easy to get wrong here, and one that was
///
/// * **The tail is not a by-product of the loop.** The loop calls
///   `Grain_Sow(county, workers, grain − grainEaten)` over every possible
///   staffing; the tail calls `Grain_Sow(county, staff, grain)` — the *actual*
/// allocation and the *undiminished* store. Two different questions, and
///   folding them would produce a plausible wrong number.
/// * **`crop[2]` and `+0x2FC` are written only in their own seasons** and keep
/// their previous value otherwise.
///   pass. Winter's arm then reads the `crop[2]` it has just written.
/// * **`+0x22C` is zeroed before the `popBand` guard**, so an empty county
///   forecasts nothing. The same shape
/// as [`herd_preview`], and the same reason.
/// * **This is why the estimate round runs twice.** `crate::field`'s module docs
///   worked that out — *"the panel forecasts, which the estimates fill from
///   whatever the allocator last decided"* — and then the port carried none of
///   them. Knowing why a pass exists is not the same as carrying what it writes.
///
/// `[D]`, read out of `0x0044D374` and matched against the row that draws it.
pub fn grain_preview(t: &Tables, county: &mut County, season_next: Season, advanced_farming: bool) {
    county.grain_change_expected = 0;
    if county.pop_band == 0 {
        return;
    }
    let staff = county.labour[crate::tables::JOB_GRAIN_FARMING];
    county.grain_sown_expected =
        sow_seed(t, county.fields_grain, county.grain, staff, advanced_farming).0;
    match season_next {
        Season::Winter => {
            county.crop[2] = harvest_step(t, county, staff, county.crop[1], advanced_farming);
        }
        Season::Summer | Season::Autumn => {
            county.grain_grown_expected =
                grow_step(t, county, staff, county.crop[1], advanced_farming);
        }
        Season::Spring => {}
    }
    county.grain_change_expected = match season_next {
        Season::Spring => -county.grain_sown_expected - county.grain_eaten,
        Season::Winter => county.crop[2] - county.grain_eaten,
        _ => -county.grain_eaten,
    };
}

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
/// for the same reason the original does: the
/// integer truncation inside `Grain_Sow` is part of the answer.
///
/// Two details that are the original's and look like slips:
///
/// * the search subtracts **`grainEaten`** from the store and the panel
///   forecast one line below it does not;
/// * `wanted` and `useful` are assigned in the same `if`, so for grain alone
/// the floor and the ceiling agree — **except when the search found
///   nothing**, where they are −1 and 0 because that is what they were
///   initialised to. That is the only reason [`GrainEstimate`] has two fields.
///   `Labour_Allocate` reads only the ceiling — **verified by exhaustion over
///   its 2,147 bytes: it reads `+0xCC + slot*0x0C` eight times and
///   `+0xC8 + slot*0x0C` not once** — so the floor is a display value and
///   nothing in the simulation depends on it.
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
) -> Option<GrainEstimate> {
    if county.pop_band == 0 {
        return None;
    }
    let store = county.grain - county.grain_eaten;
    let mut best = 1;
    let mut estimate = GrainEstimate { wanted: crate::county::LABOUR_NO_FLOOR, useful: 0 };
    for workers in 0..county.population {
        let got = match season_next {
            Season::Spring => sow_seed(t, county.fields_grain, store, workers, advanced_farming).0,
            Season::Winter => {
                harvest_step(t, county, workers, county.crop[1], advanced_farming)
            }
            _ => grow_step(t, county, workers, county.crop[1], advanced_farming),
        };
        if best < got {
            estimate = GrainEstimate { wanted: workers, useful: workers };
            best = got;
        }
    }
    Some(estimate)
}

