//! **The England turn-one fixture**, read against itself.
//!
//! ```text
//! LORDS2_FIXTURES="E:\dev\lords2-fixtures" cargo test -p l2-formats --test save_england_turn1
//! ```
//!
//! # What this file is for
//!
//! `tests/save.rs` asserts what is true of *any* save. This file asserts one
//! particular saved game: the England map at the start of turn one, Winter
//! 1268. Every test here is gated on `l2_testkit::england_turn1`, which finds
//! `%LORDS2_FIXTURES%\england-turn1.sav` and **checks it is that game** before
//! handing it over. Absent, the tests skip and say so; present and wrong, they
//! fail with a message naming the fixture rather than a bare assertion diff.
//!
//! # Why the file is called a fixture and not "the shipped save"
//!
//! A clean GOG install ships no saves at all — verified by diffing a pristine
//! copy of the install against a played one: six files differ and five of them
//! are saves. This save was produced by someone in an earlier session of this
//! project starting a campaign. The old name said "shipped", the old code read
//! it out of the game directory, and the game rewrites that file every turn
//! somebody plays. Ten minutes of play turned nine tests red, and the fix that
//! matters is not the path — it is that a fixture is now a name with a
//! fingerprint behind it.
//!
//! # Three assertions that were accidents
//!
//! Comparing two independently created England turn-one saves settled what is
//! scenario and what is per-game noise. Three of the nine tests below were
//! asserting noise, and each one is called out where it now stands:
//!
//! * the **realm→county assignment** is rolled per game; only the set of five
//!   counties is fixed;
//! * `g_weatherCounty` is a per-game roll;
//! * "county 1 is the odd one out on food" is really **"realm 5's starting
//!   county begins on Half rations"** — a genuine mechanic that had been pinned
//!   to a county index by coincidence.
//!
//! All three passed against the original file. They would have passed forever,
//! for the wrong reason, which is `docs/decisions.md` C12's shape.

use l2_formats::save::COUNTY_RECORDS;
use l2_testkit::{england, england_county_of_realm, ENGLAND_TURN1_COUNTIES};

/// The starting position: fourteen counties, five of them held, one realm each.
///
/// **Corrected.** This used to read
/// `assert_eq!(owners, vec![(1, 5), (4, 4), (8, 1), (11, 3), (13, 2)])`. The
/// county *set* is scenario; the realm *assignment* is not — a second England
/// turn-one save gives 1→4, 4→2, 8→5, 11→3, 13→1 for the same five counties.
/// County 11 matching in both is chance, and is exactly why one playthrough
/// reads so convincingly as a rule.
#[test]
fn the_england_map_starts_with_five_owned_counties_one_realm_each() {
    let save = england!();
    let counties = save.counties().unwrap();
    assert_eq!(counties.len(), COUNTY_RECORDS);

    let owned: Vec<usize> = counties.iter().filter(|c| c.is_owned()).map(|c| c.index).collect();
    assert_eq!(owned, ENGLAND_TURN1_COUNTIES, "the five starting counties");

    let mut realms: Vec<u8> = counties.iter().filter(|c| c.is_owned()).map(|c| c.owner).collect();
    realms.sort_unstable();
    assert_eq!(realms, [1, 2, 3, 4, 5], "one county each, in some order");

    let real: Vec<_> = counties.iter().filter(|c| c.is_county()).collect();
    assert_eq!(real.len(), 14, "fourteen counties on the England map");
    assert_eq!(real.iter().filter(|c| !c.is_owned()).count(), 9, "nine unowned");

    // Records 0, 15 and 16 are array slots, not places.
    for i in [0, 15, 16] {
        assert!(!counties[i].is_county(), "record {i} is not a county");
    }
}

/// The happiness split: owned counties store 72, unowned 77, and the difference
/// is the unowned bonus arriving through `shownEvents`.
#[test]
fn owned_counties_store_seventy_two_and_unowned_seventy_seven() {
    let save = england!();
    for c in save.counties().unwrap().iter().filter(|c| c.is_county()) {
        let expected = if c.is_owned() { 72 } else { 77 };
        assert_eq!(c.happiness, expected, "county {} happiness", c.index);
        assert_eq!(c.happiness_last, 65, "county {} last turn", c.index);
        assert_eq!((c.shown_tax, c.shown_health, c.shown_ration), (5, 1, 1));
        assert_eq!(
            c.shown_events,
            if c.is_owned() { 0 } else { 5 },
            "county {}: the unowned bonus arrives through shownEvents",
            c.index
        );
        assert_eq!(c.health_meter, 67);
        assert_eq!(c.health_band, 3);
    }
}

/// Population: every county grew from the same 417, and the birth rate differs
/// only because happiness does.
#[test]
fn population_grew_from_the_same_starting_number_everywhere() {
    let save = england!();
    for c in save.counties().unwrap().iter().filter(|c| c.is_county()) {
        assert_eq!(c.pop_last, 417, "county {}", c.index);
        assert_eq!(c.deaths, 45, "county {}", c.index);
        if c.is_owned() {
            assert_eq!(c.births, 63, "owned county {}", c.index);
            assert_eq!(c.population, 435, "417 + 63 - 45");
            assert_eq!(c.pop_band, 18);
        } else {
            assert_eq!(c.births, 84, "unowned county {}", c.index);
            assert_eq!(c.population, 456, "417 + 84 - 45");
            assert_eq!(c.pop_band, 19);
        }
    }
}

/// Turn one: nothing has been taxed, every rate is zero, and only owned
/// counties have a castle. These are the facts a new game starts from, and they
/// are what a scenario importer has to reproduce.
#[test]
fn turn_one_has_no_tax_and_castles_only_where_someone_lives() {
    let save = england!();
    for c in save.counties().unwrap().iter().filter(|c| c.is_county()) {
        assert_eq!(c.tax_rate, 0, "county {}", c.index);
        assert_eq!(c.tax_collected, 0, "county {}", c.index);
        assert_eq!(c.weather, 3, "cloudy everywhere");
        assert_eq!(c.fertility, 0, "basic farming leaves fertility at zero");
        assert_eq!(
            c.castle_type != 0,
            c.is_owned(),
            "county {} has a castle iff it is owned",
            c.index
        );
    }
}

/// The food split `docs/kingdom.md` §4.3 derives: an unowned county at
/// population 456 with a herd of 67 slaughters 13 head. Read here rather than
/// computed, which is what makes it evidence.
#[test]
fn the_documented_food_split_is_what_the_file_stores() {
    let save = england!();
    let unowned: Vec<_> =
        save.counties().unwrap().into_iter().filter(|c| c.is_county() && !c.is_owned()).collect();
    assert_eq!(unowned.len(), 9);
    for c in &unowned {
        assert_eq!(c.population, 456);
        assert_eq!(c.herd, 67, "county {}", c.index);
        assert_eq!(c.herd_eaten, 13, "county {}: DivCeil(456 - 67*5, 10)", c.index);
        assert_eq!(c.ration_wanted, 3, "normal rations");
    }
}

/// The realm records say the same thing the county owner bytes do, from a
/// different field: five realms in play, one county each. `+0x29` is the
/// realm's own count of what it owns.
///
/// **Corrected.** This used to assert `realms[1].lord == 0` and the AI lords as
/// `[1, 2, 4, 3]`. Which lord sits behind which realm is part of the same
/// per-game roll as the county assignment, so what is asserted now is the
/// structure: five distinct lords, the person is realm 1, and row 0 of every
/// lord-indexed table belongs to whoever the person is.
#[test]
fn five_realms_are_in_play_and_each_owns_exactly_one_county() {
    let save = england!();
    let realms = save.realms().unwrap();
    assert_eq!(realms.len(), 6, "six records, index 0 unused");
    assert!(!realms[0].in_play(), "realm 0 is an array slot");

    for r in realms.iter().skip(1) {
        assert!(r.in_play(), "realm {} is in play", r.index);
        assert_eq!(r.county_count, 1, "realm {} owns one county", r.index);
        assert_eq!(r.gold, 1000, "realm {} starts on a thousand crowns", r.index);
        assert_eq!((r.iron, r.stone), (50, 50), "realm {}", r.index);
        assert_eq!(r.ai_step, 0, "realm {} has not begun a turn", r.index);
        assert_eq!(r.rank, 0, "nothing has been ranked yet");
        assert_eq!(r.tax_hap_empire, 0, "every rate is zero");
        assert_eq!(r.wages, 0);
    }

    let human: Vec<usize> = realms.iter().filter(|r| r.is_human).map(|r| r.index).collect();
    assert_eq!(human, vec![1], "one human realm");
    assert_eq!(realms[1].lord, 0, "row 0 of every lord-indexed table is the person's");

    let mut lords: Vec<u8> = realms.iter().skip(1).map(|r| r.lord).collect();
    lords.sort_unstable();
    assert_eq!(lords, [0, 1, 2, 3, 4], "five distinct lords, one apiece");
}

/// The clock, the options and who is playing — the scalars outside the two
/// arrays.
///
/// **Corrected.** This used to assert `g_weatherCounty == 2` and that the
/// person holds county 8. Both are per-game rolls: the second England save
/// gives weather county 10 and puts the person on county 13. What survives is
/// that the weather county is a real county and that the person holds exactly
/// one of the five starting counties.
#[test]
fn the_globals_are_a_turn_one_winter_game_driven_by_realm_one() {
    let save = england!();
    let g = save.globals().unwrap();
    assert_eq!(g.county_count, 14);
    assert_eq!(g.scenario_index, 0, "the England map");
    assert_eq!(g.local_player, 1);
    assert_eq!((g.season, g.season_next, g.year, g.turn_count), (4, 1, 1268, 1));
    assert_eq!((g.turn_phase, g.turn_phase_step), (1, 0), "parked at the start of phase 1");
    assert_eq!((g.opt_difficulty, g.opt_advanced_farming, g.opt_armies_eat), (0, 0, 0));
    assert_eq!(g.merchant_count, 6);
    assert!(
        (1..=g.county_count).contains(&g.weather_county),
        "g_weatherCounty {} is a real county (which one is rolled per game)",
        g.weather_county
    );

    let mine = england_county_of_realm(&save, g.local_player as u8);
    assert!(
        ENGLAND_TURN1_COUNTIES.contains(&mine),
        "the person holds county {mine}, one of the five starting counties"
    );
    assert_eq!(save.counties().unwrap()[mine].owner as i32, g.local_player);
}

/// The map is one piece and its shape is the England map's: fourteen counties,
/// twenty-seven borders, and county 1 the dead end with a single neighbour.
///
/// Symmetry itself is an invariant and lives in `tests/save.rs`; what is here
/// is the *shape of this map*, which is what the old test was really asserting
/// when it said `assert_eq!(real.len(), 14)` under an invariant's name.
#[test]
fn the_england_map_has_fourteen_counties_and_county_one_is_the_dead_end() {
    let save = england!();
    let counties = save.counties().unwrap();
    let real: Vec<_> = counties.iter().filter(|c| c.is_county()).collect();
    assert_eq!(real.len(), 14);

    let directed: usize = real.iter().map(|c| c.neighbours().len()).sum();
    assert_eq!(directed % 2, 0);
    eprintln!("England: {} borders", directed / 2);

    // County 1 is the map's dead end - one neighbour - and it matters later:
    // it is the county whose ration term needs the season's own pre-season
    // store to reproduce.
    assert_eq!(counties[1].neighbours(), &[2]);
    assert_eq!(counties[1].neighbour_count, 1);
    assert_eq!(directed / 2, 27, "twenty-seven borders on the England map");
}

/// **The food configuration, and the mechanic the old test had by the tail.**
///
/// Three shapes on turn one:
///
/// * the nine unowned counties: 100 sacks, 67 head, split 100 (all livestock),
///   13 head slaughtered;
/// * four of the five owned counties: no grain, a herd big enough that five
///   people per head covers the population, nothing eaten;
/// * **one owned county on its own**: no grain, a split of 0 (all grain), and a
///   stored `rationAchieved` of 2 — the only county on the map not on Normal.
///
/// The old test asserted that the third county was **county 1**, with
/// `(owner, grain, herd, ration_split) == (5, 0, 74, 0)`. It is not county 1.
/// In a second England turn-one save the odd county is 8 — and county 1 in the
/// first save and county 8 in the second are both **realm 5's** starting
/// county. The short-of-food start follows the *realm*, not the index; the test
/// had pinned a real mechanic to a coincidence, and would have gone on passing
/// against the one file it was written from.
///
/// It is also the only county whose `dHapRation` (−2) disagrees with its
/// `shownRation` (+1), which is the fingerprint of the ration pass running
/// twice per season.
#[test]
fn realm_fives_starting_county_is_the_one_that_begins_on_half_rations() {
    let save = england!();
    let counties = save.counties().unwrap();

    for c in counties.iter().filter(|c| c.is_county() && !c.is_owned()) {
        assert_eq!((c.grain, c.herd, c.ration_split), (100, 67, 100), "county {}", c.index);
        assert_eq!((c.grain_eaten, c.herd_eaten), (0, 13), "county {}", c.index);
        assert_eq!(c.ration_achieved, 3, "county {}", c.index);
        assert_eq!(c.d_hap_ration, 1, "county {}", c.index);
    }

    // Found from the data, not written down: the county short of food is
    // whichever one realm 5 was given.
    let hungry = england_county_of_realm(&save, 5);
    let short: Vec<usize> = counties
        .iter()
        .filter(|c| c.is_county() && c.ration_achieved != 3)
        .map(|c| c.index)
        .collect();
    assert_eq!(short, vec![hungry], "exactly one county starts short, and it is realm 5's");

    for c in counties.iter().filter(|c| c.is_owned() && c.index != hungry) {
        assert_eq!(c.grain, 0, "owned county {} holds no grain", c.index);
        assert!(c.herd * 5 >= c.population, "owned county {} lives on cheese", c.index);
        assert_eq!((c.grain_eaten, c.herd_eaten), (0, 0), "county {}", c.index);
        assert_eq!(c.ration_achieved, 3, "county {}", c.index);
        assert_eq!(c.d_hap_ration, 1, "county {}", c.index);
    }

    let one = counties[hungry];
    assert_eq!(one.owner, 5, "by construction");
    assert_eq!((one.grain, one.herd, one.ration_split), (0, 74, 0));
    assert_eq!(one.ration_wanted, 3, "it asked for Normal");
    assert_eq!(one.ration_achieved, 2, "and the preview says it will get Half");
    assert_eq!(one.d_hap_ration, -2, "3L - 8 at L = 2");
    assert_eq!(one.shown_ration, 1, "but the happiness it stores was built on +1");
    assert_eq!(one.happiness, 72, "65 + 5 + 1 + 1");
}

/// `+0x180` and `+0x184` equal the stores themselves, in every county. The pass
/// that wrote them therefore spent nothing — which is the ration *preview*, and
/// is the file's own evidence for the two-call reading of `Ration_Apply`.
#[test]
fn the_recorded_available_food_is_the_food_still_in_store() {
    let save = england!();
    for c in save.counties().unwrap().iter().filter(|c| c.is_county()) {
        assert_eq!(c.grain_available, c.grain, "county {}", c.index);
        assert_eq!(c.herd_available, c.herd, "county {}", c.index);
    }
}

/// The fixture is what it says it is. Runs the fingerprint explicitly so a
/// green run has said, out loud, which game it asserted against.
#[test]
fn the_fixture_is_the_england_turn_one_position() {
    let save = england!();
    l2_testkit::england_turn1_fingerprint(&save).expect("the gate already checked this");
    let g = save.globals().unwrap();
    eprintln!(
        "england-turn1: scenario {}, {} counties, turn {}, season {}, year {}, realm {} is the person",
        g.scenario_index, g.county_count, g.turn_count, g.season, g.year, g.local_player
    );
    for r in 1..=5u8 {
        eprintln!("  realm {r} holds county {}", england_county_of_realm(&save, r));
    }
}

/// **The falsifiable prediction in `docs/formats/plane4.md` §2.2, checked.**
///
/// That document simulated `Merchant_PickStartCounties` over `L2_maps.dat` and
/// predicted that England's six merchants start in counties **14, 5, 13, 11, 12
/// and 4** — marked `[I]`, "the specific county list, which was not recorded in
/// that run". It is in the save, and it is those six, in that order.
///
/// The routes come with it: row 1 is `14 → 4 → 7 → 8 → 2`, which is the scan
/// order the same document derives from castle geography rather than anything an
/// author wrote. Two files produced by different code — `L2_maps.dat` shipped
/// with the game, `england-turn1.sav` written by the running engine — agreeing
/// on 41 numbers.
#[test]
fn england_ships_six_merchants_on_the_six_routes_plane4_predicted() {
    let save = england!();
    assert_eq!(save.merchant_start_counties().unwrap(), [14, 5, 13, 11, 12, 4]);
    assert_eq!(save.globals().unwrap().merchant_count, 6);

    let rows = save.merchant_routes().unwrap();
    let live = |r: usize| -> Vec<u8> { rows[r].iter().copied().take_while(|&c| c != 0).collect() };
    assert_eq!(live(0), vec![14, 4, 7, 8, 2]);
    assert_eq!(live(1), vec![5, 12, 6, 3, 8, 2, 1]);
    assert_eq!(live(2), vec![14, 11, 13, 5, 10, 9, 7, 3, 1]);
    assert_eq!(live(3), vec![11, 12, 6, 10, 4, 9, 3, 1]);
    assert_eq!(live(4), vec![14, 11, 5, 13, 12, 10, 2]);
    assert_eq!(live(5), vec![13, 6, 4, 9, 7, 8]);

    // §1.1's invariant, on this map: every county is on at least one route.
    let mut seen = [false; 15];
    for r in 0..6 {
        for c in live(r) {
            seen[c as usize] = true;
        }
    }
    assert!(seen[1..=14].iter().all(|&s| s), "a county on no route");
}

/// The six units the England position holds are **six merchants and nothing
/// else** — no armies, no mobs, no transports — each owned by nobody, standing
/// in its own start county, with its route number and a route cursor of 1.
///
/// **The cursor is 1, not 0**, which is why a merchant's first destination is
/// the *second* county on its list. And every one of them carries
/// `moveAllowance = 0`: the field is written by `Merchant_Tick` and turn one has
/// not ticked them yet, so an importer that filled in 10 there would be
/// inventing a number the game had not reached.
#[test]
fn the_six_units_are_merchants_waiting_in_their_start_counties() {
    let save = england!();
    let start = save.merchant_start_counties().unwrap();
    let units: Vec<_> = save.units().unwrap().into_iter().filter(|u| u.is_live()).collect();
    assert_eq!(units.len(), 6, "the England position holds six units");

    for (n, u) in units.iter().enumerate() {
        assert_eq!(u.index, n + 1, "merchants take slots 1..6");
        assert_eq!(u.kind, 3, "unit {} is not a merchant", u.index);
        assert_eq!(u.owner, 6, "a merchant belongs to nobody");
        assert_eq!(u.county, start[n], "merchant {} is not in its start county", u.index);
        assert_eq!(u.role, start[n], "+0x167 is the start county");
        assert_eq!(u.name_index as usize, n, "the route number is the slot, zero-based");
        assert_eq!(u.route_cursor(), 1, "the cursor starts at 1");
        assert_eq!(u.morale, 100, "Merchant_SpawnAll writes 100 to +0x166");
        assert_eq!(u.move_allowance, 0, "tick-maintained, and turn one has not ticked");
        assert_eq!(u.men, 0, "a merchant is not troops");
        assert!(u.needs_destination, "and none of them has been given anywhere to go");
        assert_eq!(u.path_len, 0);
    }
}
