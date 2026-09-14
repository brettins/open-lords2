#![allow(unused_imports)]
use super::*;
use super::b11a_trade::*;
use super::b12_b15_quirks::*;
use super::*;
use super::economy::*;
use super::victory::*;
use super::wire::*;
use l2_kingdom::county::County;
use l2_kingdom::kingdom::Kingdom;
use l2_kingdom::realm::Realm;
use l2_kingdom::tables::{Season, Tables, Weather};
use l2_kingdom::{Quirk, Quirks};

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


