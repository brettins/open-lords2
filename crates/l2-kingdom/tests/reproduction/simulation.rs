#![allow(unused_imports)]
use super::*;
use super::helpers::*;
use super::reproduction::*;
use super::food_and_ration::*;
use super::population_and_labour::*;
use l2_formats::save::Save;
use l2_kingdom::county::County;
use l2_kingdom::phase::SEASON_PIPELINE;
use l2_kingdom::tables::{health_band, Season, Tables, Weather};
use l2_scenario::{Scenario, STARTING_HEALTH_METER};

#[test]
fn the_whole_five_stage_chain_lands_on_both_bands() {
    let s = england!();
    let file = s.kingdom(SEED);
    let k = kingdom_after_the_first_season(&s);

    for id in [8usize, 14] {
        let a = &k.counties[id];
        let b = &file.counties[id];
        assert_eq!((a.shown_ration, a.health_meter, a.health_band), (1, 67, 3), "county {id}");
        assert_eq!((b.shown_ration, b.health_meter, b.health_band), (1, 67, 3), "county {id}");
        assert_eq!((a.shown_health, a.happiness), (b.shown_health, b.happiness), "county {id}");
        assert_eq!((a.births, a.deaths, a.population), (b.births, b.deaths, b.population));
    }
    assert_eq!((file.counties[8].happiness, file.counties[8].shown_events), (72, 0));
    assert_eq!((file.counties[14].happiness, file.counties[14].shown_events), (77, 5));
    assert_eq!((file.counties[8].births, file.counties[14].births), (63, 84));
    assert_eq!((file.counties[8].pop_band, file.counties[14].pop_band), (18, 19));
}

#[test]
fn no_county_draws_an_event_in_the_first_year() {
    let s = england!();
    let mut k = s.starting_kingdom(SEED);
    k.start_new_game();
    for id in s.county_ids() {
        assert!(!k.counties[id].event_fired, "county {id}");
        assert_eq!(k.counties[id].event_population_pct, 0, "county {id}");
        assert_eq!(k.counties[id].event_id, 0, "county {id}");
    }
}

#[test]
fn the_tax_term_reproduces_across_the_whole_map() {
    let s = england!();
    let file = s.kingdom(SEED);
    let mut k = s.starting_kingdom(SEED);
    k.start_new_game();
    for id in s.county_ids() {
        assert_eq!(file.counties[id].tax_rate, 0, "the file, county {id}");
        assert_eq!(k.counties[id].tax_rate, 0, "county {id}");
        assert_eq!(k.counties[id].shown_tax, 5, "county {id}: 5 - 0");
        assert_eq!(k.counties[id].tax_collected, 0, "county {id}");
    }
    for id in 1..=5 {
        assert_eq!(k.realms[id].gold, 1000, "realm {id} banked nothing");
        assert_eq!(k.realms[id].gold, file.realms[id].gold, "realm {id}");
        assert_eq!(k.realms[id].tax_hap_empire, 0, "realm {id}");
    }
}

#[test]
fn the_reproduction_is_bit_identical_run_to_run() {
    let s = england!();
    let run = || {
        let mut k = s.starting_kingdom(SEED);
        k.start_new_game();
        k
    };
    let first = run();
    for _ in 0..8 {
        assert_eq!(first, run());
    }
    let again = england!();
    assert_eq!(s, again);
}

#[test]
fn ten_more_seasons_conserve_the_clock_and_the_labour_and_do_not_stand_still() {
    let s = england!();
    let mut k = kingdom_after_the_first_season(&s);
    let (season0, year0, turn0) = (k.season, k.year, k.turn_count);
    let labour0: Vec<i32> =
        s.county_ids().map(|id| k.counties[id].labour.iter().sum::<i32>()).collect();
    for (n, id) in s.county_ids().enumerate() {
        assert_eq!(labour0[n], k.counties[id].population, "county {id} starts fully employed");
    }
    let before: Vec<(i32, i32, i32)> =
        s.county_ids().map(|id| {
            let c = &k.counties[id];
            (c.population, c.happiness, c.herd)
        }).collect();

    for step in 1..=10u32 {
        k.advance_season();

        let elapsed = turn0 + step;
        assert_eq!(k.turn_count, elapsed, "one turn per season");
        let expected_season = (season0 as u32 - 1 + step) % 4 + 1;
        assert_eq!(k.season as u32, expected_season, "season {step}");
        assert_eq!(k.season_next as u32, expected_season % 4 + 1, "season {step}");
        let rolls =
            (1..=step).filter(|i| (season0 as u32 - 1 + i) % 4 + 1 == 4).count() as i32;
        assert_eq!(k.year, year0 + rolls, "season {step}");
        assert_eq!(k.year_next, k.year + 1, "season {step}");

        for id in s.county_ids() {
            let c = &k.counties[id];
            let assigned: i32 = c.labour.iter().sum();
            assert!(c.labour.iter().all(|&j| j >= 0), "county {id} season {step}");
            assert_eq!(
                assigned, c.population,
                "county {id} season {step}: every peasant is in exactly one of the nine \
                 records - {:?}",
                c.labour
            );
        }
    }

    let after: Vec<(i32, i32, i32)> =
        s.county_ids().map(|id| {
            let c = &k.counties[id];
            (c.population, c.happiness, c.herd)
        }).collect();
    assert_ne!(before, after, "ten seasons changed nothing anywhere");
    assert!(
        s.county_ids().all(|id| k.counties[id].population > 0),
        "every county still has people in it"
    );
}

/// `docs/decisions.md` C12: *a test that passes before and after the change is
/// not testing the thing its name claims*. The test above used to assert nine
/// ranges — happiness in `0..=100`, `health_band <= 4`, `unrest <= 4`,
/// `ration_achieved` in `0..=5`, every store non-negative — and every one of
/// those is a **clamp our own code applied moments earlier**:
///
/// `happiness.rs:74`, `health.rs:40`, `county.rs:476`. It could not fail. It
/// sat inside the file that *is* the C12 correction, and it survived that
/// correction because it was reasonable-looking and green.
#[test]
fn ten_seasons_under_a_different_ruleset_reach_a_different_kingdom() {
    let s = england!();

    let run = |tables: Tables| {
        let mut k = s.starting_kingdom_with_tables(SEED, tables);
        k.start_new_game();
        for _ in 0..10 {
            k.advance_season();
        }
        s.county_ids().map(|id| k.counties[id].population).collect::<Vec<i32>>()
    };

    let stock = run(Tables::DEFAULT);
    let mut lean = Tables::DEFAULT;
    lean.food.dairy_per_head /= 2;
    let hungry = run(lean);

    assert_ne!(stock, hungry, "halving the dairy yield changed nothing in ten seasons");
    assert_eq!(stock, run(Tables::DEFAULT), "the same rules must give the same run");
}


