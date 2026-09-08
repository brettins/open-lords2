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
use crate::math::{clamp, pct};
use crate::tables::{Season, Tables, Weather};

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

/// `Field_ReclaimTick` (`0x0044C093`) — push every field under reclamation
/// towards 800 by at most 200.
///
/// **`[I]` on which fields are being reclaimed.** `docs/kingdom.md` §7.2 gives
/// the rate, the target and the manual's confirmation of the quarter-per-season
/// cap, and does not say what marks a field as in progress. A field with
/// progress strictly between 0 and 800 is taken to be under way; a field at 0
/// has not been started and a field at 800 is done.
pub fn reclaim_fields(t: &Tables, county: &mut County) {
    for field in 0..MAX_FIELDS {
        let p = county.field_progress[field] as i32;
        if p > 0 && p < t.field.progress_max {
            county.reclaim_field(field, t.field.reclaim_per_season);
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
    let divisor =
        if advanced_farming { t.grain.labour_divisor_advanced } else { t.grain.labour_divisor_basic };
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

/// The labour figure `Grain_Sow` tests against.
///
/// **Not established** — see [`JOB_GRAIN_FARMING`]. `docs/kingdom.md` §7.1
/// gives the inequality and never says which of the ten job slots supplies the
/// left-hand side.
pub fn grain_labour(t: &Tables, county: &County) -> i32 {
    county.labour[t.job.grain_farming]
}

/// Sow: spend the seed, and put this year's crop into stage 0.
pub fn sow(t: &Tables, county: &mut County, advanced_farming: bool) {
    let sacks =
        sacks_per_field(t, county.fields_grain, county.grain, grain_labour(t, county), advanced_farming);
    let seed = county.fields_grain * sacks;
    county.grain -= seed;
    let crop = seed * t.grain.yield_per_sack;
    county.crop = [sow_factor(county.weather).apply(crop), 0, 0];
}

/// Grow: move the crop on one stage, scaled by this season's weather.
pub fn grow(county: &mut County, stage: usize) {
    let previous = county.crop[stage - 1];
    county.crop[stage] = grow_factor(county.weather).apply(previous);
}

/// Harvest: the last stage lands in the store and the crop is cleared.
pub fn harvest(county: &mut County) {
    let yield_ = harvest_factor(county.weather).apply(county.crop[2]);
    county.grain += yield_;
    county.crop = [0; 3];
}

/// `Grain_SeasonTick` — sow, grow, grow, harvest, plus the random-event
/// modifier on the store.
///
/// `season` is `g_season`, the season now *beginning*.
pub fn grain_season_tick(t: &Tables, county: &mut County, season: Season, advanced_farming: bool) {
    match season {
        Season::Spring => sow(t, county, advanced_farming),
        Season::Summer => grow(county, 1),
        Season::Autumn => grow(county, 2),
        Season::Winter => harvest(county),
    }
    if county.event_grain_pct != 0 {
        county.grain += pct(county.grain, county.event_grain_pct);
        county.event_grain_pct = 0;
    }
    county.grain = county.grain.max(0);
}

// ---------------------------------------------------------------------------
// Livestock
// ---------------------------------------------------------------------------

/// `Herd_SeasonTick` (`0x0044D60D`) — the weather's percentage swing on the
/// herd, plus the random-event modifier.
///
/// The event modifier is tested for the sentinel **before** it is tested for
/// its sign: `if (mod == 99) { change = 0; births = 0; }`. So 99 is not
/// "+99%", it is *"Cattle will not reproduce this season"* — the *"No bull"*
/// event, `L2.eng` group 314. See [`crate::event::HERD_NO_GROWTH`].
///
/// The order matters: the sentinel suppresses the **weather** swing as well as
/// the event's own, so a sunny season and a dead prize bull cancel out.
pub fn herd_season_tick(t: &Tables, county: &mut County) {
    let weather_change =
        pct(county.herd, t.weather[county.weather.index() as usize].herd_pct);
    if county.event_herd_pct == crate::event::HERD_NO_GROWTH {
        county.event_herd_pct = 0;
        county.herd = county.herd.max(0);
        return;
    }
    county.herd += weather_change;
    if county.event_herd_pct != 0 {
        county.herd += pct(county.herd, county.event_herd_pct);
        county.event_herd_pct = 0;
    }
    county.herd = county.herd.max(0);
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
    /// county's fertility in the shipped save is 0.
    #[test]
    fn basic_farming_pins_fertility_at_zero() {
        let mut c = County::new();
        c.fields_fallow = 10;
        c.fields_grain = 1;
        update_fertility(&mut c, false);
        assert_eq!(c.fertility, 0);
    }

    #[test]
    fn only_fields_already_started_are_reclaimed() {
        let mut c = County::new();
        c.field_progress[0] = 0; // never started
        c.field_progress[1] = 100; // under way
        c.field_progress[2] = 800; // done
        reclaim_fields(T, &mut c);
        assert_eq!(c.field_progress[0], 0);
        assert_eq!(c.field_progress[1], 300);
        assert_eq!(c.field_progress[2], 800);
    }

    #[test]
    fn a_field_under_way_finishes_in_four_seasons_at_most() {
        let mut c = County::new();
        c.field_progress[5] = 1;
        for _ in 0..4 {
            reclaim_fields(T, &mut c);
        }
        assert_eq!(c.field_progress[5], T.field.progress_max as u16);
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
    #[test]
    fn a_full_year_of_grain_turns_each_sack_into_twelve() {
        let mut c = County::new();
        c.fields_grain = 6;
        c.grain = 200;
        c.labour[T.job.grain_farming] = 10_000;
        c.weather = Weather::Cloudy;

        grain_season_tick(T, &mut c, Season::Spring, true);
        assert_eq!(c.grain, 140, "60 sacks of seed spent");
        assert_eq!(c.crop[0], 720);

        grain_season_tick(T, &mut c, Season::Summer, true);
        assert_eq!(c.crop[1], 720);
        grain_season_tick(T, &mut c, Season::Autumn, true);
        assert_eq!(c.crop[2], 720);
        grain_season_tick(T, &mut c, Season::Winter, true);
        assert_eq!(c.grain, 860, "140 + 720");
        assert_eq!(c.crop, [0; 3]);
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

    /// Only *Sunny* grows the herd and only *Cloudy* leaves it alone.
    #[test]
    fn the_herd_follows_the_weather_table_exactly() {
        for w in Weather::ALL {
            let mut c = County::new();
            c.herd = 1000;
            c.weather = w;
            herd_season_tick(T, &mut c);
            assert_eq!(c.herd, 1000 + T.weather[w.index() as usize].herd_pct * 10, "{}", w.name());
        }
    }

    #[test]
    fn wolves_take_their_percentage_and_the_modifier_is_consumed() {
        let mut c = County::new();
        c.herd = 200;
        c.weather = Weather::Cloudy;
        c.event_herd_pct = -25; // "taken by wolves"
        herd_season_tick(T, &mut c);
        assert_eq!(c.herd, 150);
        assert_eq!(c.event_herd_pct, 0);
    }

    #[test]
    fn a_herd_of_one_survives_a_bad_season_because_percentages_round_to_zero() {
        let mut c = County::new();
        c.herd = 1;
        c.weather = Weather::Drought;
        herd_season_tick(T, &mut c);
        assert_eq!(c.herd, 1, "Pct(1, -10) truncates to 0");
    }

    #[test]
    fn a_factor_is_an_exact_ratio_rather_than_a_rounded_percentage() {
        assert_eq!(Factor(3, 2).apply(7), 10, "not 7 * 150% rounded twice");
        assert_eq!(Factor(1, 4).apply(7), 1);
        assert_eq!(Factor::NONE.apply(12345), 12345);
    }
}
