#![allow(unused_imports)]
use super::*;
use super::migration::*;
use super::update::*;
use crate::county::{ChangeReason, County, CHANGE_REASON_MIN_PCT, MAX_INFLOW_SOURCES};
use crate::math::pct;
use crate::tables::{Season, Tables};
use l2_net::{Quirk, Quirks};

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    const Q: Quirks = Quirks::FAITHFUL;

    const T: &Tables = &Tables::DEFAULT;

    /// **Corrected.** This table read "owned (4 counties)" and "unowned (10
    /// counties)" — the invented split that `docs/decisions.md` C20 retracted,
    /// still sitting here in a doc comment after the correction. The file holds
    /// **five owned, one per realm, and nine unowned**; the *numbers* were
    /// always right, which is exactly why the wrong count survived a correction
    /// aimed at them. The word "shipped" is wrong too: a clean install ships no
    /// saves at all, and calling a rolling autosave "shipped" is what let it be
    /// treated as a fixture until somebody played the game.
    #[test]
    fn both_population_rows_from_the_england_fixture_reproduce() {
        let build = |happiness: i32| {
            let mut c = County::new();
            c.population = 417;
            c.happiness = happiness;
            c.health_band = 3;
            c
        };

        let mut owned = build(72);
        update_one(T, &mut owned, Season::Winter, Q);
        assert_eq!(owned.pop_last, 417);
        assert_eq!(owned.births, 63, "Pct(417, Pct(20, 75)) + 1");
        assert_eq!(owned.deaths, 45, "Pct(417, 3 + 8)");
        assert_eq!(owned.population, 435);
        assert_eq!(owned.pop_band, 18);

        let mut unowned = build(77);
        update_one(T, &mut unowned, Season::Winter, Q);
        assert_eq!(unowned.births, 84, "Pct(417, 20) + 1");
        assert_eq!(unowned.deaths, 45);
        assert_eq!(unowned.population, 456);
        assert_eq!(unowned.pop_band, 19);
    }

    /// **C170: the `+1` and the one-person floor compare the rate
    /// after happiness has scaled it.** `Population_UpdateAll` tests `local_c`,
    /// which is `Pct(base, factor)`, where this crate tested the ladder's `base`.
    #[test]
    fn the_extra_person_goes_where_the_scaled_birth_rate_sends_it() {
        let saved = [
            (799, 72, 2, Season::Spring, 79, 96),
            (756, 63, 3, Season::Winter, 75, 84),
        ];
        for (pop, happiness, band, season, births, deaths) in saved {
            let mut c = County::new();
            c.population = pop;
            c.happiness = happiness;
            c.health_band = band;
            update_one(T, &mut c, season, Q);
            assert_eq!(
                (c.births, c.deaths),
                (births, deaths),
                "{pop} people in {season:?}: the file stores {births} births and {deaths} deaths"
            );
        }

        // The floor's half, and **[D]** only — no save has a county this size.
        let mut c = County::new();
        c.population = 2500;
        c.happiness = 50;
        c.health_band = 4;
        update_one(T, &mut c, Season::Winter, Q);
        assert_eq!(c.births, 0, "a scaled rate of zero earns no floor");
        assert_eq!(c.deaths, 200 + 1, "Pct(2500, 8), and 0 < 8 sends the +1 to the deaths");
    }

    #[test]
    fn the_birth_factor_steps_at_twenty_six_fifty_one_seventy_six_and_a_hundred() {
        assert_eq!(T.happiness_birth_factor(0), 25);
        assert_eq!(T.happiness_birth_factor(25), 25);
        assert_eq!(T.happiness_birth_factor(26), 50);
        assert_eq!(T.happiness_birth_factor(50), 50);
        assert_eq!(T.happiness_birth_factor(51), 75);
        assert_eq!(T.happiness_birth_factor(75), 75);
        assert_eq!(T.happiness_birth_factor(76), 100);
        assert_eq!(T.happiness_birth_factor(99), 100);
        assert_eq!(T.happiness_birth_factor(100), 120);
    }

    #[test]
    fn a_diseased_county_in_winter_loses_nearly_half_its_people() {
        let mut c = County::new();
        c.population = 1000;
        c.happiness = 50;
        c.health_band = 0;
        update_one(T, &mut c, Season::Winter, Q);
        assert_eq!(c.deaths, 430 + 2 + 1);
    }

    #[test]
    fn summer_is_the_safest_season_and_winter_the_deadliest() {
        let deaths_in = |season: Season| {
            let mut c = County::new();
            c.population = 1000;
            c.happiness = 50;
            c.health_band = 2;
            update_one(T, &mut c, season, Q);
            c.deaths
        };
        assert!(deaths_in(Season::Winter) > deaths_in(Season::Spring));
        assert!(deaths_in(Season::Spring) > deaths_in(Season::Autumn));
        assert!(deaths_in(Season::Autumn) > deaths_in(Season::Summer));
    }

    #[test]
    fn a_non_zero_rate_always_costs_or_gains_at_least_one_person() {
        let mut c = County::new();
        c.population = 1;
        c.happiness = 50;
        c.health_band = 2; // 8% death rate, 0 in summer
        update_one(T, &mut c, Season::Summer, Q);
        assert!(c.births >= 1);

        let mut c = County::new();
        c.population = 1;
        c.happiness = 0;
        c.health_band = 4;
        update_one(T, &mut c, Season::Winter, Q); // 0 + 8 = 8%, Pct(1, 8) = 0
        assert!(c.deaths >= 1, "8% of one person still kills someone eventually");
    }

    #[test]
    fn a_county_that_loses_everyone_is_emptied() {
        let mut c = County::new();
        c.population = 1;
        c.happiness = 0;
        c.health_band = 0;
        update_one(T, &mut c, Season::Winter, Q);
        assert_eq!(c.population, 0);
        assert_eq!(c.births, 0);
        assert!(c.deaths <= 0, "the documented expression yields a negative count");
    }

    #[test]
    fn the_birth_ladder_caps_growth_around_two_thousand() {
        let grow = |pop: i32| {
            let mut c = County::new();
            c.population = pop;
            c.happiness = 100;
            c.health_band = 4;
            update_one(T, &mut c, Season::Winter, Q);
            c.population - pop
        };
        assert!(grow(400) > 0, "a small county grows");
        assert!(grow(2500) < 0, "a large one cannot keep up with winter");
    }

    // --- the random-event swing, C169 ------------------------
    //
    // Every expected number below is worked by hand from `Population_UpdateAll`
    // (`0x00449EF3`) — `Pct(deaths or births, |pct|) + 10`, capped at
    // `Pct(population, 20)` — and typed, so no expression here mentions the rule
    // under test. The tables it reads: death by health band 35/20/8/3/0, by
    // season Spring 4, Summer 0, Autumn 2, Winter 8.

    /// `FUN_00448F6F` pins the health meter at 25 as well, which is band 1 and a
    /// 20% death rate before the season's — so the deaths are
    /// `Pct(1000, 20 + season) + 1`, and the plague takes its percentage of
    /// *those*.
    #[test]
    fn a_plague_adds_a_percentage_of_the_deaths_plus_ten() {
        let cases = [
            (Season::Spring, 241, 82, 323),  // Pct(241, 30) = 72
            (Season::Summer, 201, 50, 251),  // Pct(201, 20) = 40
            (Season::Autumn, 221, 76, 297),  // Pct(221, 30) = 66
            (Season::Winter, 281, 122, 403), // Pct(281, 40) = 112
        ];
        for (season, natural, swing, after) in cases {
            let mut c = County::new();
            c.owner = 1;
            c.population = 1000;
            c.happiness = 100;
            c.health_meter = 60;
            c.health_band = 2;
            let mut purse = crate::event::RealmPurse::default();
            let plague = crate::event::EventKind::Plague;
            assert!(crate::event::fire(&mut c, 1, &mut purse, plague, season, Q));
            assert_eq!(c.health_band, 1, "the handler clamps the meter to 25");

            update_one(T, &mut c, season, Q);
            assert_eq!(c.event_population_swing, swing, "{season:?}: the letter's figure");
            assert_eq!(c.deaths, after, "{season:?}: {natural} natural, plus the swing");
            assert_eq!(c.births, 140, "{season:?}: a plague moves no births");
        }
    }

    #[test]
    fn a_wedding_adds_a_percentage_of_the_births_plus_ten() {
        let cases = [
            (Season::Spring, 141, 94, 235, 120), // Pct(141, 60) = 84
            (Season::Summer, 141, 80, 221, 80),  // Pct(141, 50) = 70
            (Season::Autumn, 141, 66, 207, 100), // Pct(141, 40) = 56
            (Season::Winter, 140, 52, 192, 161), // Pct(140, 30) = 42
        ];
        for (season, natural, swing, after, deaths) in cases {
            let mut c = County::new();
            c.owner = 1;
            c.population = 1000;
            c.happiness = 100;
            c.health_band = 2;
            let mut purse = crate::event::RealmPurse::default();
            let wedding = crate::event::EventKind::WeddingFever;
            assert!(crate::event::fire(&mut c, 1, &mut purse, wedding, season, Q));

            update_one(T, &mut c, season, Q);
            assert_eq!(c.event_population_swing, swing, "{season:?}: the letter's figure");
            assert_eq!(c.births, after, "{season:?}: {natural} natural, plus the swing");
            assert_eq!(c.deaths, deaths, "{season:?}: a wedding moves no deaths");
        }
    }

    #[test]
    fn the_swing_is_capped_at_a_fifth_of_the_county_and_is_never_less_than_ten() {
        let mut c = County::new();
        c.population = 100;
        c.happiness = 100;
        c.health_band = 4;
        c.event_population_pct = 60;
        update_one(T, &mut c, Season::Spring, Q);
        assert_eq!(c.event_population_swing, 20, "capped at Pct(100, 20), not 46");
        assert_eq!(c.births, 61 + 20);

        let mut c = County::new();
        c.population = 1000;
        c.happiness = 50;
        c.health_band = 4;
        c.event_population_pct = -20;
        update_one(T, &mut c, Season::Summer, Q);
        assert_eq!(c.event_population_swing, 10, "Pct(0, 20) + 10");
        assert_eq!(c.deaths, 10);
        assert_eq!(c.event_population_pct, 0, "and consumed");
    }

    /// `Population_UpdateAll` zeroes `+0x2F8` in every county before it looks
    /// at the event byte,
    /// figure
    #[test]
    fn a_season_without_an_event_writes_the_figure_back_to_zero() {
        let mut c = County::new();
        c.population = 1000;
        c.happiness = 50;
        c.health_band = 2;
        c.event_population_swing = 74; // last season's Winter plague
        update_one(T, &mut c, Season::Spring, Q);
        assert_eq!(c.event_population_swing, 0);
    }

    #[test]
    fn the_army_field_is_cleared_every_season() {
        let mut c = County::new();
        c.population = 400;
        c.army = 250;
        update_one(T, &mut c, Season::Spring, Q);
        assert_eq!(c.army, 0);
    }


    fn linked(happiness: [i32; 3], unowned: bool) -> Vec<County> {
        let mut counties = vec![County::new(); 4];
        for id in 1..=3 {
            counties[id].population = 1000;
            counties[id].happiness = happiness[id - 1];
            counties[id].owner = if unowned { 0 } else { 1 };
        }
        counties[1].add_neighbour(2);
        counties[2].add_neighbour(1);
        counties[2].add_neighbour(3);
        counties[3].add_neighbour(2);
        counties
    }

    #[test]
    fn people_move_towards_the_happier_neighbour_and_the_cap_bites() {
        let mut c = linked([20, 90, 90], false);
        migrate_all(&mut c, 3, Q);
        assert_eq!(c[1].emigrants, MIGRATION_CAP);
        assert_eq!(c[1].emigrant_destination, 2);
        assert_eq!(c[2].immigrants, MIGRATION_CAP);
        assert_eq!(c[2].emigrants, 0, "nobody leaves the happiest county");
    }

    #[test]
    fn a_perfectly_happy_county_never_loses_anyone() {
        assert_eq!(movers(10_000, 100, 100, false), 0);
        let mut c = linked([100, 100, 100], false);
        migrate_all(&mut c, 3, Q);
        for id in 1..=3 {
            assert_eq!(c[id].emigrants, 0);
        }
    }

    #[test]
    fn an_unowned_county_sheds_people_at_half_the_rate() {
        let owned = movers(1000, 20, 90, false);
        let unowned = movers(1000, 20, 90, true);
        assert_eq!(unowned, owned / 2);
    }

    #[test]
    fn nobody_moves_towards_an_unhappier_neighbour() {
        let mut c = linked([90, 20, 20], false);
        migrate_all(&mut c, 3, Q);
        assert_eq!(c[1].emigrants, 0, "the happiest county loses nobody");
        assert_eq!(c[3].emigrants, 0, "3's only neighbour is no happier than it is");
        assert_eq!(c[2].emigrants, MIGRATION_CAP, "2 empties towards 1");
        assert_eq!(c[2].emigrant_destination, 1);
        assert_eq!(c[1].immigrants, c[2].emigrants);
        assert_eq!(c[3].immigrants, 0);
    }

    #[test]
    fn the_inflow_list_repeats_one_source_into_every_free_slot() {
        let mut c = linked([20, 90, 90], false);
        migrate_all(&mut c, 3, Q);
        assert_eq!(c[2].inflow_sources, [1u8; MAX_INFLOW_SOURCES]);
        assert_eq!(c[2].largest_inflow, MIGRATION_CAP);
        assert_eq!(c[2].largest_inflow_source, 1);
    }

    #[test]
    fn migration_gives_the_same_answer_every_time() {
        let a = {
            let mut c = linked([10, 40, 90], false);
            migrate_all(&mut c, 3, Q);
            c
        };
        for _ in 0..8 {
            let mut b = linked([10, 40, 90], false);
            migrate_all(&mut b, 3, Q);
            assert_eq!(a, b);
        }
    }

    #[test]
    fn migration_moves_nobody_until_the_population_pass_runs() {
        let mut c = linked([20, 90, 90], false);
        migrate_all(&mut c, 3, Q);
        assert_eq!(c[1].population, 1000, "still there");
        update_all(T, &mut c, 3, Season::Summer, Q);
        assert!(c[1].population < 1000 + c[1].births);
        assert_eq!(c[2].population, 1000 + c[2].births - c[2].deaths + MIGRATION_CAP);
    }
}

