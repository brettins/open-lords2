
mod fertility;
pub use fertility::*;
mod reclaim;
pub use reclaim::*;
mod grain;
pub use grain::*;
mod herd;
pub use herd::*;

use crate::county::{County, MAX_FIELDS};
use crate::field::FieldType;
use crate::math::{clamp, pct, pct_of, per_myriad};
use crate::tables::{HerdCrowdingRow, Season, Tables, Weather, HERD_CROWDING_COUNT};
use l2_net::{Quirk, Quirks};


/// The fertility scale's bounds. `L2.eng` group 22 names seven levels from
/// *"Infertile — almost no production"* to *"Excellent fertility — bumper
/// crop!"*, but `docs/kingdom.md` §7.2 could not trace the mapping from this
/// scalar to those seven, so this crate does not invent one.
pub const FERTILITY_MIN: i32 = -100;
pub const FERTILITY_MAX: i32 = 100;

pub const FERTILITY_PER_FALLOW: i32 = 6;
pub const FERTILITY_PER_GRAIN: i32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Factor(pub i32, pub i32);

impl Factor {
    pub const NONE: Factor = Factor(1, 1);

    pub fn apply(self, value: i32) -> i32 {
        ((value as i64 * self.0 as i64) / self.1 as i64) as i32
    }
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrainEstimate {
    /// `+0xC8` — the wanted floor, or [`crate::county::LABOUR_NO_FLOOR`].
    pub wanted: i32,
    /// `+0xCC` — the useful ceiling, the only one `Labour_Allocate` reads.
    pub useful: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HerdEstimate {
    /// `+0xD4` — the break-even staffing, below which the count is drawn red.
    pub wanted: i32,
    /// `+0xD8` — the growth-maximising staffing, the only one
    /// `Labour_Allocate` reads.
    pub useful: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    const Q: Quirks = Quirks::FAITHFUL;

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

    #[test]
    fn basic_farming_pins_fertility_at_zero() {
        let mut c = County::new();
        c.fields_fallow = 10;
        c.fields_grain = 1;
        update_fertility(&mut c, false);
        assert_eq!(c.fertility, 0);
    }

    fn reclaiming(slots: &[(usize, u16)], workers: i32) -> (County, crate::map::CampaignMap) {
        let mut map = crate::map::CampaignMap::empty();
        let mut c = County::new();
        c.labour[crate::tables::JOB_FIELD_RECLAMATION] = workers;
        for slot in 0..MAX_FIELDS {
            c.field_tiles[slot] = slot as u16 + 1;
            map.terrain[slot + 1] = crate::field::terrain::FALLOW;
        }
        for &(slot, progress) in slots {
            c.field_progress[slot] = progress;
            map.terrain[slot + 1] = reclaim_terrain(T, progress as i32);
        }
        (c, map)
    }

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

    #[test]
    fn a_county_with_nobody_on_reclamation_reclaims_nothing() {
        let (mut c, mut map) = reclaiming(&[(0, 100), (3, 400)], 0);
        reclaim_fields(T, &mut c, &mut map);
        assert_eq!(c.field_progress[0], 100);
        assert_eq!(c.field_progress[3], 400);

        c.labour[crate::tables::JOB_FIELD_RECLAMATION] = 50;
        reclaim_fields(T, &mut c, &mut map);
        assert_eq!(c.field_progress[3], 450, "the most advanced field goes first");
        assert_eq!(c.field_progress[0], 100, "and the other gets nothing");
    }

    #[test]
    fn the_budget_walks_the_rota_from_the_leading_field() {
        let (mut c, mut map) = reclaiming(&[(0, 0), (7, 700)], 500);
        reclaim_fields(T, &mut c, &mut map);
        assert_eq!(c.field_progress[7], 900);
        assert_eq!(map.terrain[8], crate::field::terrain::FALLOW);
        assert_eq!(c.field_progress[0], 200);
    }

    #[test]
    fn a_well_supplied_county_sows_ten_sacks_a_field_not_five() {
        assert_eq!(sacks_per_field(T, 6, 10_000, 10_000, true), 10);
        assert_eq!(sacks_per_field(T, 9, 10_000, 10_000, true), 10);
        assert_eq!(6 * sacks_per_field(T, 6, 10_000, 10_000, true), 60);
        assert_eq!(9 * sacks_per_field(T, 9, 10_000, 10_000, true), 90);
    }

    #[test]
    fn the_seed_store_is_the_binding_constraint_when_labour_is_plentiful() {
        assert_eq!(sacks_per_field(T, 6, 30, 100_000, true), 5);
        assert_eq!(sacks_per_field(T, 6, 29, 100_000, true), 4);
        assert_eq!(sacks_per_field(T, 6, 5, 100_000, true), 0, "not even one a field");
    }

    #[test]
    fn advanced_farming_needs_less_labour_for_the_same_sowing() {
        for fields in 1..=16 {
            let advanced = sacks_per_field(T, fields, 10_000, 200, true);
            let basic = sacks_per_field(T, fields, 10_000, 200, false);
            assert!(advanced >= basic, "{fields} fields: {advanced} vs {basic}");
        }
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

    #[test]
    fn a_full_year_of_grain_turns_each_sack_into_twelve() {
        let mut c = County::new();
        c.fields_grain = 6;
        c.grain = 200;
        c.labour[T.job.grain_farming] = 10_000;
        c.weather = Weather::Cloudy;

        grain_season_tick(T, &mut c, Season::Spring, true, Q);
        assert_eq!(c.grain, 140, "60 sacks of seed spent");
        assert_eq!(c.crop[0], 60, "the seed, not the crop");
        assert_eq!(c.crop[1], 720, "and the crop is the seed times twelve");
        assert_eq!(c.fields_grain_sown, 6);

        grain_season_tick(T, &mut c, Season::Summer, true, Q);
        assert_eq!(c.crop[1], 720);
        assert_eq!(c.crop[2], 0, "nothing is harvested in summer");
        grain_season_tick(T, &mut c, Season::Autumn, true, Q);
        assert_eq!(c.crop[1], 720);
        grain_season_tick(T, &mut c, Season::Winter, true, Q);
        assert_eq!(c.crop[2], 720, "and this is the harvest");
        assert_eq!(c.grain, 860, "140 + 720");
    }

    /// **The wheat picture reads the standing crop for three seasons and the
    /// harvest for one, and divides by `+0x206`** — `Grain_SeasonTick`'s three
    /// `FUN_0044CF6F` calls (`0x0044C8AE`).
    ///
    /// The expected bands are literals from `FUN_0044CF6F`'s thresholds (41 and
    /// 81 sacks a field), not recomputed through [`grain_crop_band`]. The case
    /// that fails the first fix is the first one: `crop[2]` is always zero in
    /// Spring, so banding it answers 2 for a crop of 480 sacks on 6 fields.
    #[test]
    fn the_wheat_is_banded_by_this_seasons_crop_word_over_the_fields_still_standing() {
        let mut c = County::new();
        c.fields_grain = 6;
        c.fields_grain_standing = 6;
        c.crop = [40, 480, 0];
        for season in [Season::Spring, Season::Summer, Season::Autumn] {
            assert_eq!(grain_stage_band(&c, season), 7, "{season:?} reads crop[1]");
        }
        assert_eq!(grain_stage_band(&c, Season::Winter), 2, "Winter reads crop[2], and it is 0");
        c.crop[2] = 486;
        assert_eq!(grain_stage_band(&c, Season::Winter), 11);

        // **The divisor is `+0x206`, not `fieldsGrain`.** Paint two more grain
        // fields after sowing: `fieldsGrain` rises, `+0x206` does not, and the
        // picture is unchanged. Banded by `fieldsGrain` it would fall to 3.
        c.fields_grain = 8;
        assert_eq!(grain_stage_band(&c, Season::Summer), 7, "480 / 6, not 480 / 8");
        c.fields_grain_standing = 5;
        assert_eq!(grain_stage_band(&c, Season::Summer), 11);
        c.fields_grain_standing = 0x100 + 6;
        assert_eq!(grain_stage_band(&c, Season::Summer), 7);
    }

    /// Sowing writes `+0x206` beside `+0x202`, including the shortfall's `1`.
    #[test]
    fn sowing_writes_the_standing_count_beside_the_sown_one() {
        let mut c = County::new();
        c.fields_grain = 6;
        c.grain = 200;
        c.labour[T.job.grain_farming] = 10_000;
        c.weather = Weather::Cloudy;
        grain_season_tick(T, &mut c, Season::Spring, true, Q);
        assert_eq!((c.fields_grain_sown, c.fields_grain_standing), (6, 6));

        let mut poor = County::new();
        poor.fields_grain = 6;
        poor.grain = 3; // not one sack a field
        poor.labour[T.job.grain_farming] = 10_000;
        poor.weather = Weather::Cloudy;
        grain_season_tick(T, &mut poor, Season::Spring, true, Q);
        assert!(poor.sow_shortfall, "the token handful");
        assert_eq!((poor.fields_grain_sown, poor.fields_grain_standing), (1, 1));
    }

    /// **`FUN_00469D21`'s shortfall arm (`0x00469D21`)**: with `+0x1A7` set, the
    /// first grain tile of the county in tile-index order takes the crop's band
    /// and every later one is repainted `2`. A tile of another county, and a
    /// pasture of this one, are outside the sweep's tests and keep their bytes.
    #[test]
    fn a_short_sowing_paints_the_crop_on_the_first_grain_tile_and_bare_on_the_rest() {
        use crate::map::{flags, index, CampaignMap};
        let mut map = CampaignMap::empty();
        let grain = [index(10, 10), index(11, 10), index(10, 11)];
        for &t in &grain {
            map.county[t] = 3;
            map.flags[t] = flags::FARMLAND;
            map.terrain[t] = 2;
        }
        let foreign = index(12, 12);
        map.county[foreign] = 4;
        map.flags[foreign] = flags::FARMLAND;
        map.terrain[foreign] = 2;
        let pasture = index(13, 13);
        map.county[pasture] = 3;
        map.flags[pasture] = flags::FARMLAND;
        map.terrain[pasture] = 0x14;

        let mut c = County::new();
        c.crop = [5, 60, 0];
        c.fields_grain_standing = 1; // the shortfall records one field
        c.sow_shortfall = true;
        grain_repaint_fields(3, &c, Season::Spring, &mut map);
        assert_eq!(map.terrain[grain[0]], 7, "the first grain tile takes the crop");
        assert_eq!((map.terrain[grain[1]], map.terrain[grain[2]]), (2, 2), "the rest are bare");
        assert_eq!(map.terrain[foreign], 2, "another county's field is not swept");
        assert_eq!(map.terrain[pasture], 0x14, "a pasture is outside 2 ..= 0x0E");

        c.sow_shortfall = false;
        grain_repaint_fields(3, &c, Season::Spring, &mut map);
        assert_eq!(grain.map(|t| map.terrain[t]), [7, 7, 7], "without the flag every one does");
    }

    #[test]
    fn a_crop_nobody_tends_comes_to_nothing() {
        let sow_and_run = |grow_hands: i32| {
            let mut c = County::new();
            c.fields_grain = 6;
            c.grain = 200;
            c.weather = Weather::Cloudy;
            c.labour[T.job.grain_farming] = 10_000;
            grain_season_tick(T, &mut c, Season::Spring, true, Q);
            assert_eq!(c.crop[1], 720);
            c.labour[T.job.grain_farming] = grow_hands;
            for season in [Season::Summer, Season::Autumn, Season::Winter] {
                grain_season_tick(T, &mut c, season, true, Q);
            }
            c.crop[2]
        };
        assert_eq!(sow_and_run(0), 0, "nobody tends it, nobody reaps it");
        assert_eq!(sow_and_run(10), 15);
        assert_eq!(sow_and_run(10_000), 720, "and a full workforce loses nothing");
    }

    #[test]
    fn fertility_multiplies_the_crop_twice_a_year() {
        let year = |fertility: i32| {
            let mut c = County::new();
            c.fields_grain = 6;
            c.grain = 200;
            c.weather = Weather::Cloudy;
            c.labour[T.job.grain_farming] = 10_000;
            grain_season_tick(T, &mut c, Season::Spring, true, Q);
            c.fertility = fertility;
            for season in [Season::Summer, Season::Autumn, Season::Winter] {
                grain_season_tick(T, &mut c, season, true, Q);
            }
            c.crop[2]
        };
        assert_eq!(year(0), 720);
        assert_eq!(year(100), 1620);
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
        grain_season_tick(T, &mut c, Season::Spring, true, Q);
        assert_eq!(c.crop[1], 720);

        c.fields_grain = 3; // half the fields turned over to pasture
        grain_season_tick(T, &mut c, Season::Summer, true, Q);
        assert_eq!(c.crop[1], 360);

        c.fields_grain = 12;
        grain_season_tick(T, &mut c, Season::Autumn, true, Q);
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
                grain_season_tick(T, &mut c, season, true, Q);
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
    fn the_grain_tick_records_what_the_event_and_the_weather_did() {
        let mut c = County::new();
        c.grain = 1000;
        c.weather = Weather::Cloudy;
        c.event_grain_pct = -30; // rats
        grain_season_tick(T, &mut c, Season::Winter, true, Q);
        assert_eq!(c.grain_event_change, 300, "a magnitude, not -300");
        c.event_grain_pct = 20; // found as surplus
        grain_season_tick(T, &mut c, Season::Winter, true, Q);
        assert_eq!(c.grain_event_change, pct(700, 20));
        grain_season_tick(T, &mut c, Season::Winter, true, Q);
        assert_eq!(c.grain_event_change, 0, "zeroed by the tick that has no event");

        let mut c = County::new();
        c.fields_grain = 6;
        c.fields_grain_sown = 6;
        c.labour[T.job.grain_farming] = 10_000;
        c.crop[1] = 400;
        c.weather = Weather::Sunny;
        grain_season_tick(T, &mut c, Season::Summer, true, Q);
        let before = c.crop[1] - c.grain_weather_change;
        assert!(c.grain_weather_change > 0, "sunshine gains");
        assert_eq!(c.crop[1], before * 3 / 2, "after less before is the band's own effect");
        c.weather = Weather::Cloudy;
        grain_season_tick(T, &mut c, Season::Summer, true, Q);
        assert_eq!(c.grain_weather_change, 0, "Cloudy has no band");
    }

    #[test]
    fn the_grain_event_modifier_is_applied_and_consumed() {
        let mut c = County::new();
        c.grain = 1000;
        c.weather = Weather::Cloudy;
        c.event_grain_pct = -30; // "eaten by rats"
        grain_season_tick(T, &mut c, Season::Winter, true, Q);
        assert_eq!(c.grain, 700);
        assert_eq!(c.event_grain_pct, 0);
    }

    #[test]
    fn the_grain_store_never_goes_negative() {
        let mut c = County::new();
        c.grain = 10;
        c.event_grain_pct = -200;
        grain_season_tick(T, &mut c, Season::Winter, true, Q);
        assert_eq!(c.grain, 0);
    }


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
        //... and the ordering the table encodes is still the ordering.
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
    fn the_herd_tick_records_what_the_event_and_the_weather_did() {
        for w in Weather::ALL {
            let mut c = grazing(1000, 100, 3000);
            c.weather = w;
            herd_season_tick(T, &mut c, SUMMER, AUTUMN);
            assert_eq!(c.herd_weather_change, pct(1000, T.weather[w.index() as usize].herd_pct), "{}", w.name());
            assert_eq!(c.herd_event_change, 0, "{}: no event", w.name());
        }
        let mut c = grazing(200, 20, 600);
        c.event_herd_pct = -25; // wolves
        herd_season_tick(T, &mut c, SUMMER, AUTUMN);
        assert_eq!(c.herd_event_change, 50, "a magnitude, not -50");

        let mut c = grazing(1000, 100, 3000);
        c.weather = Weather::Sunny;
        c.event_herd_pct = crate::event::HERD_NO_GROWTH;
        herd_season_tick(T, &mut c, SUMMER, AUTUMN);
        assert_eq!(c.herd_weather_change, 0, "No Bull cancels the sun, and the figure says so");
        assert_eq!(c.herd_event_change, 0, "and has no figure of its own");
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

        let staffed = herd_growth(T, herd, fields, full, crowding, SUMMER);
        let abandoned = herd_growth(T, herd, fields, 0, crowding, SUMMER);
        assert_eq!(staffed.deaths, 1, "1 per 10,000 of 100 head");
        assert_eq!(abandoned.deaths, 34, "1 + (100 - 0) / 3 per 10,000, which is a third of it");
        assert_eq!(staffed.births, 14, "1,400 per 10,000");
        assert_eq!(abandoned.births, 0, "and a staffing of zero is a birth rate of zero");
        assert!(staffed.net() > 0 && abandoned.net() < 0, "the sign of the season flips");

        assert_eq!(herd_growth(T, herd, fields, full - 1, crowding, SUMMER).deaths, 1);
        assert_eq!(herd_growth(T, herd, fields, full / 3, crowding, SUMMER).deaths, 23);
    }

    #[test]
    fn the_staffing_benefit_stops_at_twice_the_workers() {
        let (herd, fields) = (10_000, 1_000);
        let crowding = herd_crowding(T, herd, fields);
        let at = |labour: i32| herd_growth(T, herd, fields, labour, crowding, SUMMER).births;
        let full = herd * T.herd.labour_per_head;
        assert!(at(2 * full) > at(full), "twice the workers is worth having");
        assert_eq!(at(2 * full), at(10 * full), "ten times over is not");
        let ninety_nine = full * 199 / 100;
        assert!(at(ninety_nine) < at(2 * full));
    }

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
        let crowding = herd_crowding(T, 25, 10);
        let plain = herd_growth(T, 25, 10, 75, crowding, SUMMER);
        assert_eq!(plain.births, per_myriad(25, 1400).max(1));
    }

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

    #[test]
    fn no_pasture_is_maximum_crowding_at_every_herd_size() {
        for herd in 0..=500 {
            assert_eq!(herd_crowding(T, herd, 0), 40, "herd {herd} on no pasture");
        }
        assert_eq!(herd_crowding(T, 0, 8), 10);
    }

    #[test]
    fn a_season_is_judged_at_last_seasons_crowding_and_then_recomputed() {
        let mut c = grazing(300, 10, 900); // density 30 -> band 30
        assert_eq!(c.herd_crowding, 30);
        c.fields_cattle = 100; // pasture bought: density would now be 3
        herd_season_tick(T, &mut c, SUMMER, AUTUMN);
        assert_eq!(c.herd_crowding, 10, "and the new pasture counts from next season");
    }

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

    #[test]
    fn an_unnamed_crowding_value_is_treated_as_the_worst_one() {
        let (herd, fields, labour) = (10_000, 1_000, 30_000);
        let worst = herd_growth(T, herd, fields, labour, 40, SUMMER);
        for odd in [0, 5, 15, 25, 35, 41, 1_000, -7] {
            assert_eq!(herd_growth(T, herd, fields, labour, odd, SUMMER), worst, "crowding {odd}");
        }
    }

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
        for season in [SUMMER, AUTUMN, 0] {
            assert_eq!(herd_growth(T, herd, fields, herd * 3, crowding, season), plain);
        }
    }

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
        assert_eq!(c.herd_change_expected, g.net() - 13, "the coming slaughter, off the net too");

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

