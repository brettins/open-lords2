//! Population and migration — `docs/kingdom.md` §5,
//! `Population_UpdateAll` (`0x00449EF3`) and `Migration_UpdateAll`
//! (`0x0044A6BA`).
//!
//! ```text
//! popLast = pop;
//! cap     = Pct(pop, 20);
//! base    = Table_Lookup(pop, g_birthRateLadder, 20, 1);
//! death   = g_deathRateByHealth[healthBand] + g_deathRateBySeason[g_season];
//! factor  = happiness < 26 ? 25 : happiness < 51 ? 50
//!         : happiness < 76 ? 75 : happiness < 100 ? 100 : 120;
//! births  = Pct(pop, Pct(base, factor));
//! deaths  = Pct(pop, death);
//! if (births == 0 && base  != 0) births = 1;
//! if (deaths == 0 && death != 0) deaths = 1;
//! if (healthBand == 0)   deaths += 2;
//! if (base < death)      deaths += 1; else births += 1;
//! /* random-event modifier, capped at 20% of the population */
//! pop += births - deaths;
//! if (pop < 1) { births = 0; deaths = pop; pop = 0; }
//! pop -= emigrants; pop += immigrants;
//! ```
//!
//! This is the pass `docs/kingdom.md` §9 reproduces twice each for births and
//! deaths, on two different happiness bands, from the shipped save. It is the
//! strongest evidence in that document and the reason most of §4 and §5 is
//! marked **`[V]`**.

use crate::county::{ChangeReason, County, CHANGE_REASON_MIN_PCT, MAX_INFLOW_SOURCES};
use crate::math::{clamp, pct};
use crate::tables::{Season, Tables};

/// Migration is capped at this many people per county per season, before the
/// unowned halving. `docs/kingdom.md` §5.3.
pub const MIGRATION_CAP: i32 = 100;

/// How many people would leave a county for a happier neighbour.
///
/// ```text
/// pct    = Pct(bestNeighbourHappiness - happiness, (100 - happiness) / 3);
/// movers = Pct(population, pct);
/// if (movers > 100) movers = 100;
/// if (owner == 0)   movers /= 2;
/// ```
///
/// Two consequences worth stating, both asserted below: migration is capped at
/// 100 people per county per season, and **a county at 100 happiness never
/// emigrates**, whatever its neighbours do — `(100 - 100) / 3` is zero.
pub fn movers(population: i32, happiness: i32, best_neighbour: i32, unowned: bool) -> i32 {
    if best_neighbour <= happiness {
        return 0;
    }
    let rate = pct(best_neighbour - happiness, (100 - happiness) / 3);
    let mut movers = pct(population, rate);
    if movers > MIGRATION_CAP {
        movers = MIGRATION_CAP;
    }
    if unowned {
        movers /= 2;
    }
    movers.max(0)
}

/// `Migration_UpdateAll` — every county's emigrants and immigrants for this
/// season.
///
/// Happiness is snapshotted before anything moves, so a county's decision does
/// not depend on how many counties happen to be earlier in the array. The
/// original walks the array in index order and so does this, which is the same
/// thing said twice — but only one of the two is still true if somebody later
/// parallelises the loop (`docs/netcode.md` §3).
pub fn migrate_all(counties: &mut [County], county_count: usize) {
    for id in 1..=county_count {
        counties[id].emigrants = 0;
        counties[id].immigrants = 0;
        counties[id].largest_inflow = 0;
        counties[id].largest_inflow_source = 0;
        counties[id].emigrant_destination = 0;
        counties[id].inflow_sources = [0; MAX_INFLOW_SOURCES];
    }

    let happiness: Vec<i32> = (0..counties.len()).map(|i| counties[i].happiness).collect();

    for id in 1..=county_count {
        // The happiest neighbour, first one wins on a tie. Stored order, never
        // sorted: the tie-break is part of the simulation.
        let mut best: Option<(u8, i32)> = None;
        for &n in counties[id].neighbours() {
            let n = n as usize;
            if n == 0 || n >= counties.len() {
                continue;
            }
            if best.map_or(true, |(_, h)| happiness[n] > h) {
                best = Some((n as u8, happiness[n]));
            }
        }
        let Some((dest, best_happiness)) = best else { continue };

        let leaving = movers(
            counties[id].population,
            happiness[id],
            best_happiness,
            counties[id].is_unowned(),
        );
        if leaving == 0 {
            continue;
        }

        counties[id].emigrants = leaving;
        counties[id].emigrant_destination = dest;

        let d = dest as usize;
        counties[d].immigrants += leaving;
        if leaving > counties[d].largest_inflow {
            counties[d].largest_inflow = leaving;
            counties[d].largest_inflow_source = id as u8;
        }
        record_inflow_source(&mut counties[d], id as u8);
    }
}

/// Record a source in a destination's sixteen-slot inflow list.
///
/// **This reproduces a documented bug.** `docs/kingdom.md` §5.3: *"the loop
/// that records the source county in the destination's 16-byte inflow list has
/// no `break`: it writes the source id into **every** free slot rather than the
/// first."* The list therefore ends up holding one repeated value.
///
/// It is marked **`[D]`** — read from decompiled C, not observed — and it is
/// cosmetic: the pass that reads it back only takes a maximum, so the visible
/// effect is limited to the *"arrive from"* line of the population panel naming
/// the wrong county. It is reproduced rather than fixed because a
/// reimplementation that quietly corrects the original's bugs cannot be
/// differentially tested against it.
fn record_inflow_source(dest: &mut County, source: u8) {
    for slot in 0..MAX_INFLOW_SOURCES {
        if dest.inflow_sources[slot] == 0 {
            dest.inflow_sources[slot] = source;
        }
    }
}

/// One county's population pass.
///
/// `season` is `g_season` — the season now *beginning*, which is what makes
/// `g_deathRateBySeason[4] = 8` the Winter figure.
pub fn update_one(t: &Tables, county: &mut County, season: Season) {
    county.pop_last = county.population;

    let cap = pct(county.population, t.event.population_cap_pct);
    let base = t.birth_rate(county.population);
    let death = t.health[(county.health_band as usize).min(t.health.len() - 1)].death_rate
        + t.season[season.index() as usize].death_rate;
    let factor = t.happiness_birth_factor(county.happiness);

    let mut births = pct(county.population, pct(base, factor));
    let mut deaths = pct(county.population, death);

    if births == 0 && base != 0 {
        births = 1;
    }
    if deaths == 0 && death != 0 {
        deaths = 1;
    }
    if county.health_band == 0 {
        deaths += 2;
    }
    if base < death {
        deaths += 1;
    } else {
        births += 1;
    }

    // The random-event modifier. docs/kingdom.md §8.1: most event handlers
    // write a percentage into +0x1FB, which this pass applies to births and
    // deaths, capped at 20% of the county.
    if county.event_population_pct != 0 {
        let swing = clamp(pct(county.population, county.event_population_pct), -cap, cap);
        if swing >= 0 {
            births += swing;
        } else {
            deaths += -swing;
        }
        county.event_population_pct = 0;
    }

    county.population += births - deaths;
    if county.population < 1 {
        births = 0;
        // Written exactly as docs/kingdom.md §5 states it. `pop` is negative
        // here, so a county that dies out stores a *negative* death count -
        // see the errata note in the crate documentation.
        deaths = county.population;
        county.population = 0;
    }

    // "Zeroed by Population_UpdateAll and filled elsewhere."
    county.army = 0;

    county.population -= county.emigrants;
    county.population += county.immigrants;

    county.births = births;
    county.deaths = deaths;
    county.pop_band = county.compute_pop_band();
    county.pop_change_pct = if county.pop_last != 0 {
        (county.population - county.pop_last).abs() * 100 / county.pop_last
    } else {
        0
    };
    county.change_reason = change_reason(county);
}

/// `L2.eng` group 65's *"Births / Deaths / Emigration / Immigration"* line,
/// suppressed below a 6% change (`docs/kingdom.md` §1.2, county `+0x5B`).
///
/// **`[I]` on the selection.** The document gives the five values, the ordering
/// and the 6% suppression, and does not say how one of the four causes is
/// picked when several moved. The largest contributor, ties broken by the
/// enumeration order, is the only choice that makes the field mean anything.
fn change_reason(county: &County) -> ChangeReason {
    if county.pop_change_pct < CHANGE_REASON_MIN_PCT {
        return ChangeReason::None;
    }
    let candidates = [
        (county.births, ChangeReason::Births),
        (county.deaths.abs(), ChangeReason::Deaths),
        (county.emigrants, ChangeReason::Emigration),
        (county.immigrants, ChangeReason::Immigration),
    ];
    let mut best = ChangeReason::None;
    let mut best_size = 0;
    for (size, reason) in candidates {
        if size > best_size {
            best_size = size;
            best = reason;
        }
    }
    best
}

/// `Population_UpdateAll` over the whole kingdom, in index order.
pub fn update_all(t: &Tables, counties: &mut [County], county_count: usize, season: Season) {
    for id in 1..=county_count {
        update_one(t, &mut counties[id], season);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The stock ruleset. Every rule below takes it as an argument now.
    const T: &Tables = &Tables::DEFAULT;

    /// **`docs/kingdom.md` §9 point 8, both rows.** Every county in the England
    /// turn-one position started at `popLast = 417`, giving a 20% base birth
    /// rate:
    ///
    /// | | happiness | factor | births | deaths | population |
    /// |---|---:|---:|---:|---:|---:|
    /// | owned (5 counties) | 72 | 75% | 63 | 45 | 435 |
    /// | unowned (9 counties) | 77 | 100% | 84 | 45 | 456 |
    ///
    /// The deaths figure needs `g_deathRateByHealth[3] = 3` **and**
    /// `g_deathRateBySeason[4] = 8`, so it confirms the season index too.
    ///
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
        update_one(T, &mut owned, Season::Winter);
        assert_eq!(owned.pop_last, 417);
        assert_eq!(owned.births, 63, "Pct(417, Pct(20, 75)) + 1");
        assert_eq!(owned.deaths, 45, "Pct(417, 3 + 8)");
        assert_eq!(owned.population, 435);
        assert_eq!(owned.pop_band, 18);

        let mut unowned = build(77);
        update_one(T, &mut unowned, Season::Winter);
        assert_eq!(unowned.births, 84, "Pct(417, 20) + 1");
        assert_eq!(unowned.deaths, 45);
        assert_eq!(unowned.population, 456);
        assert_eq!(unowned.pop_band, 19);
    }

    /// The happiness factor bands, at every boundary.
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

    /// A Diseased county in winter loses 43% of its people in one season, plus
    /// the flat +2 the band carries.
    #[test]
    fn a_diseased_county_in_winter_loses_nearly_half_its_people() {
        let mut c = County::new();
        c.population = 1000;
        c.happiness = 50;
        c.health_band = 0;
        update_one(T, &mut c, Season::Winter);
        // 35 + 8 = 43%, +2 for Diseased, +1 because base (12) < death (43).
        assert_eq!(c.deaths, 430 + 2 + 1);
    }

    #[test]
    fn summer_is_the_safest_season_and_winter_the_deadliest() {
        let deaths_in = |season: Season| {
            let mut c = County::new();
            c.population = 1000;
            c.happiness = 50;
            c.health_band = 2;
            update_one(T, &mut c, season);
            c.deaths
        };
        assert!(deaths_in(Season::Winter) > deaths_in(Season::Spring));
        assert!(deaths_in(Season::Spring) > deaths_in(Season::Autumn));
        assert!(deaths_in(Season::Autumn) > deaths_in(Season::Summer));
    }

    /// The two "never quite zero" clauses: a non-zero rate always moves at
    /// least one person.
    #[test]
    fn a_non_zero_rate_always_costs_or_gains_at_least_one_person() {
        let mut c = County::new();
        c.population = 1;
        c.happiness = 50;
        c.health_band = 2; // 8% death rate, 0 in summer
        update_one(T, &mut c, Season::Summer);
        assert!(c.births >= 1);

        let mut c = County::new();
        c.population = 1;
        c.happiness = 0;
        c.health_band = 4;
        update_one(T, &mut c, Season::Winter); // 0 + 8 = 8%, Pct(1, 8) = 0
        assert!(c.deaths >= 1, "8% of one person still kills someone eventually");
    }

    /// A county that dies out is emptied rather than going negative.
    #[test]
    fn a_county_that_loses_everyone_is_emptied() {
        let mut c = County::new();
        c.population = 1;
        c.happiness = 0;
        c.health_band = 0;
        update_one(T, &mut c, Season::Winter);
        assert_eq!(c.population, 0);
        assert_eq!(c.births, 0);
        // docs/kingdom.md §5 stores the (negative) leftover here. See the
        // errata note - this is almost certainly a decompilation artefact.
        assert!(c.deaths <= 0, "the documented expression yields a negative count");
    }

    /// The soft cap: a big county in winter shrinks and a small one grows.
    #[test]
    fn the_birth_ladder_caps_growth_around_two_thousand() {
        let grow = |pop: i32| {
            let mut c = County::new();
            c.population = pop;
            c.happiness = 100;
            c.health_band = 4;
            update_one(T, &mut c, Season::Winter);
            c.population - pop
        };
        assert!(grow(400) > 0, "a small county grows");
        assert!(grow(2500) < 0, "a large one cannot keep up with winter");
    }

    #[test]
    fn the_event_modifier_is_capped_at_a_fifth_of_the_county() {
        let mut c = County::new();
        c.population = 1000;
        c.happiness = 50;
        c.health_band = 4;
        c.event_population_pct = 90; // a 90% baby boom, capped to 20%
        update_one(T, &mut c, Season::Summer);
        // Pct(1000, Pct(12, 50)) = 60 natural births, +1, +200 from the capped
        // event - not the 900 the event asked for.
        assert_eq!(c.births, 60 + 1 + 200, "capped at Pct(1000, 20)");
        assert_eq!(c.event_population_pct, 0, "and consumed");

        let mut c = County::new();
        c.population = 1000;
        c.happiness = 50;
        c.health_band = 4;
        c.event_population_pct = -90;
        update_one(T, &mut c, Season::Summer);
        assert_eq!(c.deaths, 200, "a plague is capped the same way");
    }

    #[test]
    fn the_army_field_is_cleared_every_season() {
        let mut c = County::new();
        c.population = 400;
        c.army = 250;
        update_one(T, &mut c, Season::Spring);
        assert_eq!(c.army, 0);
    }

    // --- migration ---------------------------------------------------------

    fn linked(happiness: [i32; 3], unowned: bool) -> Vec<County> {
        let mut counties = vec![County::new(); 4];
        for id in 1..=3 {
            counties[id].population = 1000;
            counties[id].happiness = happiness[id - 1];
            counties[id].owner = if unowned { 0 } else { 1 };
        }
        // A line: 1 - 2 - 3.
        counties[1].add_neighbour(2);
        counties[2].add_neighbour(1);
        counties[2].add_neighbour(3);
        counties[3].add_neighbour(2);
        counties
    }

    #[test]
    fn people_move_towards_the_happier_neighbour_and_the_cap_bites() {
        let mut c = linked([20, 90, 90], false);
        migrate_all(&mut c, 3);
        // Pct(90 - 20, (100 - 20) / 3 = 26) = 18; Pct(1000, 18) = 180 -> capped.
        assert_eq!(c[1].emigrants, MIGRATION_CAP);
        assert_eq!(c[1].emigrant_destination, 2);
        assert_eq!(c[2].immigrants, MIGRATION_CAP);
        assert_eq!(c[2].emigrants, 0, "nobody leaves the happiest county");
    }

    /// **A county at 100 happiness never emigrates**, whatever its neighbours
    /// do — `(100 - 100) / 3` is zero.
    #[test]
    fn a_perfectly_happy_county_never_loses_anyone() {
        assert_eq!(movers(10_000, 100, 100, false), 0);
        let mut c = linked([100, 100, 100], false);
        migrate_all(&mut c, 3);
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
        // A line 1 - 2 - 3 at happiness 90, 20, 20.
        let mut c = linked([90, 20, 20], false);
        migrate_all(&mut c, 3);
        assert_eq!(c[1].emigrants, 0, "the happiest county loses nobody");
        assert_eq!(c[3].emigrants, 0, "3's only neighbour is no happier than it is");
        assert_eq!(c[2].emigrants, MIGRATION_CAP, "2 empties towards 1");
        assert_eq!(c[2].emigrant_destination, 1);
        assert_eq!(c[1].immigrants, c[2].emigrants);
        assert_eq!(c[3].immigrants, 0);
    }

    /// The documented bug, reproduced: one source fills every free slot.
    #[test]
    fn the_inflow_list_repeats_one_source_into_every_free_slot() {
        let mut c = linked([20, 90, 90], false);
        migrate_all(&mut c, 3);
        assert_eq!(c[2].inflow_sources, [1u8; MAX_INFLOW_SOURCES]);
        assert_eq!(c[2].largest_inflow, MIGRATION_CAP);
        assert_eq!(c[2].largest_inflow_source, 1);
    }

    /// Migration is deterministic and repeatable — it is one of the two passes
    /// where a county's outcome depends on other counties.
    #[test]
    fn migration_gives_the_same_answer_every_time() {
        let a = {
            let mut c = linked([10, 40, 90], false);
            migrate_all(&mut c, 3);
            c
        };
        for _ in 0..8 {
            let mut b = linked([10, 40, 90], false);
            migrate_all(&mut b, 3);
            assert_eq!(a, b);
        }
    }

    /// The flows are applied by the *population* pass, not by migration.
    #[test]
    fn migration_moves_nobody_until_the_population_pass_runs() {
        let mut c = linked([20, 90, 90], false);
        migrate_all(&mut c, 3);
        assert_eq!(c[1].population, 1000, "still there");
        update_all(T, &mut c, 3, Season::Summer);
        assert!(c[1].population < 1000 + c[1].births);
        assert_eq!(c[2].population, 1000 + c[2].births - c[2].deaths + MIGRATION_CAP);
    }
}
