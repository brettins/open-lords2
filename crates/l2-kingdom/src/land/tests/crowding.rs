#![allow(unused_imports)]
use super::*;
use super::fertility_tests::*;
use super::grain_tests::*;
use super::herd_tests::*;
use super::super::*;

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

