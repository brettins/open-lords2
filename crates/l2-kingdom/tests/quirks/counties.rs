#![allow(unused_imports)]
use super::*;
use super::economy::*;
use super::victory::*;
use super::wire::*;
use l2_kingdom::county::County;
use l2_kingdom::kingdom::Kingdom;
use l2_kingdom::realm::Realm;
use l2_kingdom::tables::{Season, Tables, Weather};
use l2_kingdom::{Quirk, Quirks};

// ---------------------------------------------------------------------------
// B11a — an unowned county trades with neither stock nor gold
// ---------------------------------------------------------------------------

#[test]
fn b11a_a_lordless_county_sells_grain_it_does_not_have_or_is_refused() {
    use l2_kingdom::trade::{Good, Order, Quote, Refusal};
    let (faithful, fixed) = pair(Quirk::UnownedCountyTradesUnchecked);
    let quote = Quote { buy: 10, sell: 8 };

    let run = |quirks: Quirks| {
        let mut k = furnished_kingdom(11);
        k.options.quirks = quirks;
        k.counties[3].owner = 0; // nobody's county
        k.counties[3].grain = 0; // and nothing in the barn
        l2_kingdom::trade::trade(&mut k, Order::sell(Good::Grain, 100, quote, 0, 3))
            .map(|r| r.crowns)
            .map_err(|e| e)
    };

    assert_eq!(run(faithful), Ok(800), "reproduced: it sells what it has not got");
    assert_eq!(run(fixed), Err(Refusal::NotEnoughStock));
}

#[test]
fn b11a_a_lordless_county_buys_with_an_empty_purse_or_is_refused() {
    use l2_kingdom::trade::{Good, Order, Quote, Refusal};
    let (faithful, fixed) = pair(Quirk::UnownedCountyTradesUnchecked);
    let quote = Quote { buy: 10, sell: 8 };

    let run = |quirks: Quirks| {
        let mut k = furnished_kingdom(12);
        k.options.quirks = quirks;
        k.counties[3].owner = 0;
        k.counties[3].purse = 0;
        l2_kingdom::trade::trade(&mut k, Order::buy(Good::Grain, 100, quote, 0, 3))
            .map(|_| k.counties[3].purse)
    };

    assert_eq!(run(faithful), Ok(-1000), "reproduced: the purse goes negative");
    assert_eq!(run(fixed), Err(Refusal::NotEnoughGold));
}

/// An **owned** county was always guarded, so the switch must not touch it.
#[test]
fn b11a_an_owned_county_is_refused_either_way() {
    use l2_kingdom::trade::{Good, Order, Quote, Refusal};
    let (faithful, fixed) = pair(Quirk::UnownedCountyTradesUnchecked);
    let quote = Quote { buy: 10, sell: 8 };
    for quirks in [faithful, fixed] {
        let mut k = furnished_kingdom(13);
        k.options.quirks = quirks;
        k.counties[3].owner = 1;
        k.counties[3].grain = 0;
        assert_eq!(
            l2_kingdom::trade::trade(&mut k, Order::sell(Good::Grain, 100, quote, 1, 3)),
            Err(Refusal::NotEnoughStock)
        );
    }
}

// ---------------------------------------------------------------------------
// B12 — turning castle building on removes its labour share
// ---------------------------------------------------------------------------

/// The share moves the wrong way on every click with the quirk on, and the
/// right way with it off — asserted over **four clicks**, because one click
/// cannot tell "inverted" from "off by one".
#[test]
fn b12_the_castle_switch_moves_its_labour_share_backwards_or_forwards() {
    use l2_kingdom::industry::MapToggle;
    let (faithful, fixed) = pair(Quirk::CastleSwitchMovesShareBackwards);

    let walk = |quirks: Quirks| {
        let mut c = County::new();
        c.population = 400;
        let mut shares = Vec::new();
        for _ in 0..4 {
            l2_kingdom::industry::toggle_from_map(&mut c, MapToggle::Castle, quirks);
            shares.push(c.labour_share[l2_kingdom::tables::JOB_CASTLE_BUILDING]);
        }
        shares
    };

    let a = walk(faithful);
    let b = walk(fixed);
    assert_ne!(a, b, "the share sequence must differ: {a:?} against {b:?}");
    assert!(
        b[0] > 0,
        "fixed, switching castle building ON gives it a share: {b:?}"
    );
}

// ---------------------------------------------------------------------------
// B15 — the migration inflow list is written with no break
// ---------------------------------------------------------------------------

#[test]
fn b15_the_inflow_list_holds_one_repeated_value_or_a_list() {
    let (faithful, fixed) = pair(Quirk::InflowListHasNoBreak);

    let sources = |quirks: Quirks| {
        let mut k = furnished_kingdom(15);
        k.options.quirks = quirks;
        // Two miserable counties beside a happy one, so both send people to it.
        // **The adjacency has to be set** - `migrate_all` walks
        // `County::neighbours`, and a county with none emigrates nowhere, which
        // is how the first draft of this test measured an empty list twice and
        // called them different.
        k.counties[1].happiness = 100;
        for id in 2..=3 {
            k.counties[id].happiness = 0;
            k.counties[id].population = 500;
            k.counties[id].add_neighbour(1);
            k.counties[1].add_neighbour(id as u8);
        }
        l2_kingdom::population::migrate_all(&mut k.counties, k.county_count, quirks);
        (1..=k.county_count)
            .map(|id| k.counties[id].inflow_sources)
            .collect::<Vec<_>>()
    };

    let a = sources(faithful);
    let b = sources(fixed);
    let filled = |list: &[u8; l2_kingdom::county::MAX_INFLOW_SOURCES]| {
        list.iter().filter(|v| **v != 0).count()
    };

    // The reproduced bug: some county's list is one value written into every
    // free slot, so it is full.
    assert!(
        a.iter().any(|l| filled(l) > 1 && l.iter().filter(|v| **v != 0).all(|v| *v == l[0])),
        "reproduced: a destination's sixteen bytes hold one repeated source"
    );
    assert!(
        b.iter().all(|l| filled(l) <= 2),
        "fixed: one slot per arriving county, not sixteen"
    );
    assert_ne!(a, b);
}

// ---------------------------------------------------------------------------
// B16 — a county that dies out records a negative death count
// ---------------------------------------------------------------------------

#[test]
fn b16_an_extinct_county_reports_negative_deaths_or_the_people_it_lost() {
    let (faithful, fixed) = pair(Quirk::ExtinctCountyRecordsNegativeDeaths);

    let run = |quirks: Quirks| {
        let mut c = County::new();
        // One person, the worst health band and no happiness, in Winter. Births:
        // Pct(1, Pct(100, 25)) = 0, floored to 1. Deaths: Pct(1, 35 + 8) = 0,
        // floored to 1, +2 for Diseased, and +1 because the scaled birth rate
        // of 25 is below 43. So 1 + 1 − 4 = −2, and the `pop < 1` arm runs.
        c.population = 1;
        c.health_band = 0;
        c.happiness = 0;
        l2_kingdom::population::update_one(T, &mut c, Season::Winter, quirks);
        (c.population, c.deaths)
    };

    let (pop_a, deaths_a) = run(faithful);
    let (pop_b, deaths_b) = run(fixed);
    assert_eq!(pop_a, 0);
    assert_eq!(pop_b, 0, "the county dies out either way — only the record differs");
    assert_eq!(deaths_a, -2, "reproduced: the county lost its last person and recorded {deaths_a}");
    assert_eq!(deaths_b, 1, "fixed: the one person who died");
}

/// **B16 has one face, and `docs/bugs.md` describes it.**
///
/// This test used to say the opposite — that the season a county loses its last
/// person lands on, and the negative number only appears the season
/// after, over a county that is already empty — and backed it with a survey that
/// found no negative case starting from a living county. **That was true of our
/// arithmetic and not of the original's.** We sent the season's extra person to
/// the births whenever the birth ladder's *unscaled* rate beat the death rate,
/// and a county of one is on the ladder's 100% rung, which beats every death rate
/// there is. `Population_UpdateAll` (`0x00449EF3`) compares the rate after
/// happiness has scaled it (C170), so at low happiness the extra
/// person is a death and the county records a negative count on the season it
/// dies — exactly B16's *"`deaths = pop` with `pop` already negative"*.
///
/// An empty county then goes on recording a negative number every season, and
/// the fixed path never records one at all. The survey below is the old one
/// turned round, so that the retraction is asserted.
#[test]
fn b16_the_negative_number_appears_on_the_season_the_county_dies() {
    let (faithful, fixed) = pair(Quirk::ExtinctCountyRecordsNegativeDeaths);

    let empty = |quirks: Quirks| {
        let mut c = County::new();
        c.population = 0; // died out last season
        c.health_band = 0;
        c.happiness = 0;
        l2_kingdom::population::update_one(T, &mut c, Season::Winter, quirks);
        c.deaths
    };
    // Births floored to 1; deaths floored to 1, +2, +1: 0 + 1 − 4.
    assert_eq!(empty(faithful), -3, "reproduced: minus three people died in an empty county");
    assert_eq!(empty(fixed), 0, "fixed: nobody was there to die");

    let mut alive_and_negative = 0usize;
    for population in 1..=400i32 {
        for health_band in 0..=4u8 {
            for happiness in [0, 25, 50, 75, 100] {
                for season in [Season::Winter, Season::Spring, Season::Summer, Season::Autumn] {
                    for (quirks, reproduced) in [(faithful, true), (fixed, false)] {
                        let mut c = County::new();
                        c.population = population;
                        c.health_band = health_band;
                        c.happiness = happiness;
                        l2_kingdom::population::update_one(T, &mut c, season, quirks);
                        if !reproduced {
                            assert!(
                                c.deaths >= 0,
                                "fixed: a county of {population} recorded {} deaths",
                                c.deaths
                            );
                        } else if c.deaths < 0 {
                            alive_and_negative += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(
        alive_and_negative > 0,
        "reproduced: no living county recorded a negative count — the season of death \
         lands on zero again, which is what comparing the unscaled birth rate did"
    );
}

// ---------------------------------------------------------------------------
// B17 — the AI unrest ladder has a dead band from happiness 1 to 10
// ---------------------------------------------------------------------------

/// **Every value in the band, not one of them.** C26 exactly: the dead band is
/// ten inputs wide and a fixture that used only happiness 5 would say nothing
/// about the edges, which are where an off-by-one lives.
#[test]
fn b17_an_ai_county_in_the_dead_band_is_frozen_or_climbs() {
    let (faithful, fixed) = pair(Quirk::AiUnrestDeadBand);

    for happiness in 1..=10 {
        let run = |quirks: Quirks| {
            let mut c = County::new();
            c.owner = 2; // an AI's county
            c.happiness = happiness;
            let mut out = Vec::new();
            for _ in 0..4 {
                l2_kingdom::unrest::update(&mut c, 1, false, quirks, &mut out);
            }
            c.unrest
        };
        assert_eq!(run(faithful), 0, "happiness {happiness} is frozen in the original");
        assert_eq!(
            run(fixed),
            l2_kingdom::unrest::UNREST_REVOLT,
            "happiness {happiness} climbs to revolt once the hole is closed"
        );
    }
}

/// **The three rungs either side of the band are untouched.** A fix that moved
/// the walk-down instead, or that widened the reset, would change these — and
/// would be a third ladder belonging to neither setting.
#[test]
fn b17_the_rest_of_the_ai_ladder_is_identical_either_way() {
    let (faithful, fixed) = pair(Quirk::AiUnrestDeadBand);
    for happiness in (0..=100).filter(|h| !(1..=10).contains(h)) {
        let run = |quirks: Quirks, start: u8| {
            let mut c = County::new();
            c.owner = 2;
            c.happiness = happiness;
            c.unrest = start;
            let mut out = Vec::new();
            l2_kingdom::unrest::update(&mut c, 1, false, quirks, &mut out);
            c.unrest
        };
        for start in 0..=4u8 {
            assert_eq!(
                run(faithful, start),
                run(fixed, start),
                "happiness {happiness} from unrest {start} is outside the band"
            );
        }
    }
}

