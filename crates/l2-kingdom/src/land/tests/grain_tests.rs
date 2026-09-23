#![allow(unused_imports)]
use super::*;
use super::fertility_tests::*;
use super::herd_tests::*;
use super::crowding::*;
use super::super::*;

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


