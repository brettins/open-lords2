//! Weather — `docs/kingdom.md` §7.3, `Weather_UpdateAll` (`0x00449889`).
//!
//! ```text
//! delta = {Spring: 8, Summer: 24, Autumn: 12, Winter: -12}[g_season] - random/8;
//! for every county:            dryness += delta;
//! pick one county c:           dryness[c] += delta + localModifier(c);
//! for each neighbour n of c:   dryness[n] += delta/2 + localModifier(n);
//!
//! band =  dryness <  5 ? Flooding   /* and dryness is pulled up to 30 */
//!       : dryness < 20 ? Storms
//!       : dryness < 70 ? Cloudy
//!       : dryness < 95 ? Sunny
//!       :                Drought;   /* and dryness is pulled down to 70 */
//! if (g_season == Winter || g_season == Spring) {
//!     if (band == Drought) band = Frost;
//!     else if (band == Sunny && dryness > 74) band = Frost;
//! }
//! if (!g_optAdvancedFarming) band = Cloudy;
//! ```
//!
//! **Weather is regional, not per-county**: every county gets the same seasonal
//! push, and one randomly chosen county plus its neighbours get an extra swing.
//! The clamps at the two extremes are mean reversion — a flood pulls the county
//! back to 30, a drought back to 70.
//!
//! A player observation falls straight out of it: *"Floods happen during winter
//! and spring, while droughts happen during summer"*. Dryness rises fastest in
//! Summer and falls only in Winter, and Drought is rewritten to Frost in Winter
//! and Spring so it cannot be *seen* in those two seasons at all.
//!
//! # `random/8`, resolved
//!
//! `docs/kingdom.md` §7.3 writes the jitter as `random/8` and gives no range,
//! which `docs/decisions.md` lists as an open question and which made
//! [`WEATHER_JITTER_BOUND`] the one invented constant in this crate. It is not
//! invented any more.
//!
//! The original's generator is `FUN_00404A46`, called once at the top of
//! `Season_Advance`. It steps **two 31-bit LFSRs** — taps at bits 0 and 4,
//! feeding bit 30, thirty-one iterations a call — and then publishes six masked
//! values from them. `Weather_UpdateAll` reads two:
//!
//! * the jitter is `(LFSR_B & 0x7F) >> 3`, so **`random` is 0..=127 and the
//!   jitter is 0..=15**;
//! * the county that gets the local swing is `(LFSR_A & 0x7F) & 0xF`, i.e.
//!   0..=15, with three fallbacks when that misses — see [`chosen_county`].
//!
//! So the jitter really can cancel Spring's +8 and Autumn's +12 outright, and
//! bites two thirds of the way into Summer's +24. `[V]`
//!
//! **This crate keeps `l2_net::Pcg32` rather than porting the two LFSRs.** The
//! *range* is what a rule turns on and it is the original's now; the exact bit
//! sequence is not, and it would only matter for a save-state-level
//! differential test, which would need the LFSR seeds out of a running game
//! anyway.
//!
//! **`localModifier(c)` is still not traced.** It is `FUN_00449D6E`, called for
//! the chosen county and for each of its neighbours, and it is zero here.

use crate::county::County;
use crate::math::clamp;
use crate::tables::{Season, Tables, Weather};
use l2_net::Pcg32;

/// The dryness accumulator is a signed byte in the original (county `+0x21D`),
/// so it is clamped to that range on every write. See [`County::dryness`].
pub const DRYNESS_MIN: i32 = i8::MIN as i32;
pub const DRYNESS_MAX: i32 = i8::MAX as i32;

/// Mean reversion: a flood pulls the county back to 30 and a drought back
/// to 70.
pub const DRYNESS_AFTER_FLOOD: i32 = 30;
pub const DRYNESS_AFTER_DROUGHT: i32 = 70;

/// The band ladder's four thresholds, wettest first.
pub const DRYNESS_LADDER: [i32; 4] = [5, 20, 70, 95];

/// The *Sunny* reading above which Winter and Spring rewrite the band to
/// *Frost*.
pub const FROST_SUNNY_THRESHOLD: i32 = 74;

/// The range `random` in `random/8` actually has: `LFSR & 0x7F`, so 0..=127.
///
/// **No longer an invention.** This was the one constant in the crate with no
/// evidence behind it — `docs/decisions.md` records it as an open question — and
/// it was 64. The generator (`FUN_00404A46`) masks its two LFSRs with `0x7F`,
/// and `Weather_UpdateAll` shifts the result right by three, so the jitter is
/// **0..=15**, twice the range this crate had guessed.
pub const WEATHER_JITTER_BOUND: u32 = 128;

/// The shift the original applies to the draw: `>> 3`, spelled out because it
/// is the `/8` in `docs/kingdom.md` §7.3's `random/8`.
pub const WEATHER_JITTER_SHIFT: u32 = 3;

/// The mask the original applies to the *other* LFSR to pick the county that
/// gets the local swing: `& 0xF`, so 0..=15.
pub const WEATHER_COUNTY_MASK: u32 = 0xF;

/// **Never traced.** `Weather_UpdateAll` adds a per-county modifier to the
/// chosen county and to each of its neighbours; it is `FUN_00449D6E`, and
/// `docs/kingdom.md` §7.3 names it and nothing more. Zero until somebody reads
/// it out of the binary.
pub fn local_modifier(_county: &County) -> i32 {
    0
}

/// The season's push on every county's dryness, jitter included.
///
/// The jitter is `draw >> 3` of a 0..=127 draw, which is 0..=15 — enough to
/// cancel Spring and Autumn outright and to take two thirds off Summer.
pub fn seasonal_delta(t: &Tables, season: Season, rng: &mut Pcg32) -> i32 {
    let jitter = (rng.below(WEATHER_JITTER_BOUND) >> WEATHER_JITTER_SHIFT) as i32;
    t.season[season.index() as usize].dryness - jitter
}

/// Which county gets the local swing.
///
/// ```c
/// c = g_seasonRandomA & 0xF;              /* 0 .. 15 */
/// if (c > g_countyCount) c = g_weatherCounty + 1;   /* last season's, plus one */
/// if (c > g_countyCount) c = 1;
/// if (c < 1)             c = 1;
/// g_weatherCounty = c;
/// ```
///
/// **`[V]`, and it is not a uniform draw over the counties.** The mask is a
/// flat 0..=15 whatever the map's county count is, so on a 14-county map the
/// two out-of-range values fall through to *"the county after last season's"* —
/// which makes the local swing walk steadily around the map about an eighth of
/// the time, rather than jumping. `previous` is `g_weatherCounty`
/// (`0x00554020`), which is its own four-byte save block.
pub fn chosen_county(draw: u32, county_count: usize, previous: usize) -> usize {
    let mut c = (draw & WEATHER_COUNTY_MASK) as usize;
    if c > county_count {
        c = previous + 1;
    }
    if c > county_count {
        c = 1;
    }
    if c < 1 {
        c = 1;
    }
    c
}

/// Band a dryness reading, applying the two mean-reversion clamps in place.
pub fn band(dryness: &mut i32) -> Weather {
    if *dryness < DRYNESS_LADDER[0] {
        *dryness = DRYNESS_AFTER_FLOOD;
        Weather::Flooding
    } else if *dryness < DRYNESS_LADDER[1] {
        Weather::Storms
    } else if *dryness < DRYNESS_LADDER[2] {
        Weather::Cloudy
    } else if *dryness < DRYNESS_LADDER[3] {
        Weather::Sunny
    } else {
        *dryness = DRYNESS_AFTER_DROUGHT;
        Weather::Drought
    }
}

/// The cold-season rewrite. `dryness` is the reading the band came from,
/// **before** the drought clamp would have moved it — which does not matter,
/// because the only band the threshold applies to is Sunny and the clamp only
/// touches Drought.
pub fn apply_frost(band: Weather, dryness: i32, season: Season) -> Weather {
    if season != Season::Winter && season != Season::Spring {
        return band;
    }
    match band {
        Weather::Drought => Weather::Frost,
        Weather::Sunny if dryness > FROST_SUNNY_THRESHOLD => Weather::Frost,
        other => other,
    }
}

/// `Weather_UpdateAll` — one regional push, one local swing, then band
/// everybody.
///
/// The randomly chosen county is drawn once from the shared generator, so the
/// number of draws this pass makes is `1 + 1` regardless of the kingdom's size
/// (`docs/netcode.md` §3: the stream must not depend on how much work there was
/// to do).
pub fn update_all(
    t: &Tables,
    counties: &mut [County],
    county_count: usize,
    season: Season,
    advanced_farming: bool,
    rng: &mut Pcg32,
    previous_county: &mut usize,
) {
    if county_count == 0 {
        return;
    }

    let delta = seasonal_delta(t, season, rng);
    // The original draws from its *other* LFSR here; one draw either way, so
    // the stream's length is unchanged.
    let chosen = chosen_county(rng.below(WEATHER_JITTER_BOUND), county_count, *previous_county);
    *previous_county = chosen;

    for id in 1..=county_count {
        push(&mut counties[id], delta);
    }

    let local = delta + local_modifier(&counties[chosen]);
    push(&mut counties[chosen], local);

    let neighbours: Vec<u8> = counties[chosen].neighbours().to_vec();
    for n in neighbours {
        let n = n as usize;
        if n == 0 || n > county_count {
            continue;
        }
        let swing = delta / 2 + local_modifier(&counties[n]);
        push(&mut counties[n], swing);
    }

    for id in 1..=county_count {
        let mut dryness = counties[id].dryness;
        let b = band(&mut dryness);
        counties[id].dryness = clamp(dryness, DRYNESS_MIN, DRYNESS_MAX);
        let b = apply_frost(b, counties[id].dryness, season);
        // "and every county's weather byte is 3 (Cloudy)" - docs/kingdom.md §9
        // point 3, which is exactly what this override forces.
        counties[id].weather = if advanced_farming { b } else { Weather::Cloudy };
    }
}

fn push(county: &mut County, by: i32) {
    county.dryness = clamp(county.dryness + by, DRYNESS_MIN, DRYNESS_MAX);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The stock ruleset. Every rule below takes it as an argument now.
    const T: &Tables = &Tables::DEFAULT;

    fn kingdom(n: usize, dryness: i32) -> Vec<County> {
        let mut c = vec![County::new(); n + 1];
        for id in 1..=n {
            c[id].dryness = dryness;
        }
        c
    }

    #[test]
    fn every_band_boundary_lands_where_the_ladder_says() {
        let cases = [
            (-100, Weather::Flooding),
            (4, Weather::Flooding),
            (5, Weather::Storms),
            (19, Weather::Storms),
            (20, Weather::Cloudy),
            (69, Weather::Cloudy),
            (70, Weather::Sunny),
            (94, Weather::Sunny),
            (95, Weather::Drought),
            (127, Weather::Drought),
        ];
        for (d, expected) in cases {
            let mut dryness = d;
            assert_eq!(band(&mut dryness), expected, "dryness {d}");
        }
    }

    #[test]
    fn the_two_extremes_pull_the_accumulator_back_towards_the_middle() {
        let mut dry = 0;
        assert_eq!(band(&mut dry), Weather::Flooding);
        assert_eq!(dry, DRYNESS_AFTER_FLOOD, "a flood reverts to 30");

        let mut dry = 120;
        assert_eq!(band(&mut dry), Weather::Drought);
        assert_eq!(dry, DRYNESS_AFTER_DROUGHT, "a drought reverts to 70");

        let mut dry = 50;
        band(&mut dry);
        assert_eq!(dry, 50, "the middle bands leave it alone");
    }

    /// *"Droughts happen during summer"* — and cannot be seen in Winter or
    /// Spring at all, because they are rewritten to Frost.
    #[test]
    fn a_drought_is_invisible_in_the_two_cold_seasons() {
        for season in Season::ALL {
            let cold = season == Season::Winter || season == Season::Spring;
            assert_eq!(
                apply_frost(Weather::Drought, 95, season),
                if cold { Weather::Frost } else { Weather::Drought },
                "{}",
                season.name()
            );
        }
    }

    #[test]
    fn a_dry_sunny_winter_reads_as_frost_and_a_damp_one_does_not() {
        assert_eq!(apply_frost(Weather::Sunny, 75, Season::Winter), Weather::Frost);
        assert_eq!(apply_frost(Weather::Sunny, 74, Season::Winter), Weather::Sunny);
        assert_eq!(apply_frost(Weather::Sunny, 90, Season::Summer), Weather::Sunny);
    }

    #[test]
    fn the_cold_rewrite_never_touches_the_wet_bands() {
        for w in [Weather::Flooding, Weather::Storms, Weather::Cloudy] {
            for season in Season::ALL {
                assert_eq!(apply_frost(w, 0, season), w);
            }
        }
    }

    /// **`docs/kingdom.md` §9 point 3.** With Advanced Farming off, every
    /// county's weather byte in the shipped save is 3 (Cloudy).
    #[test]
    fn basic_farming_forces_cloudy_everywhere_whatever_the_accumulator_says() {
        let mut rng = Pcg32::from_seed(1);
        let mut c = kingdom(14, 200);
        update_all(T, &mut c, 14, Season::Winter, false, &mut rng, &mut 1);
        for id in 1..=14 {
            assert_eq!(c[id].weather, Weather::Cloudy, "county {id}");
        }
    }

    /// Weather is *regional*: the seasonal push is identical for every county,
    /// so a kingdom that starts uniform stays uniform except for the one
    /// chosen county and its neighbours.
    #[test]
    fn the_seasonal_push_is_the_same_for_every_county() {
        let mut rng = Pcg32::from_seed(7);
        let mut c = kingdom(6, 50);
        // No adjacency, so only the chosen county diverges.
        update_all(T, &mut c, 6, Season::Summer, true, &mut rng, &mut 1);
        let mut readings: Vec<i32> = (1..=6).map(|i| c[i].dryness).collect();
        readings.sort_unstable();
        readings.dedup();
        assert_eq!(readings.len(), 2, "one county swung twice; the rest moved together");
    }

    /// The local swing: the chosen county gets `2 x delta`, each of its
    /// neighbours `delta + delta/2`. Tested on a fully connected kingdom, so
    /// the answer does not depend on *which* county was drawn.
    #[test]
    fn the_chosen_county_swings_twice_and_its_neighbours_by_half_again() {
        let mut rng = Pcg32::from_seed(3);
        let mut c = kingdom(3, 20);
        for a in 1..=3u8 {
            for b in 1..=3u8 {
                if a != b {
                    c[a as usize].add_neighbour(b);
                }
            }
        }
        // Recover the delta this pass will use without disturbing the stream.
        let delta = seasonal_delta(T, Season::Summer, &mut rng.clone());
        assert!(delta > 0);

        update_all(T, &mut c, 3, Season::Summer, true, &mut rng, &mut 1);

        let readings: Vec<i32> = (1..=3).map(|i| c[i].dryness).collect();
        let chosen = 20 + 2 * delta;
        let neighbour = 20 + delta + delta / 2;
        assert_eq!(readings.iter().filter(|&&d| d == chosen).count(), 1);
        assert_eq!(readings.iter().filter(|&&d| d == neighbour).count(), 2);
    }

    /// Summer dries the land out fastest and Winter is the only season that
    /// wets it — the shape behind *"floods in winter and spring, droughts in
    /// summer"*.
    #[test]
    fn only_winter_pushes_the_accumulator_downwards_on_average() {
        let mut rng = Pcg32::from_seed(11);
        let mut totals = [0i64; 5];
        for _ in 0..2000 {
            for season in Season::ALL {
                totals[season.index() as usize] += seasonal_delta(T, season, &mut rng) as i64;
            }
        }
        assert!(totals[Season::Winter as usize] < 0, "Winter should wet the land");
        for season in [Season::Spring, Season::Summer, Season::Autumn] {
            assert!(totals[season.index() as usize] > 0, "{} should dry it", season.name());
        }
    }

    /// **The jitter is 0..=15 and it really can flip Spring and Autumn.**
    ///
    /// The previous version of this test asserted that it could not — the
    /// property [`WEATHER_JITTER_BOUND`] was *invented* to have. The binary
    /// says otherwise: `(LFSR & 0x7F) >> 3` is 0..=15, so Spring's +8 can come
    /// out at −7 and Autumn's +12 at −3. Only Summer's +24 is safe.
    #[test]
    fn the_jitter_can_flip_spring_and_autumn_but_never_summer() {
        let mut rng = Pcg32::from_seed(99);
        let mut spring_flipped = false;
        let mut autumn_flipped = false;
        for _ in 0..4000 {
            let spring = seasonal_delta(T, Season::Spring, &mut rng);
            let summer = seasonal_delta(T, Season::Summer, &mut rng);
            let autumn = seasonal_delta(T, Season::Autumn, &mut rng);
            let winter = seasonal_delta(T, Season::Winter, &mut rng);
            // The exact bounds: push - 15 .. push.
            assert!((-7..=8).contains(&spring), "spring {spring}");
            assert!((9..=24).contains(&summer), "summer {summer}");
            assert!((-3..=12).contains(&autumn), "autumn {autumn}");
            assert!((-27..=-12).contains(&winter), "winter {winter}");
            spring_flipped |= spring < 0;
            autumn_flipped |= autumn < 0;
        }
        assert!(spring_flipped, "a wet spring is reachable");
        assert!(autumn_flipped, "so is a wet autumn");
    }

    /// The county picked for the local swing is masked to 0..=15 whatever the
    /// map holds, and the two fallbacks walk it on from last season's.
    #[test]
    fn an_out_of_range_draw_walks_the_local_swing_on_from_last_season() {
        // In range: the draw wins.
        assert_eq!(chosen_county(0x0A, 14, 3), 10);
        // 15 is beyond a 14-county map, so it takes last season's plus one.
        assert_eq!(chosen_county(0x0F, 14, 3), 4);
        // ... and if that is beyond the map too, county 1.
        assert_eq!(chosen_county(0x0F, 14, 14), 1);
        // A masked zero is pulled up to 1 rather than indexing the unused
        // record.
        assert_eq!(chosen_county(0x10, 14, 3), 1, "0x10 & 0xF is 0");
        // Every possible draw lands inside the map.
        for count in 1..=16usize {
            for draw in 0..128u32 {
                let c = chosen_county(draw, count, count);
                assert!((1..=count).contains(&c), "count {count} draw {draw} gave {c}");
            }
        }
    }

    /// The determinism property that matters: the same seed and the same
    /// kingdom give the same weather, every time.
    #[test]
    fn the_same_seed_produces_the_same_weather_every_run() {
        let run = || {
            let mut rng = Pcg32::from_seed(0xC0FFEE);
            let mut c = kingdom(8, 40);
            for id in 1..=8 {
                c[id].add_neighbour((id % 8 + 1) as u8);
            }
            for season in [Season::Spring, Season::Summer, Season::Autumn, Season::Winter] {
                update_all(T, &mut c, 8, season, true, &mut rng, &mut 1);
            }
            c
        };
        assert_eq!(run(), run());
    }

    /// The number of draws must not depend on the size of the kingdom, or two
    /// peers with different county counts would diverge in the shared stream.
    #[test]
    fn the_pass_draws_exactly_twice_whatever_the_kingdom_size() {
        for n in [1usize, 5, 16] {
            let mut rng = Pcg32::from_seed(42);
            let mut c = kingdom(n, 50);
            update_all(T, &mut c, n, Season::Summer, true, &mut rng, &mut 1);
            let mut reference = Pcg32::from_seed(42);
            reference.advance(2);
            assert_eq!(rng, reference, "kingdom of {n} should have drawn twice");
        }
    }

    #[test]
    fn an_empty_kingdom_draws_nothing_and_does_nothing() {
        let mut rng = Pcg32::from_seed(5);
        let before = rng.clone();
        let mut c = kingdom(0, 0);
        update_all(T, &mut c, 0, Season::Spring, true, &mut rng, &mut 1);
        assert_eq!(rng, before);
    }
}
