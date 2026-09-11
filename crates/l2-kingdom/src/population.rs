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
//! rate    = Pct(base, factor);             /* local_c: the SCALED rate */
//! births  = Pct(pop, rate);
//! deaths  = Pct(pop, death);
//! if (births == 0 && rate  != 0) births = 1;
//! if (deaths == 0 && death != 0) deaths = 1;
//! if (healthBand == 0)   deaths += 2;
//! if (rate < death)      deaths += 1; else births += 1;
//! swing   = pct < 0 ? Pct(deaths, -pct) + 10 : pct > 0 ? Pct(births, pct) + 10 : 0;
//! if (swing > cap) swing = cap;               /* county +0x2F8, the letter's figure */
//! if (pct < 0) deaths += swing; else if (pct > 0) births += swing;
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
use crate::math::pct;
use crate::tables::{Season, Tables};
use l2_net::{Quirk, Quirks};

/// Migration is capped at this many people per county per season, before the
/// unowned halving. `docs/kingdom.md` §5.3.
pub const MIGRATION_CAP: i32 = 100;

/// The flat number of people a population event adds on top of its percentage:
/// `Pct(deaths or births, |pct|) + 10`. `[V]` — `Population_UpdateAll`
/// (`0x00449EF3`) adds the literal `10` in both arms, and nowhere else. So a
/// plague always costs at least ten, and a county under fifty people, whose
/// 20% cap is below ten, loses the whole cap instead.
///
/// A `const` rather than a ruleset key: `kingdom.event` has two keys today
/// (`docs/modding.md`) and this is the first number the swing needed beyond the
/// cap.
pub const EVENT_SWING_FLOOR: i32 = 10;

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
pub fn migrate_all(counties: &mut [County], county_count: usize, quirks: Quirks) {
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
        record_inflow_source(&mut counties[d], id as u8, quirks);
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
///
/// **Switchable** — [`Quirk::InflowListHasNoBreak`], `docs/bugs.md` B15. The
/// fixed path stops at the first free slot, which turns the sixteen bytes back
/// into the list they are named for.
fn record_inflow_source(dest: &mut County, source: u8, quirks: Quirks) {
    for slot in 0..MAX_INFLOW_SOURCES {
        if dest.inflow_sources[slot] == 0 {
            dest.inflow_sources[slot] = source;
            if !quirks.reproduces(Quirk::InflowListHasNoBreak) {
                return;
            }
        }
    }
}

/// One county's population pass.
///
/// `season` is `g_season` — the season now *beginning*, which is what makes
/// `g_deathRateBySeason[4] = 8` the Winter figure.
pub fn update_one(t: &Tables, county: &mut County, season: Season, quirks: Quirks) {
    county.pop_last = county.population;

    let cap = pct(county.population, t.event.population_cap_pct);
    let base = t.birth_rate(county.population);
    let death = t.health[(county.health_band as usize).min(t.health.len() - 1)].death_rate
        + t.season[season.index() as usize].death_rate;
    let factor = t.happiness_birth_factor(county.happiness);
    // `local_c` in the decompilation: the ladder's rate **already scaled by
    // happiness**. It is this, not the ladder's own `base`, that both tests
    // below compare — `if ((births == 0) && (local_c != 0))` and
    // `if (local_c < iVar3)` — and CNEW-factored-rate is the season of births
    // and deaths we got one wrong in either direction by comparing `base`.
    let rate = pct(base, factor);

    let mut births = pct(county.population, rate);
    let mut deaths = pct(county.population, death);

    if births == 0 && rate != 0 {
        births = 1;
    }
    if deaths == 0 && death != 0 {
        deaths = 1;
    }
    if county.health_band == 0 {
        deaths += 2;
    }
    if rate < death {
        deaths += 1;
    } else {
        births += 1;
    }

    // The random-event swing — `Population_UpdateAll` (`0x00449EF3`), the
    // eighteen lines after the `+1` above. Only two handlers write `+0x1FB`:
    // *Plague* (`FUN_00448F6F`, −20 … −40) and *Wedding fever* (`FUN_004491F0`,
    // +30 … +60). `[V]` — every write to the byte in the binary is one of those
    // eight or one of the two clears.
    //
    // **A percentage of the deaths or the births, not of the county.** This
    // read `Pct(population, pct)` until CNEW-plague-swing, which made a Winter
    // plague on 1,000 people in health band 2 at happiness 50 kill 200 extra
    // rather than the original's 74 (`Pct(161, 40) + 10`).
    //
    // ```c
    // county.+0x2F8 = 0;
    // if (pct < 0)      county.+0x2F8 = Pct(deaths, -pct) + 10;
    // else if (pct > 0) county.+0x2F8 = Pct(births,  pct) + 10;
    // if (cap < county.+0x2F8) county.+0x2F8 = cap;
    // if (pct < 0) deaths += county.+0x2F8; else if (pct > 0) births += county.+0x2F8;
    // ```
    //
    // The base is the natural figure **after** the `+1`/`+2` adjustments, and
    // the stored value is what the letter prints (`County::event_population_swing`).
    let p = county.event_population_pct;
    county.event_population_swing = 0;
    if p < 0 {
        county.event_population_swing = pct(deaths, -p) + EVENT_SWING_FLOOR;
    } else if p > 0 {
        county.event_population_swing = pct(births, p) + EVENT_SWING_FLOOR;
    }
    if cap < county.event_population_swing {
        county.event_population_swing = cap;
    }
    if p < 0 {
        deaths += county.event_population_swing;
    } else if p > 0 {
        births += county.event_population_swing;
    }
    county.event_population_pct = 0;

    county.population += births - deaths;
    if county.population < 1 {
        births = 0;
        // Written exactly as docs/kingdom.md §5 states it. `pop` is negative
        // here, so a county that dies out stores a *negative* death count -
        // see the errata note in the crate documentation.
        //
        // **Switchable** - [`Quirk::ExtinctCountyRecordsNegativeDeaths`],
        // `docs/bugs.md` B16. The fixed path records the people who actually
        // died, which is what the county had before the pass: the negation
        // that reading suggests was lost is put back rather than the whole
        // statement rewritten, so the shape of the original stays visible.
        deaths = if quirks.reproduces(Quirk::ExtinctCountyRecordsNegativeDeaths) {
            county.population
        } else {
            county.pop_last
        };
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
pub fn update_all(
    t: &Tables,
    counties: &mut [County],
    county_count: usize,
    season: Season,
    quirks: Quirks,
) {
    for id in 1..=county_count {
        update_one(t, &mut counties[id], season, quirks);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Faithful. The switched-off answers live in `tests/quirks.rs`.
    #[allow(dead_code)]
    const Q: Quirks = Quirks::FAITHFUL;

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

    /// **CNEW-factored-rate: the `+1` and the one-person floor compare the rate
    /// after happiness has scaled it.** `Population_UpdateAll` tests `local_c`,
    /// which is `Pct(base, factor)`, where this crate tested the ladder's `base`.
    ///
    /// The two counties are the original's own after-saves, typed from the file
    /// rather than computed: `siege-lastturn.sav` county 1 and
    /// `siege-old_turn.sav` county 3, which `crates/l2-game/tests/differential.rs`
    /// had each one person out in births and one in deaths. Both are at factor
    /// 75 on a ladder rate of 14, so the scaled rate is 10 — below the death rate,
    /// where 14 is above it.
    #[test]
    fn the_extra_person_goes_where_the_scaled_birth_rate_sends_it() {
        // (population last season, happiness, health band, season, births, deaths)
        let saved = [
            // siege-lastturn county 1: Spring, band 2 — deaths 8 + 4 = 12%.
            (799, 72, 2, Season::Spring, 79, 96),
            // siege-old_turn county 3: Winter, band 3 — deaths 3 + 8 = 11%.
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
        // Above 2,000 people the ladder gives 1%, and at happiness 50 that
        // scales to Pct(1, 50) = 0: no births, and no one-person floor for them.
        let mut c = County::new();
        c.population = 2500;
        c.happiness = 50;
        c.health_band = 4;
        update_one(T, &mut c, Season::Winter, Q);
        assert_eq!(c.births, 0, "a scaled rate of zero earns no floor");
        assert_eq!(c.deaths, 200 + 1, "Pct(2500, 8), and 0 < 8 sends the +1 to the deaths");
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
        update_one(T, &mut c, Season::Winter, Q);
        // 35 + 8 = 43%, +2 for Diseased, +1 because the scaled birth rate
        // Pct(12, 50) = 6 is below the death rate of 43.
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

    /// The two "never quite zero" clauses: a non-zero rate always moves at
    /// least one person.
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

    /// A county that dies out is emptied rather than going negative.
    #[test]
    fn a_county_that_loses_everyone_is_emptied() {
        let mut c = County::new();
        c.population = 1;
        c.happiness = 0;
        c.health_band = 0;
        update_one(T, &mut c, Season::Winter, Q);
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
            update_one(T, &mut c, Season::Winter, Q);
            c.population - pop
        };
        assert!(grow(400) > 0, "a small county grows");
        assert!(grow(2500) < 0, "a large one cannot keep up with winter");
    }

    // --- the random-event swing, CNEW-plague-swing ------------------------
    //
    // Every expected number below is worked by hand from `Population_UpdateAll`
    // (`0x00449EF3`) — `Pct(deaths or births, |pct|) + 10`, capped at
    // `Pct(population, 20)` — and typed, so no expression here mentions the rule
    // under test. The tables it reads: death by health band 35/20/8/3/0, by
    // season Spring 4, Summer 0, Autumn 2, Winter 8.
    //
    // The first two tests use a county of 1,000 at happiness 100: a factored
    // birth rate of `Pct(12, 120) = 14`, so 140 births before the `+1`.

    /// **A plague, in all four seasons, through the handler that deals it.**
    /// `FUN_00448F6F` pins the health meter at 25 as well, which is band 1 and a
    /// 20% death rate before the season's — so the deaths are
    /// `Pct(1000, 20 + season) + 1`, and the plague takes its percentage of
    /// *those*.
    ///
    /// The old rule took it of the county and capped it: 200 extra deaths in
    /// every one of these seasons.
    #[test]
    fn a_plague_adds_a_percentage_of_the_deaths_plus_ten() {
        // (season, natural deaths, the swing, deaths after)
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

    /// **Wedding fever, in all four seasons, through the handler that deals
    /// it.** Band 2 keeps the deaths below the factored birth rate except in
    /// Winter, so the `+1` lands on the births three times and on the deaths
    /// once — and the wedding takes its percentage of the births *after* it.
    #[test]
    fn a_wedding_adds_a_percentage_of_the_births_plus_ten() {
        // (season, natural births, the swing, births after, deaths)
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

    /// The two ends of the rule: the 20% cap still bites on a small county, and
    /// the flat ten is the whole of a plague in a county that was going to lose
    /// nobody.
    #[test]
    fn the_swing_is_capped_at_a_fifth_of_the_county_and_is_never_less_than_ten() {
        // A hundred people at happiness 100, band 4, Spring: Pct(50, 120) = 60
        // births + 1 = 61, and a Spring wedding asks Pct(61, 60) + 10 = 46.
        let mut c = County::new();
        c.population = 100;
        c.happiness = 100;
        c.health_band = 4;
        c.event_population_pct = 60;
        update_one(T, &mut c, Season::Spring, Q);
        assert_eq!(c.event_population_swing, 20, "capped at Pct(100, 20), not 46");
        assert_eq!(c.births, 61 + 20);

        // Band 4 in Summer dies of nothing — a rate of 0 earns no `+1` — so a
        // Summer plague's 20% is of zero deaths.
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
    /// at the event byte, so a season without an event clears last season's
    /// figure rather than leaving the letter's number standing.
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
        migrate_all(&mut c, 3, Q);
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
        // A line 1 - 2 - 3 at happiness 90, 20, 20.
        let mut c = linked([90, 20, 20], false);
        migrate_all(&mut c, 3, Q);
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
        migrate_all(&mut c, 3, Q);
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
            migrate_all(&mut c, 3, Q);
            c
        };
        for _ in 0..8 {
            let mut b = linked([10, 40, 90], false);
            migrate_all(&mut b, 3, Q);
            assert_eq!(a, b);
        }
    }

    /// The flows are applied by the *population* pass, not by migration.
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
