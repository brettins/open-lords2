//! Weather — `docs/kingdom.md` §7.3, `Weather_UpdateAll` (`0x00449889`).
//!
//! The original's generator is `FUN_00404A46`, called once at the top of
//! `Season_Advance`. It steps **two 31-bit LFSRs** — taps at bits 0 and 4,
//! feeding bit 30, thirty-one iterations a call — and then publishes six masked
//! values from them. `Weather_UpdateAll` reads two:
//!
//! So the jitter really can cancel Spring's +8 and Autumn's +12 outright, and
//! bites two thirds of the way into Summer's +24. `[V]`
//!
//! It is `FUN_00449D6E` — [`local_modifier`] — and it is not zero any more.
//!
//! It reads county `+0x21E`, a **climate band 0…4**, and returns a swing that
//! depends on the band and on the season. The band is set once, in
//! `County_Reset` (`0x00451150`), **from the county's index and nothing else** —
//! see [`climate_band`] — and no other function in the binary writes it. So it
//! is a property of where a county sits in the scenario's own ordering, and on
//! most maps that ordering runs roughly north to south.
//!
//! Only the chosen county and its neighbours get it, so it is a *local* term on
//! top of the regional push, and it runs in **Summer and Winter only**. `[V]`

mod update;
pub use update::*;

use crate::county::County;
use crate::math::clamp;
use crate::tables::{Season, Tables, Weather};
use l2_net::Pcg32;

/// The dryness accumulator is a signed byte in the original (county `+0x21D`),
/// so it is clamped to that range on every write. See [`County::dryness`].
pub const DRYNESS_MIN: i32 = i8::MIN as i32;
pub const DRYNESS_MAX: i32 = i8::MAX as i32;

pub const DRYNESS_AFTER_FLOOD: i32 = 30;
pub const DRYNESS_AFTER_DROUGHT: i32 = 70;

pub const DRYNESS_LADDER: [i32; 4] = [5, 20, 70, 95];

pub const FROST_SUNNY_THRESHOLD: i32 = 74;

/// **No longer an invention.** This was the one constant in the crate with no
/// evidence behind it — `docs/decisions.md` records it as an open question — and
/// it was 64. The generator (`FUN_00404A46`) masks its two LFSRs with `0x7F`,
/// and `Weather_UpdateAll` shifts the result right by three, so the jitter is
/// **0..=15**, twice the range this crate had guessed.
pub const WEATHER_JITTER_BOUND: u32 = 128;

pub const WEATHER_JITTER_SHIFT: u32 = 3;

pub const WEATHER_COUNTY_MASK: u32 = 0xF;

#[cfg(test)]
mod tests {
    use super::*;

    const T: &Tables = &Tables::DEFAULT;

    fn kingdom(n: usize, dryness: i32) -> Vec<County> {
        let mut c = vec![County::new(); n + 1];
        for id in 1..=n {
            c[id].dryness = dryness;
        }
        c
    }

    fn blightable(dryness: i32) -> (Vec<County>, crate::map::CampaignMap) {
        let mut c = kingdom(1, dryness);
        let mut map = crate::map::CampaignMap::empty();
        for slot in 0..4 {
            let tile = crate::map::index(slot as u8, 8);
            c[1].set_field_tile(slot, Some(tile));
            map.terrain[tile] = crate::field::terrain::FALLOW;
        }
        c[1].owner = 1;
        (c, map)
    }

    /// A drought parches one field and tells the owner — `Msg_Enqueue(0x8F)`,
    /// `L2.eng` group 143 *"Drought."*.
    #[test]
    fn a_drought_parches_one_field_and_posts_group_143() {
        let (mut c, mut map) = blightable(DRYNESS_MAX);
        let mut msgs = Vec::new();
        update_all(
            T,
            &mut c,
            1,
            &mut map,
            Season::Summer,
            true,
            &mut Pcg32::new(7, 1),
            &mut 1,
            &|owner| owner == 1,
            &mut msgs,
        );
        assert_eq!(c[1].weather, Weather::Drought);
        let parched = (0..4)
            .filter(|&s| map.terrain[c[1].field_tile(s).unwrap()] == crate::field::terrain::PARCHED)
            .count();
        assert_eq!(parched, 1, "one field, not all four");
        assert_eq!(msgs, vec![crate::report::Message::Drought { county: 1 }]);
        assert_eq!(msgs[0].original_id(), Some(crate::report::MSG_DROUGHT));
    }

    /// …and a flood floods one, `Msg_Enqueue(0x90)`, group 144 *"Flooding."*.
    #[test]
    fn a_flood_floods_one_field_and_posts_group_144() {
        let (mut c, mut map) = blightable(DRYNESS_MIN);
        let mut msgs = Vec::new();
        update_all(
            T,
            &mut c,
            1,
            &mut map,
            Season::Summer,
            true,
            &mut Pcg32::new(7, 1),
            &mut 1,
            &|owner| owner == 1,
            &mut msgs,
        );
        assert_eq!(c[1].weather, Weather::Flooding);
        let flooded = (0..4)
            .filter(|&s| map.terrain[c[1].field_tile(s).unwrap()] == crate::field::terrain::FLOODED)
            .count();
        assert_eq!(flooded, 1);
        assert_eq!(msgs, vec![crate::report::Message::Flooding { county: 1 }]);
        assert_eq!(msgs[0].original_id(), Some(crate::report::MSG_FLOODING));
    }

    #[test]
    fn basic_farming_flattens_the_byte_and_still_ruins_the_field() {
        let (mut c, mut map) = blightable(DRYNESS_MIN);
        let mut msgs = Vec::new();
        update_all(
            T,
            &mut c,
            1,
            &mut map,
            Season::Summer,
            false,
            &mut Pcg32::new(7, 1),
            &mut 1,
            &|owner| owner == 1,
            &mut msgs,
        );
        assert_eq!(c[1].weather, Weather::Cloudy);
        assert_eq!(map.terrain[c[1].field_tile(1).unwrap()], crate::field::terrain::FLOODED);
        assert_eq!(msgs.len(), 1);
    }

    fn run(
        t: &Tables,
        counties: &mut [County],
        n: usize,
        season: Season,
        advanced_farming: bool,
        rng: &mut Pcg32,
        previous: &mut usize,
    ) {
        update_all(
            t,
            counties,
            n,
            &mut crate::map::CampaignMap::empty(),
            season,
            advanced_farming,
            rng,
            previous,
            &|_| false,
            &mut Vec::new(),
        );
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

    #[test]
    fn basic_farming_forces_cloudy_everywhere_whatever_the_accumulator_says() {
        let mut rng = Pcg32::from_seed(1);
        let mut c = kingdom(14, 200);
        run(T, &mut c, 14, Season::Winter, false, &mut rng, &mut 1);
        for id in 1..=14 {
            assert_eq!(c[id].weather, Weather::Cloudy, "county {id}");
        }
    }

    #[test]
    fn the_seasonal_push_is_the_same_for_every_county() {
        let mut rng = Pcg32::from_seed(7);
        let mut c = kingdom(6, 50);
        run(T, &mut c, 6, Season::Summer, true, &mut rng, &mut 1);
        let mut readings: Vec<i32> = (1..=6).map(|i| c[i].dryness).collect();
        readings.sort_unstable();
        readings.dedup();
        assert_eq!(readings.len(), 2, "one county swung twice; the rest moved together");
    }

    /// **This is the assertion that goes red if [`local_modifier`] returns to
    /// zero**, which is what it did until `FUN_00449D6E` was read: delete the
    /// `+ local_modifier(...)` from either arm of [`update_all`].
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
        let delta = seasonal_delta(T, Season::Summer, &mut rng.clone());
        assert!(delta > 0);
        let local = local_modifier(1, Season::Summer);
        assert_eq!(local, 4, "counties 1..3 are climate band 0");

        run(T, &mut c, 3, Season::Summer, true, &mut rng, &mut 1);

        let readings: Vec<i32> = (1..=3).map(|i| c[i].dryness).collect();
        let chosen = 20 + 2 * delta + local;
        let neighbour = 20 + delta + delta / 2 + local;
        assert_eq!(readings.iter().filter(|&&d| d == chosen).count(), 1, "{readings:?}");
        assert_eq!(readings.iter().filter(|&&d| d == neighbour).count(), 2, "{readings:?}");
    }

    #[test]
    fn the_climate_band_is_cut_out_of_the_county_index() {
        let expected = [
            (1, 0u8), (2, 0), (3, 0),
            (4, 1), (5, 1),
            (6, 2), (7, 2), (8, 2), (9, 2),
            (10, 3), (11, 3),
            (12, 4), (13, 4), (14, 4), (16, 4),
        ];
        for (id, band) in expected {
            assert_eq!(climate_band(id), band, "county {id}");
        }
    }

    /// **`FUN_00449D6E` in full, including the arm that cannot run.**
    #[test]
    fn the_summer_climate_ladder_skips_band_three_and_never_reaches_minus_24() {
        let summer = [(1usize, 4i32), (4, 2), (6, -8), (10, 0), (12, -12)];
        for (id, expected) in summer {
            assert_eq!(local_modifier(id, Season::Summer), expected, "county {id} in Summer");
        }
        assert!(
            (1..=16).all(|id| local_modifier(id, Season::Summer) != -24),
            "the -24 arm repeats the test above it and can never run"
        );

        let winter = [(1usize, 0i32), (4, -2), (6, -4), (10, -6), (12, -10)];
        for (id, expected) in winter {
            assert_eq!(local_modifier(id, Season::Winter), expected, "county {id} in Winter");
        }

        for season in [Season::Spring, Season::Autumn] {
            for id in 1..=16 {
                assert_eq!(local_modifier(id, season), 0, "{} county {id}", season.name());
            }
        }
    }

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

    #[test]
    fn an_out_of_range_draw_walks_the_local_swing_on_from_last_season() {
        assert_eq!(chosen_county(0x0A, 14, 3), 10);
        assert_eq!(chosen_county(0x0F, 14, 3), 4);
        assert_eq!(chosen_county(0x0F, 14, 14), 1);
        assert_eq!(chosen_county(0x10, 14, 3), 1, "0x10 & 0xF is 0");
        for count in 1..=16usize {
            for draw in 0..128u32 {
                let c = chosen_county(draw, count, count);
                assert!((1..=count).contains(&c), "count {count} draw {draw} gave {c}");
            }
        }
    }

    #[test]
    fn the_same_seed_produces_the_same_weather_every_run() {
        let run = || {
            let mut rng = Pcg32::from_seed(0xC0FFEE);
            let mut c = kingdom(8, 40);
            for id in 1..=8 {
                c[id].add_neighbour((id % 8 + 1) as u8);
            }
            for season in [Season::Spring, Season::Summer, Season::Autumn, Season::Winter] {
                run(T, &mut c, 8, season, true, &mut rng, &mut 1);
            }
            c
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn the_pass_draws_exactly_twice_whatever_the_kingdom_size() {
        for n in [1usize, 5, 16] {
            let mut rng = Pcg32::from_seed(42);
            let mut c = kingdom(n, 50);
            run(T, &mut c, n, Season::Summer, true, &mut rng, &mut 1);
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
        run(T, &mut c, 0, Season::Spring, true, &mut rng, &mut 1);
        assert_eq!(rng, before);
    }
}

