//! **The reproduction from the shipped save.**
//!
//! `docs/kingdom.md` §9 runs the field map in §1 against the `lastturn.sav` in
//! a real install — a turn-1 autosave of the England map — and lands eight
//! independent predictions. This file asserts them against this crate, driven
//! through the real [`Kingdom::advance_season`] pipeline rather than by calling
//! the rules directly.
//!
//! The reason this file matters more than the crate does: §9's chain is
//! **ration → health meter → health band → happiness → birth rate →
//! population**, five stages with no free parameters, ending on two different
//! happiness bands that produce two different birth counts. Getting the last
//! number right by accident is not available. If this file passes, the
//! ordering in `SEASON_PIPELINE`, the inclusive sense of the health ladder, the
//! index order of the health delta table, the 1-based season array, the birth
//! ladder's pairing and the unowned happiness bonus are all simultaneously
//! right.
//!
//! Nothing here needs a game install. The numbers are quoted from
//! `docs/kingdom.md`, not read from a file.

use l2_kingdom::county::County;
use l2_kingdom::phase::SEASON_PIPELINE;
use l2_kingdom::tables::{health_band, Season, Weather, CASTLE_STARTING_TYPE};
use l2_kingdom::{Kingdom, Options};

/// The England map: fourteen counties, four owned by the human realm and ten
/// unowned.
const COUNTY_COUNT: usize = 14;
const OWNED: usize = 4;

/// Every county in the shipped save shares this pre-turn state. The starting
/// health of 65 is `docs/kingdom.md` §9 point 7's *"published dump of the
/// new-game presets … for a medium county"* — the one number in the chain that
/// comes from prior art rather than from the binary, and the one the chain
/// then pins from both ends.
const START_POPULATION: i32 = 417;
const START_HAPPINESS: i32 = 65;
const START_HEALTH_METER: i32 = 65;
const START_HERD: i32 = 67;

/// Build the kingdom the save was taken from, one season before the save.
fn england_before_the_first_season() -> Kingdom {
    let mut k = Kingdom::new(0x10D5_2);

    // g_optDifficulty 0, g_optAdvancedFarming 0, g_optArmiesEat 0 - the three
    // option bytes docs/kingdom.md §9 reads out of the save.
    k.options = Options { difficulty: 0, advanced_farming: false, armies_eat: false };

    assert!(k.set_county_count(COUNTY_COUNT), "14 counties must fit in the array");

    // g_localPlayer = 1, and it is a person.
    k.realms[1].in_play = true;
    k.realms[1].is_human = true;
    k.realms[1].lord = 0;
    k.realms[1].county_count = OWNED as u8;

    for id in 1..=COUNTY_COUNT {
        let owner = if id <= OWNED { 1 } else { 0 };
        let c = &mut k.counties[id];
        c.owner = owner;
        c.population = START_POPULATION;
        c.happiness = START_HAPPINESS;
        c.health_meter = START_HEALTH_METER;
        c.health_band = health_band(START_HEALTH_METER);
        c.herd = START_HERD;
        c.ration_wanted = 3; // Normal
        c.ration_split = 100; // all from livestock
        c.tax_rate = 0;
        // Every player-owned county in the shipped save has castleType = 3.
        c.castle_type = if owner == 1 { CASTLE_STARTING_TYPE } else { 0 };
        // Fully connected, so migration is actually exercised rather than
        // skipped - and still moves nobody, which is the point.
        for n in 1..=COUNTY_COUNT as u8 {
            if n as usize != id {
                c.add_neighbour(n);
            }
        }
    }
    k
}

fn england_after_the_first_season() -> Kingdom {
    let mut k = england_before_the_first_season();
    k.start_new_game();
    k
}

/// §9 point 1: **`g_season` = 4, `g_year` = 1268, `g_turnCount` = 1** — the
/// prediction §3.3 made from `Game_NewGame`'s constants plus one
/// `Season_Advance`.
#[test]
fn a_new_game_reads_back_as_winter_1268_turn_one() {
    let k = england_after_the_first_season();
    assert_eq!(k.season, 4);
    assert_eq!(k.season(), Some(Season::Winter));
    assert_eq!(k.year, 1268);
    assert_eq!(k.turn_count, 1);
    assert_eq!(k.season_next, 1, "Spring is next");
}

/// §9 point 2: **`g_countyCount` = 14**, and county records 15 and 16 are all
/// zero while 1 … 14 are populated — the 17-record array with an unused
/// index 0.
#[test]
fn the_array_holds_seventeen_records_and_only_fourteen_are_a_county() {
    let k = england_after_the_first_season();
    assert_eq!(k.county_count, 14);
    assert_eq!(k.counties.len(), 17);

    let empty = County::new();
    assert_eq!(k.counties[0], empty, "record 0 is never a county");
    assert_eq!(k.counties[15], empty);
    assert_eq!(k.counties[16], empty);
    for id in 1..=COUNTY_COUNT {
        assert_ne!(k.counties[id], empty, "county {id} should be populated");
    }
}

/// §9 point 3: **`g_optAdvancedFarming` = 0, and every county's weather byte is
/// 3 (Cloudy) and every fertility is 0** — exactly the two overrides §7.2 and
/// §7.3 say the option forces.
#[test]
fn basic_farming_leaves_every_county_cloudy_with_zero_fertility() {
    let k = england_after_the_first_season();
    for id in 1..=COUNTY_COUNT {
        assert_eq!(k.counties[id].weather, Weather::Cloudy, "county {id}");
        assert_eq!(k.counties[id].weather.index(), 3, "the stored byte is 3");
        assert_eq!(k.counties[id].fertility, 0, "county {id}");
    }
}

/// §9 point 6: **the tax term reproduces.** `taxRate` is 0 everywhere, and
/// `dHapTax = 5 - 0 = 5`.
#[test]
fn the_tax_term_reproduces_across_the_whole_map() {
    let k = england_after_the_first_season();
    for id in 1..=COUNTY_COUNT {
        assert_eq!(k.counties[id].tax_rate, 0);
        assert_eq!(k.counties[id].shown_tax, 5, "county {id}");
        assert_eq!(k.counties[id].tax_collected, 0, "a rate of 0 banks nothing");
    }
    assert_eq!(k.realms[1].gold, 0);
    assert_eq!(k.realms[1].tax_hap_empire, 0, "no county is above the free rate");
}

/// §9 point 7: **the health chain reproduces.** A starting meter of 65 bands as
/// **2** (`65 <= 65`); `g_healthDeltaTable[Normal][2]` is **+2**, giving 67;
/// and 67 bands as **3** (`67 <= 90`). The save stores meter 67 and band 3.
///
/// One number pins the ladder's comparison sense and the delta table's indexing
/// at once.
#[test]
fn the_health_chain_reproduces_across_the_whole_map() {
    assert_eq!(health_band(START_HEALTH_METER), 2, "65 is Average on the ladder");
    let k = england_after_the_first_season();
    for id in 1..=COUNTY_COUNT {
        assert_eq!(k.counties[id].health_meter, 67, "county {id}");
        assert_eq!(k.counties[id].health_band, 3, "county {id}");
        assert_eq!(k.counties[id].shown_health, 1, "Good health is +1 happiness");
    }
}

/// §9 point 5: **happiness reproduces exactly.** Every county has
/// `happinessLast = 65`, `shownTax = +5`, `shownHealth = +1`,
/// `shownRation = +1`. Player-owned counties store **72 = 65 + 5 + 1 + 1**;
/// unowned counties store **77**, with `shownEvents = +5`.
///
/// Fourteen counties, two cases.
#[test]
fn both_happiness_cases_reproduce_exactly() {
    let k = england_after_the_first_season();
    for id in 1..=COUNTY_COUNT {
        let c = &k.counties[id];
        assert_eq!(c.happiness_last, 65, "county {id}");
        assert_eq!((c.shown_tax, c.shown_health, c.shown_ration), (5, 1, 1), "county {id}");

        if id <= OWNED {
            assert_eq!(c.happiness, 72, "owned county {id}: 65 + 5 + 1 + 1");
            assert_eq!(c.shown_events, 0);
        } else {
            assert_eq!(c.happiness, 77, "unowned county {id}: 72 + the unowned bonus");
            assert_eq!(c.shown_events, 5);
        }
        assert_eq!(c.happiness_sum, c.happiness, "turn 1: the sum is this turn");
        assert_eq!(c.happiness_avg, c.happiness);
    }
}

/// §9 point 8: **births and deaths reproduce exactly, on two different
/// happiness bands.** Every county started at `popLast = 417`, so
/// `g_birthRateLadder` gives 20%:
///
/// | | happiness | factor | births | deaths | population |
/// |---|---:|---:|---:|---:|---:|
/// | owned (4 counties) | 72 | 75% | `Pct(417, Pct(20,75)) + 1` = **63** | `Pct(417, 3+8)` = **45** | **435** |
/// | unowned (10 counties) | 77 | 100% | `Pct(417, 20) + 1` = **84** | **45** | **456** |
///
/// The deaths figure needs `g_deathRateByHealth[3] = 3` **and**
/// `g_deathRateBySeason[4] = 8`, so it also confirms the season index
/// independently.
#[test]
fn both_population_rows_reproduce_exactly() {
    let k = england_after_the_first_season();
    for id in 1..=COUNTY_COUNT {
        let c = &k.counties[id];
        assert_eq!(c.pop_last, 417, "county {id}");
        assert_eq!(c.deaths, 45, "county {id}: Pct(417, 3 + 8)");

        if id <= OWNED {
            assert_eq!(c.births, 63, "owned county {id}");
            assert_eq!(c.population, 435, "417 + 63 - 45");
        } else {
            assert_eq!(c.births, 84, "unowned county {id}");
            assert_eq!(c.population, 456, "417 + 84 - 45");
        }
    }
}

/// `popBand` (`+0xB8`) also checks: `(435-1)/25 + 1 = 18` and
/// `(456-1)/25 + 1 = 19`, which are the stored values.
#[test]
fn the_population_band_reproduces_for_both_cases() {
    let k = england_after_the_first_season();
    for id in 1..=COUNTY_COUNT {
        let expected = if id <= OWNED { 18 } else { 19 };
        assert_eq!(k.counties[id].pop_band, expected, "county {id}");
    }
}

/// §4.3's food split: an unowned county with population 456, herd 67, ration
/// *Normal*, split 100% gives `456 - 67x5 = 121` people left to feed and
/// `DivCeil(121, 10) = 13` head — the stored `+0x17C`, for all ten unowned
/// counties.
///
/// Asserted against the documented inputs directly rather than against the end
/// of a season, because **the season does not leave the county in that state**:
/// the herd stored in the save is 67 *after* a ration pass that this crate
/// models as spending 9 head out of a starting 67. Either `Ration_Apply` does
/// not debit the store at all, or the county started the turn with 73 head.
/// `docs/kingdom.md` §4.3 records that it *"did not untangle which write
/// survives"*, and this is the same knot from the other side.
#[test]
fn the_documented_food_split_reproduces_from_its_stated_inputs() {
    let mut c = County::new();
    c.population = 456;
    c.herd = 67;
    c.ration_wanted = 3;
    c.ration_split = 100;

    let plan = l2_kingdom::ration::choose(&c, false);
    assert_eq!(plan.dairy, 335, "67 head feeding five people each");
    assert_eq!(plan.requirement - plan.dairy, 121, "people left to feed");
    assert_eq!(plan.heads, 13, "the stored +0x17C");
    assert_eq!(plan.level, 3, "still Normal rations");
}

/// Every county in the save is on Normal rations and takes the +1, which is the
/// third term of the happiness sum the chain depends on.
#[test]
fn every_county_is_fed_at_normal_rations() {
    let k = england_after_the_first_season();
    for id in 1..=COUNTY_COUNT {
        // The *stored* ration_achieved is next season's preview - which is
        // itself one of docs/kingdom.md §4.3's findings - but it is still
        // Normal, and the +1 that went into this season's happiness is
        // recorded in shown_ration.
        assert_eq!(k.counties[id].ration_achieved, 3, "county {id}");
        assert_eq!(k.counties[id].shown_ration, 1, "county {id}");
    }
}

/// Migration is exercised — every county neighbours every other — and moves
/// **nobody**, which is what the save's arithmetic requires: `435 = 417+63-45`
/// and `456 = 417+84-45` leave no room for a migrant.
///
/// It is not a coincidence. An owned county at 72 beside an unowned one at 77
/// computes `Pct(77-72, (100-72)/3) = Pct(5, 9) = 0`, and an unowned county has
/// no happier neighbour at all.
#[test]
fn migration_runs_and_moves_nobody_which_is_what_the_save_requires() {
    let k = england_after_the_first_season();
    for id in 1..=COUNTY_COUNT {
        assert_eq!(k.counties[id].emigrants, 0, "county {id}");
        assert_eq!(k.counties[id].immigrants, 0, "county {id}");
        assert!(k.counties[id].neighbour_count > 0, "county {id} must have neighbours");
    }
}

/// §8.1's gate: the AI never draws random events, and nobody draws one in 1268.
#[test]
fn no_county_draws_an_event_in_the_first_year() {
    let k = england_after_the_first_season();
    for id in 1..=COUNTY_COUNT {
        assert!(!k.counties[id].event_fired, "county {id}");
        assert_eq!(k.counties[id].event_population_pct, 0);
    }
}

/// The season really did run the whole documented pipeline, in order.
#[test]
fn the_first_season_ran_every_documented_pass_in_order() {
    let mut k = england_before_the_first_season();
    let report = k.start_new_game();
    assert_eq!(report.passes, SEASON_PIPELINE.to_vec());
    assert!(report.messages.is_empty(), "a happy kingdom raises no messages");
    assert!(report.revolts.is_empty());
}

/// The five-stage chain, stated as one assertion over the whole map: the
/// ration level fed the health delta, the health delta fed the band, the band
/// fed the happiness, the happiness fed the birth factor, and the birth factor
/// fed the population — twice, on two different bands.
#[test]
fn the_whole_five_stage_chain_lands_on_both_bands() {
    let k = england_after_the_first_season();

    let owned = &k.counties[1];
    let unowned = &k.counties[COUNTY_COUNT];

    // ration -> health meter -> health band
    assert_eq!((owned.shown_ration, owned.health_meter, owned.health_band), (1, 67, 3));
    assert_eq!((unowned.shown_ration, unowned.health_meter, unowned.health_band), (1, 67, 3));

    // health band -> happiness
    assert_eq!((owned.shown_health, owned.happiness), (1, 72));
    assert_eq!((unowned.shown_health, unowned.happiness), (1, 77));

    // happiness -> birth factor -> population
    assert_eq!((owned.births, owned.deaths, owned.population), (63, 45, 435));
    assert_eq!((unowned.births, unowned.deaths, unowned.population), (84, 45, 456));
}

/// And the whole thing is deterministic: the same kingdom, run again, is the
/// same kingdom. Lockstep needs this and nothing else needs it more.
#[test]
fn the_reproduction_is_bit_identical_run_to_run() {
    let a = england_after_the_first_season();
    for _ in 0..8 {
        assert_eq!(a, england_after_the_first_season());
    }
}

/// Ten more seasons past the reproduction, to show the model does not merely
/// land on turn 1 and then diverge into nonsense: every county stays inside
/// every documented bound.
#[test]
fn ten_more_seasons_keep_every_value_inside_its_documented_range() {
    let mut k = england_after_the_first_season();
    for season in 1..=10 {
        k.advance_season();
        for id in 1..=COUNTY_COUNT {
            let c = &k.counties[id];
            assert!((0..=100).contains(&c.happiness), "county {id} season {season}");
            assert!((0..=100).contains(&c.health_meter), "county {id} season {season}");
            assert!(c.health_band <= 4, "county {id} season {season}");
            assert!(c.unrest <= 4, "county {id} season {season}");
            assert!(c.population >= 0, "county {id} season {season}");
            assert!(c.herd >= 0, "county {id} season {season}");
            assert!(c.grain >= 0, "county {id} season {season}");
            assert!((-100..=100).contains(&c.fertility), "county {id} season {season}");
            assert!((0..=5).contains(&c.ration_achieved), "county {id} season {season}");
        }
        assert!((1..=4).contains(&k.season));
    }
}
