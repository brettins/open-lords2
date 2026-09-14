#![allow(unused_imports)]
use super::*;
use super::counties::*;
use super::victory::*;
use super::wire::*;
use l2_kingdom::county::County;
use l2_kingdom::kingdom::Kingdom;
use l2_kingdom::realm::Realm;
use l2_kingdom::tables::{Season, Tables, Weather};
use l2_kingdom::{Quirk, Quirks};

/// One reaper, a huge standing crop, and a sunny sky. The original stores three
/// halves of *everything the county grew*; fixed, it stores three halves of what
/// the one reaper could carry.
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
/// This is the consequence a player could notice.
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

/// A village big enough for the step to be non-zero.
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

