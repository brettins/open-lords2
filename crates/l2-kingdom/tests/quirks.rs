//! **Every switch, flipped, with the simulation watched.**
//!
//! `docs/agents.md`: *a field is only tested if something a test reads was
//! written by something the game runs.* The quirk-shaped version of that, and
//! the reason this file exists rather than a comment saying the flags work:
//!
//! > **A quirk switch that nothing reads is worse than no switch**, because it
//! > claims a behaviour is configurable when it is not.
//!
//! So there is one test per [`Quirk`], each of the same shape: run the *same*
//! rule twice over the *same* state, once faithful and once fixed, and assert
//! the two answers differ **and** that each is the answer it should be. Asserting
//! only that they differ would pass for a switch that broke the rule in some
//! third way.
//!
//! `crates/l2-testkit/tests/quirks_catalogue.rs` is the other half: it checks
//! that the switch list and `docs/bugs.md` are the same list. It can tell that a
//! variant is *named* by the simulation; only this file can tell that flipping
//! it changes an answer.
//!
//! # The two that are about the wire, not about a rule
//!
//! [`the_quirk_set_is_inside_the_lockstep_digest`] and
//! [`a_saved_game_remembers_which_bugs_it_was_played_with`] are the ones that
//! would have caught the failure this project has had five times — a field in
//! neither the save nor the digest, with a green suite over it
//! (`docs/decisions.md` C30). A quirk that did not reach the digest would let
//! two peers with different settings agree on a checksum while computing
//! different games, which is the exact defect this engine exists to replace.

use l2_kingdom::county::County;
use l2_kingdom::kingdom::Kingdom;
use l2_kingdom::realm::Realm;
use l2_kingdom::tables::{Season, Tables, Weather};
use l2_kingdom::{Quirk, Quirks};

const T: &Tables = &Tables::DEFAULT;

/// The two settings for one quirk, so every test below reads the same way:
/// `(faithful, fixed)`.
fn pair(q: Quirk) -> (Quirks, Quirks) {
    let faithful = Quirks::FAITHFUL;
    let mut fixed = Quirks::FAITHFUL;
    fixed.set_reproduced(q, false);
    assert!(faithful.reproduces(q) && !fixed.reproduces(q));
    (faithful, fixed)
}

// ---------------------------------------------------------------------------
// B1 — the harvest weather band throws away the labour cap
// ---------------------------------------------------------------------------

/// One reaper, a huge standing crop, and a sunny sky. The original stores three
/// halves of *everything the county grew*; fixed, it stores three halves of what
/// the one reaper could actually carry.
#[test]
fn b1_a_single_reaper_in_a_sunny_field_reaps_the_whole_county_or_does_not() {
    let (faithful, fixed) = pair(Quirk::HarvestIgnoresLabourCap);

    let county = || {
        let mut c = County::new();
        c.fields_grain = 6;
        c.crop[1] = 1200;
        c.labour[T.job.grain_farming] = 1; // one man
        c.weather = Weather::Sunny;
        c
    };

    let mut a = county();
    l2_kingdom::land::harvest(T, &mut a, true, faithful);
    let mut b = county();
    l2_kingdom::land::harvest(T, &mut b, true, fixed);

    assert_eq!(a.crop[2], 1800, "3/2 of the standing crop, however few reapers");
    assert!(
        b.crop[2] < a.crop[2],
        "fixed, the weather scales what was reaped: {} against {}",
        b.crop[2],
        a.crop[2]
    );
    assert_eq!(a.grain, a.crop[2], "and the store gets what was harvested");
    assert_eq!(b.grain, b.crop[2]);
}

/// **The half that would hide a wrong fix.** *Cloudy* and *Drought* are the two
/// bands the original already leaves alone, so the switch must change nothing
/// there — a fix that "helped" in six bands instead of four would be a third
/// behaviour belonging to neither setting.
#[test]
fn b1_the_two_bands_that_were_never_wrong_are_untouched() {
    let (faithful, fixed) = pair(Quirk::HarvestIgnoresLabourCap);
    for weather in [Weather::Cloudy, Weather::Drought] {
        let county = || {
            let mut c = County::new();
            c.fields_grain = 6;
            c.crop[1] = 1200;
            c.labour[T.job.grain_farming] = 1;
            c.weather = weather;
            c
        };
        let mut a = county();
        l2_kingdom::land::harvest(T, &mut a, true, faithful);
        let mut b = county();
        l2_kingdom::land::harvest(T, &mut b, true, fixed);
        assert_eq!(a.crop[2], b.crop[2], "{weather:?} was never the bug");
    }
}

// ---------------------------------------------------------------------------
// B2 — half the counties can never draw a random event
// ---------------------------------------------------------------------------

/// The original's parity lock, and its absence, measured the way `event.rs`
/// measures it: **exhaustively over every seed**, not sampled. "We never saw an
/// even county draw" and "an even county cannot draw" are different claims.
#[test]
fn b2_even_numbered_counties_draw_only_when_the_quirk_is_off() {
    let (faithful, fixed) = pair(Quirk::EventDeckParityLocksOutEvenCounties);

    let evens_that_drew = |quirks: Quirks| {
        let mut seen = std::collections::BTreeSet::new();
        for seed in 0..4096u64 {
            let mut k = furnished_kingdom(seed);
            k.options.quirks = quirks;
            let report = k.advance_season();
            for m in &report.messages {
                if let l2_kingdom::report::Message::Event { county, .. } = m {
                    if county % 2 == 0 {
                        seen.insert(*county);
                    }
                }
            }
        }
        seen
    };

    assert!(
        evens_that_drew(faithful).is_empty(),
        "the deck's parity is the bug: no even county can draw at any seed"
    );
    assert!(
        !evens_that_drew(fixed).is_empty(),
        "with the quirk off, even counties draw like everybody else"
    );
}

/// **The determinism property, and it is not optional.** Both settings must take
/// the *same number* of values from the generator, or a quirk would move every
/// later draw in the season and a desync dump would name the wrong subsystem.
#[test]
fn b2_the_number_of_random_values_drawn_does_not_depend_on_the_setting() {
    let (faithful, fixed) = pair(Quirk::EventDeckParityLocksOutEvenCounties);
    let after = |quirks: Quirks| {
        let mut k = furnished_kingdom(7);
        k.options.quirks = quirks;
        k.advance_season();
        k.rng.clone()
    };
    assert_eq!(
        after(faithful),
        after(fixed),
        "the generator must be in the same place either way"
    );
}

// ---------------------------------------------------------------------------
// B3 — the weapon a county finds is chosen by its county number
// ---------------------------------------------------------------------------

#[test]
fn b3_a_found_weapon_follows_the_county_id_or_the_countys_own_smithy() {
    let (faithful, fixed) = pair(Quirk::FoundWeaponFollowsCountyId);
    // Armour, weapon type 5: a slot `(id & 3) + 1` can never produce.
    let county = || {
        let mut c = County::new();
        c.owner = 1;
        c.weapon_type = 5;
        c.population = 400;
        c
    };
    for id in 1..=8usize {
        let mut a = county();
        let mut pa = l2_kingdom::event::RealmPurse::default();
        assert!(l2_kingdom::event::fire(
            &mut a,
            id,
            &mut pa,
            l2_kingdom::event::EventKind::WeaponsFound,
            Season::Spring,
            faithful
        ));
        let mut b = county();
        let mut pb = l2_kingdom::event::RealmPurse::default();
        assert!(l2_kingdom::event::fire(
            &mut b,
            id,
            &mut pb,
            l2_kingdom::event::EventKind::WeaponsFound,
            Season::Spring,
            fixed
        ));

        let found = |p: &l2_kingdom::event::RealmPurse| {
            (0..p.weapons.len()).find(|&s| p.weapons[s] != 0).expect("something was found")
        };
        assert_eq!(found(&pa), (id & 3) + 1, "county {id}, the original's slot");
        assert_eq!(found(&pb), 5, "county {id}, fixed: what the blacksmith makes");
    }
}

/// **The crossbow.** Slot 0 is unreachable for the whole game with the quirk on
/// — `docs/bugs.md` D6, the dead-code half of B3 — and reachable with it off.
/// This is the consequence a player could actually notice.
#[test]
fn b3_the_crossbow_can_be_found_only_with_the_quirk_off() {
    let (faithful, fixed) = pair(Quirk::FoundWeaponFollowsCountyId);
    for id in 1..=16usize {
        assert_ne!(l2_kingdom::event::weapon_slot(id, 0, faithful), 0, "county {id}");
    }
    assert_eq!(l2_kingdom::event::weapon_slot(9, 0, fixed), 0, "a crossbow county finds crossbows");
}

// ---------------------------------------------------------------------------
// B4 — the empire tax happiness term is summed into a signed byte
// ---------------------------------------------------------------------------

/// Sixteen counties taxed to the top. The original's byte wraps and the empire
/// ends up **happier**; fixed, it saturates at the floor of the byte it lives
/// in.
#[test]
fn b4_taxing_a_large_empire_hard_makes_it_happier_or_it_does_not() {
    let (faithful, fixed) = pair(Quirk::EmpireTaxHappinessWraps);

    let run = |quirks: Quirks| {
        let mut counties = vec![County::new(); 17];
        for c in counties.iter_mut().skip(1) {
            c.owner = 1;
            c.population = 500;
            c.tax_rate = l2_kingdom::tables::MAX_TAX_RATE;
        }
        let mut realms = vec![Realm::new(); l2_kingdom::realm::MAX_REALMS];
        l2_kingdom::tax::sum_empire_happiness(T, &mut counties, &mut realms, 16, quirks);
        realms[1].tax_hap_empire
    };

    let wrapped = run(faithful);
    let clamped = run(fixed);
    assert!(
        wrapped > 0,
        "the whole point of B4: sixteen counties of misery come out as +{wrapped}"
    );
    assert_eq!(clamped, i8::MIN, "fixed, it saturates at the floor rather than coming round");
    assert_ne!(wrapped, clamped);
}

/// A small empire cannot overflow the byte, so the switch must change nothing
/// there. C26's lesson: a rule can be wrong at 45 of 51 inputs and invisible at
/// the one the fixture uses — so the *unchanged* case is asserted too.
#[test]
fn b4_a_small_empire_is_the_same_number_either_way() {
    let (faithful, fixed) = pair(Quirk::EmpireTaxHappinessWraps);
    for count in 1..=4usize {
        let run = |quirks: Quirks| {
            let mut counties = vec![County::new(); 17];
            for c in counties.iter_mut().take(count + 1).skip(1) {
                c.owner = 1;
                c.tax_rate = l2_kingdom::tables::MAX_TAX_RATE;
            }
            let mut realms = vec![Realm::new(); l2_kingdom::realm::MAX_REALMS];
            l2_kingdom::tax::sum_empire_happiness(T, &mut counties, &mut realms, count, quirks);
            realms[1].tax_hap_empire
        };
        assert_eq!(run(faithful), run(fixed), "{count} counties cannot overflow the byte");
    }
}

// ---------------------------------------------------------------------------
// B10 — any ale at all fills a village under ten people
// ---------------------------------------------------------------------------

#[test]
fn b10_one_crown_of_ale_in_a_tiny_village_buys_five_happiness_or_one() {
    let (faithful, fixed) = pair(Quirk::AnyAleFillsATinyVillage);
    let county = || {
        let mut c = County::new();
        c.population = 5; // fewer than ten, so `population / step_pct` is 0
        c
    };
    let mut a = county();
    let mut b = county();
    assert_eq!(l2_kingdom::happiness::buy_ale(T, &mut a, 1, faithful), 5, "the whole ration");
    assert_eq!(l2_kingdom::happiness::buy_ale(T, &mut b, 1, fixed), 1, "one crown, one rung");
    assert_eq!(a.happiness - b.happiness, 4);
}

/// A village big enough for the step to be non-zero was never the bug.
#[test]
fn b10_a_real_village_is_the_same_ladder_either_way() {
    let (faithful, fixed) = pair(Quirk::AnyAleFillsATinyVillage);
    for population in [10, 50, 200, 435, 1000] {
        for crowns in [1, 5, 20, 100, 500] {
            let county = || {
                let mut c = County::new();
                c.population = population;
                c
            };
            let (mut a, mut b) = (county(), county());
            assert_eq!(
                l2_kingdom::happiness::buy_ale(T, &mut a, crowns, faithful),
                l2_kingdom::happiness::buy_ale(T, &mut b, crowns, fixed),
                "population {population}, {crowns} crowns"
            );
        }
    }
}

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
    // free slot, so it is full rather than short.
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
        // One person, the worst health band (which adds 2 deaths outright) and
        // no happiness: the pass takes the county under one and the `pop < 1`
        // arm runs. Nothing here is asserted on - the assertions are on what
        // `update_one` writes.
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
    assert_eq!(deaths_a, 0, "reproduced: the county lost its last person and recorded {deaths_a}");
    assert_eq!(deaths_b, 1, "fixed: the one person who died");
}

/// **Where B16's negative number actually comes from**, which is not where the
/// catalogue entry reads as though it is.
///
/// `docs/bugs.md` B16 says *"A county that dies out records a negative death
/// count. `deaths = pop` with `pop` already negative."* On the season a county
/// **loses its last person** the arithmetic lands on exactly 0, so the stored
/// figure is 0 — wrong, but not negative. The negative number appears on the
/// **next** season, when the pass runs again over a county that is already
/// empty and drives it to −1.
///
/// So the defect has two faces and the entry describes the second one. Both are
/// switched by the same flag and both are asserted here, because a test that
/// only exercised the season of death would have called the switch inert.
/// Established by walking population 0..=400 × five health bands × five
/// happiness values × four seasons × four event modifiers and finding that
/// **every** negative case has `population == 0` going in.
#[test]
fn b16_the_negative_number_appears_the_season_after_the_county_is_already_empty() {
    let (faithful, fixed) = pair(Quirk::ExtinctCountyRecordsNegativeDeaths);

    let empty = |quirks: Quirks| {
        let mut c = County::new();
        c.population = 0; // died out last season
        c.health_band = 0;
        c.happiness = 0;
        l2_kingdom::population::update_one(T, &mut c, Season::Winter, quirks);
        c.deaths
    };
    assert_eq!(empty(faithful), -1, "reproduced: minus one person died in an empty county");
    assert_eq!(empty(fixed), 0, "fixed: nobody was there to die");

    // And the survey behind the paragraph above: no negative case exists that
    // did not start empty.
    for population in 1..=400i32 {
        for health_band in 0..=4u8 {
            for happiness in [0, 25, 50, 75, 100] {
                for season in [Season::Winter, Season::Spring, Season::Summer, Season::Autumn] {
                    let mut c = County::new();
                    c.population = population;
                    c.health_band = health_band;
                    c.happiness = happiness;
                    l2_kingdom::population::update_one(T, &mut c, season, faithful);
                    assert!(
                        c.deaths >= 0,
                        "a county of {population} went negative — B16's first face is reachable \
                         after all, and this comment is now wrong"
                    );
                }
            }
        }
    }
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

// ---------------------------------------------------------------------------
// B42 — a mercenary band overshoots a county after making an offer
// ---------------------------------------------------------------------------

#[test]
fn b42_a_band_that_made_an_offer_stands_off_the_end_of_the_map_or_does_not() {
    let (faithful, fixed) = pair(Quirk::MercenaryBandOvershoots);

    let overshot = |quirks: Quirks| {
        let mut bands = l2_kingdom::MercenaryBands::init(14);
        let mut counties: [County; l2_kingdom::county::MAX_COUNTIES] =
            core::array::from_fn(|_| County::new());
        let mut seen = false;
        for _ in 0..64 {
            bands.advance(&mut counties, 14, quirks);
            for band in 1..l2_kingdom::mercenary::BAND_SLOTS {
                if bands.band_raw(band).next_county > 14 {
                    seen = true;
                }
            }
        }
        seen
    };

    assert!(overshot(faithful), "reproduced: a band sits at countyCount + 1 for a season");
    assert!(!overshot(fixed), "fixed: the second increment gets the wrap guard the first has");
}

// ---------------------------------------------------------------------------
// B51, B52, B53 — victory, defeat and the score
// ---------------------------------------------------------------------------

#[test]
fn b51_a_game_with_nobody_in_it_is_won_by_the_array_slot_or_by_nobody() {
    let (faithful, fixed) = pair(Quirk::EmptyGameIsWonBySlotZero);

    let endings = |quirks: Quirks| {
        // Nobody in play at all: leader and trailer are both 0 and `0 == 0`.
        let mut realms: [Realm; l2_kingdom::realm::MAX_REALMS] =
            core::array::from_fn(|_| Realm::new());
        for r in realms.iter_mut() {
            r.in_play = false;
            r.strength = 0;
        }
        let mut out = Vec::new();
        l2_kingdom::victory::rank_and_crown(T, &mut realms, 1, quirks, &mut out);
        out
    };

    assert!(!endings(faithful).is_empty(), "reproduced: the empty game crowns realm 0");
    assert!(endings(fixed).is_empty(), "fixed: nobody standing, nobody crowned");
}

#[test]
fn b52_the_human_wins_a_game_the_human_is_dead_in_or_does_not() {
    let (faithful, fixed) = pair(Quirk::DeadHumanCanStillWin);

    let second_call = |quirks: Quirks| {
        // Realm 2, an AI, is the only one standing. The local player is 1 and is
        // gone. `Score_RankRealms` runs many times a turn.
        let mut realms: [Realm; l2_kingdom::realm::MAX_REALMS] =
            core::array::from_fn(|_| Realm::new());
        for r in realms.iter_mut() {
            r.in_play = false;
            r.strength = 0;
        }
        realms[2].in_play = true;
        realms[2].strength = 9;
        realms[2].is_human = false;
        realms[2].lord = 1;

        let mut out = Vec::new();
        l2_kingdom::victory::rank_and_crown(T, &mut realms, 1, quirks, &mut out);
        out.clear();
        l2_kingdom::victory::rank_and_crown(T, &mut realms, 1, quirks, &mut out);
        out
    };

    let a = second_call(faithful);
    assert!(
        a.iter().any(|e| e.group == l2_kingdom::victory::MSG_VICTORY),
        "reproduced: the second call sends the dead human group 225"
    );
    assert!(
        second_call(fixed).is_empty(),
        "fixed: a victory goes only to a local player who is the realm left standing"
    );
}

#[test]
fn b53_dying_with_the_last_opponent_is_a_win_or_a_loss() {
    use l2_kingdom::victory::{Ending, Outcome, OutcomeStep, Ranking, CATEGORY_ENDING};
    let (faithful, fixed) = pair(Quirk::MutualDestructionIsAWin);

    // My own defeat message, on the pass where no opponent is left either.
    let mine = Ending { group: l2_kingdom::victory::MSG_DEFEAT, from: 1, to: 1, category: CATEGORY_ENDING };
    let ranking = Ranking { opponents_remaining: 0, ..Ranking::default() };

    assert_eq!(
        l2_kingdom::victory::outcome_of(mine, 1, ranking, faithful),
        OutcomeStep::EnqueueVictory,
        "reproduced: the opponents test is asked first, so my own defeat wins the game"
    );
    assert_eq!(
        l2_kingdom::victory::outcome_of(mine, 1, ranking, fixed),
        OutcomeStep::Set(Outcome::Lost),
        "fixed: whose defeat this is, first"
    );
}

/// Somebody else's defeat with opponents left is the ordinary case, and the
/// switch must not touch it.
#[test]
fn b53_an_ordinary_elimination_is_the_same_step_either_way() {
    use l2_kingdom::victory::{Ending, Ranking, CATEGORY_ENDING};
    let (faithful, fixed) = pair(Quirk::MutualDestructionIsAWin);
    for opponents in 1..=4u8 {
        for from in 1..=5u8 {
            let msg = Ending {
                group: l2_kingdom::victory::MSG_AI_ELIMINATED,
                from,
                to: 0,
                category: CATEGORY_ENDING,
            };
            let r = Ranking { opponents_remaining: opponents, ..Ranking::default() };
            assert_eq!(
                l2_kingdom::victory::outcome_of(msg, 1, r, faithful),
                l2_kingdom::victory::outcome_of(msg, 1, r, fixed),
                "{opponents} left, message from {from}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// The wire: the digest and the save
// ---------------------------------------------------------------------------

/// **The quirk set is inside the per-tick lockstep digest.**
///
/// `l2_kingdom::save::checksum` is `Canonical::hash_of(kingdom)` and is the
/// number two peers exchange every tick. If a quirk did not reach it, two
/// players with different settings would agree on every checksum they exchanged
/// while computing different games — which is the exact defect this engine
/// exists to replace, arrived at from the other direction. `docs/netcode.md` §6.
///
/// Asserted for **every** quirk, one at a time, because a bitfield that lost one
/// bit on the way out would still change the hash for the other thirteen.
#[test]
fn the_quirk_set_is_inside_the_lockstep_digest() {
    let base = furnished_kingdom(21);
    let faithful = l2_kingdom::save::checksum(&base);
    for q in Quirk::ALL {
        let mut k = base.clone();
        k.options.quirks.set_reproduced(*q, false);
        assert_ne!(
            l2_kingdom::save::checksum(&k),
            faithful,
            "flipping {} does not change the tick checksum — two peers that disagreed about it \
             would agree on every number they exchanged",
            q.name()
        );
    }
}

/// The `options` section is where it lands, so a desync dump names the right
/// subsystem rather than saying only "the state differs".
#[test]
fn a_quirk_difference_shows_up_as_the_options_section() {
    let base = furnished_kingdom(22);
    let mut other = base.clone();
    other.options.quirks.set_all(false);

    let digest = |k: &Kingdom| {
        let mut c = l2_net::Canonical::hashing();
        l2_net::Encode::encode(k, &mut c);
        c.finish()
    };
    assert_eq!(
        digest(&base).sole_difference(&digest(&other)),
        Some("options"),
        "a quirk change must localise to `options` and nowhere else"
    );
}

/// **A saved game remembers which bugs it was played with.**
///
/// Not a formality: a save that dropped the field would reload a fixed game as a
/// faithful one, and the county figures would start drifting from the ones the
/// player left. `docs/decisions.md` C30's shape, for the sixth time.
#[test]
fn a_saved_game_remembers_which_bugs_it_was_played_with() {
    for quirks in [Quirks::FAITHFUL, Quirks::FIXED, mixed()] {
        let mut k = furnished_kingdom(23);
        k.options.quirks = quirks;
        let bytes = l2_kingdom::save::encode(&k);
        let back = l2_kingdom::save::decode(&bytes, Tables::DEFAULT).expect("round trip");
        assert_eq!(back.options.quirks, quirks);
        assert_eq!(back.options.quirks.group(), quirks.group());
    }
}

/// **Two kingdoms that differ only in their quirks really do play differently.**
///
/// The end-to-end claim, made once over the whole season pipeline rather than
/// per rule: run the same world forward under both settings and the states
/// diverge. A switch that only changed a flag would pass every test above that
/// calls one rule directly and fail this one.
#[test]
fn the_same_world_played_faithfully_and_fixed_ends_up_in_two_different_states() {
    let mut faithful = furnished_kingdom(24);
    let mut fixed = faithful.clone();
    fixed.options.quirks.set_all(false);

    for _ in 0..8 {
        faithful.advance_season();
        fixed.advance_season();
    }
    assert_ne!(
        l2_kingdom::save::checksum(&faithful),
        l2_kingdom::save::checksum(&fixed),
        "two years of the same world under both settings must not land in the same state"
    );
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// A quirk set with some of each, so the tri-state parent's middle is exercised
/// by the save round trip too.
fn mixed() -> Quirks {
    let mut q = Quirks::FAITHFUL;
    q.set_reproduced(Quirk::HarvestIgnoresLabourCap, false);
    q.set_reproduced(Quirk::MutualDestructionIsAWin, false);
    assert_eq!(q.group(), l2_net::Group::Mixed);
    q
}

/// A small kingdom with enough in it that a season does something.
///
/// **Nothing here asserts on a field this function writes.** It sets the world
/// up and the tests read what `advance_season` and the rule functions leave
/// behind — `docs/agents.md`'s rule about fixtures that check themselves.
fn furnished_kingdom(seed: u64) -> Kingdom {
    let mut k = Kingdom::new(seed);
    k.county_count = 8;
    k.year = 1300;
    k.year_next = 1301;
    k.season = 1;
    k.season_next = 2;
    for id in 1..=k.county_count {
        let c = &mut k.counties[id];
        c.owner = if id <= 4 { 1 } else { 2 };
        c.population = 400 + id as i32 * 10;
        c.happiness = 50;
        c.health_meter = 60;
        c.health_band = 2;
        c.grain = 2000;
        c.herd = 60;
        c.fields_grain = 6;
        c.fields_fallow = 2;
        c.tax_rate = 10;
        c.weather = Weather::Cloudy;
        c.labour[T.job.grain_farming] = 100;
        c.purse = 500;
    }
    for id in 1..=2 {
        let r = &mut k.realms[id];
        r.in_play = true;
        r.strength = 12;
        r.gold = 5000;
        r.lord = if id == 1 { 0 } else { 1 };
        r.is_human = id == 1;
    }
    k
}
