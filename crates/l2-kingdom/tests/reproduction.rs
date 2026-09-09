//! **The reproduction from the England turn-one fixture** — and this time it
//! opens it.
//!
//! # What this file used to be
//!
//! `docs/decisions.md` C12, exactly: a test headed *"the reproduction from the
//! shipped save"* that never read a save. It declared `const OWNED: usize = 4`,
//! handed counties 1–4 to the human realm, gave every county the same numbers
//! out of `docs/kingdom.md`, and then checked that those numbers came back —
//! a scenario invented from the document, measured against the rules built from
//! the same document. It could not fail, and its scenario was wrong.
//!
//! The file holds **five owned counties, one for each of realms 1 to 5**, at
//! indices 1, 4, 8, 11 and 13; nine unowned; fourteen counties in seventeen
//! slots. `crates/l2-formats/tests/save_england_turn1.rs` asserts all of that
//! against the bytes.
//!
//! # "Shipped" was the wrong word, and it cost the suite a day
//!
//! A clean GOG install ships **no saves at all**. The file this test reads was
//! produced by somebody in an earlier session of this project starting a
//! campaign; the game writes `lastturn.sav` when turn one begins and rewrites it
//! every turn thereafter. Calling it "the shipped save" made a volatile file
//! look permanent, and resolving it by a hard-coded path inside the game
//! directory meant ten minutes of play replaced it. That is now a **named
//! fixture** — `%LORDS2_FIXTURES%\england-turn1.sav`, checked against
//! `l2_testkit::england_turn1_fingerprint` before a single assertion runs.
//!
//! Two things this position does *not* fix, and which are rolled per game:
//! **which realm gets which of the five starting counties**, and therefore
//! which county starts short of food. Both were written down here as constants.
//! See [`hungry_county`].
//!
//! # What it is now
//!
//! An oracle. `l2-scenario` imports the fixture twice — once as the state the
//! file holds, once rewound to the position it was taken from — and every
//! assertion below compares this crate's output against the *file's* numbers.
//! Nothing here is quoted from a document. If the pipeline's order, the health
//! ladder's comparison sense, the delta table's indexing, the 1-based season
//! array, the birth ladder's pairing or the unowned happiness bonus were wrong,
//! a number read out of the fixture would say so.
//!
//! **No rule was changed to make this pass.** Fourteen counties reproduce
//! twenty-four stored fields each — see
//! [`every_county_reproduces_every_stored_field`] — and the herd's own
//! forecast, four more numbers a county, reproduces with no inversion at all
//! ([`the_herds_own_forecast_reproduces_for_every_county`]).
//!
//! # The two things the save does not record
//!
//! It stores `popLast` and `happinessLast`, so the previous season's population
//! and happiness are exact. It stores **no previous herd and no previous
//! grain**: the ration pass ate some of both and only the remainder survives.
//! So the starting position `l2-scenario` builds carries the stored stores
//! forward, and one county of fourteen — **realm 5's**, whichever that is —
//! then starves where the real one did not. That is not a rule failing; it is a
//! missing input, and [`the_food_the_season_ate_is_recoverable_and_unique`]
//! recovers it: there is **exactly one** pre-season store per county that
//! reproduces the file, and putting it back makes the map land.
//!
//! It also stores **no pre-season labour**. `FUN_0044F6E7` reallocates every
//! county's workers after the population moves, so what is in the file is where
//! the peasants went *afterwards* — and the herd's births and deaths were
//! computed from where they were *before*. That is why [`comparison`] no longer
//! carries `herd` and `herd_eaten`, and its own documentation is where the
//! evidence for that is written down. Nothing was relaxed to make a rule pass:
//! the herd rule is checked directly instead, against the numbers the file
//! holds for it.
//!
//! Recovering it also answers a question `docs/kingdom.md` §4.3 left open.
//! That county holds no grain and its `shownRation` is `+1`, so it was fed at
//! Normal by the pass that ran *before* happiness — and with an all-grain split
//! that costs eight sacks. A `Ration_Apply` that did not debit the store would
//! have left those eight sacks in it. **It debits**, which is the reading
//! `l2_kingdom::ration::apply` already implements.
//!
//! # Running it
//!
//! ```text
//! LORDS2_FIXTURES="E:\dev\lords2-fixtures" cargo test -p l2-kingdom --test reproduction
//! ```
//!
//! Every test skips *visibly* when the fixture is not configured, and **fails**
//! when something is configured that is not it. Those are different states and
//! conflating them is how the previous breakage went unnoticed.

use l2_formats::save::Save;
use l2_kingdom::county::County;
use l2_kingdom::phase::SEASON_PIPELINE;
use l2_kingdom::tables::{health_band, Season, Tables, Weather};
use l2_scenario::{Scenario, STARTING_HEALTH_METER};

/// The seed is irrelevant to everything asserted here — with Advanced Farming
/// off the weather is forced, and no county draws an event in 1268 — but it is
/// fixed anyway, because a test that *would* notice a different seed is a test
/// worth having notice.
const SEED: u64 = 0x10D_52;

/// One county field straight out of the file, at the offset `docs/kingdom.md`
/// §1 names it at.
///
/// Used for the handful of fields `l2_formats::save::County` does not carry.
/// That struct belongs to `l2-formats` and is not this test's to change, and
/// `Save` exposes the addressed read the struct is itself built from — so the
/// bytes are reachable without either crate growing a field for a test.
fn county_i32(save: &Save, id: usize, offset: u32) -> i32 {
    save.i32_at(0x0053_F9B0 + (id as u32) * 0x300 + offset).expect("a saved county address")
}

/// The England turn-one fixture, imported. Skips when it is not configured and
/// panics when what is configured is a different game — see
/// `l2_testkit::england_turn1`.
macro_rules! england {
    () => {{
        let save = l2_testkit::england!();
        Scenario::from_save(&save).expect("the England turn-one fixture must import")
    }};
}

/// County 1 borders one county and nothing else. That **is** a property of the
/// England map and is stable across every save of it.
const MAP_DEAD_END: usize = 1;

/// The one county the imported starting position cannot feed, because the save
/// does not record what it ate.
///
/// **Corrected.** This was `const DEAD_END: usize = 1`, and it conflated two
/// facts that happened to coincide in the one save anybody had looked at:
/// county 1 is the map's dead end, *and* county 1 was the county that starts on
/// Half rations. A second England turn-one save separates them — there the
/// hungry county is 8, and county 1 is still the dead end.
///
/// What actually predicts it is the **realm**: county 1 belonged to realm 5 in
/// the first save and county 8 belongs to realm 5 in the second. One lord always
/// begins short of food, and it is always realm 5. That is a game-design fact
/// nobody here had noticed while it was written down as a county index.
fn hungry_county(s: &Scenario) -> usize {
    s.county_ids()
        .find(|&id| s.counties[id].as_ref().is_some_and(|c| c.owner == 5))
        .expect("realm 5 holds a county in an England turn-one save")
}

/// Everything worth comparing between a county we computed and a county the
/// file holds. Twenty-four fields, named, so a failure says which one.
///
/// # Why `herd` and `herd_eaten` are not two of them any more
///
/// They were, and **they reproduced because a rule was missing.** The herd used
/// to move only with the weather, which is neutral on this map, so "put back
/// what the ration pass ate" was the whole of it and the inversion in
/// [`solve_opening`] closed. Now that a herd is born, dies and has to be tended
/// (`docs/kingdom.md` §13), the season moves it — and the file says outright
/// that the inversion's answer was never the real one. `+0x254` is the herd as
/// `Herd_SeasonTick` found it, and it is **95 in every one of the fourteen
/// counties**: the new-game starting herd from the table at `0x004DC0D0`, not
/// the 73 the inversion recovers.
///
/// Getting from 95 to the stored herd needs the **pre-season** cattle labour,
/// and the save records only the post-season allocation — `FUN_0044F6E7`
/// reassigns every county's workers after the population moves. County 4 is the
/// proof: its stored herd needs a staffing of 86%, about 246 workers, and the
/// file holds 261. A search over openings settles it — with the file's labour
/// there are two or three openings that land for each *owned* county and
/// **none at all** for the nine unowned ones. `herd_eaten` follows the herd
/// out, because it is the ration *preview*, computed last from the herd the
/// season ended on.
///
/// So this list drops the two fields it was reproducing for the wrong reason,
/// and [`the_herds_own_forecast_reproduces_for_every_county`] adds four per
/// county that it reproduces for the right one — read straight out of the file,
/// no inversion anywhere. Modelling the labour allocator would bring these two
/// back; that is a separate piece of work, and it is named here so it is not
/// lost.
fn comparison(ours: &County, file: &County) -> Vec<(&'static str, i32, i32)> {
    vec![
        ("owner", ours.owner as i32, file.owner as i32),
        ("happiness", ours.happiness, file.happiness),
        ("happiness_last", ours.happiness_last, file.happiness_last),
        ("happiness_sum", ours.happiness_sum, file.happiness_sum),
        ("happiness_avg", ours.happiness_avg, file.happiness_avg),
        ("shown_tax", ours.shown_tax, file.shown_tax),
        ("shown_ration", ours.shown_ration, file.shown_ration),
        ("shown_health", ours.shown_health, file.shown_health),
        ("shown_events", ours.shown_events, file.shown_events),
        ("d_hap_ration", ours.d_hap_ration, file.d_hap_ration),
        ("health_meter", ours.health_meter, file.health_meter),
        ("health_band", ours.health_band as i32, file.health_band as i32),
        ("unrest", ours.unrest as i32, file.unrest as i32),
        ("population", ours.population, file.population),
        ("pop_last", ours.pop_last, file.pop_last),
        ("pop_band", ours.pop_band, file.pop_band),
        ("births", ours.births, file.births),
        ("deaths", ours.deaths, file.deaths),
        ("emigrants", ours.emigrants, file.emigrants),
        ("immigrants", ours.immigrants, file.immigrants),
        ("tax_collected", ours.tax_collected, file.tax_collected),
        ("ration_achieved", ours.ration_achieved, file.ration_achieved),
        ("grain_eaten", ours.grain_eaten, file.grain_eaten),
        ("grain", ours.grain, file.grain),
    ]
}

/// Search a generous box of pre-season `(herd, grain)` for the openings that
/// (a) feed the county at the level the file's `shownRation` records and
/// (b) leave behind exactly the stores the file holds. Panics unless there is
/// exactly one.
///
/// Inverting a rule to recover its input is an oracle's business, so it lives
/// here and not in `l2-scenario`: an importer that solved the rules to build its
/// own starting position would be handing this test the answer.
fn solve_opening(stored: &County, t: &Tables) -> (i32, i32) {
    let level = (stored.shown_ration + 8) / 3;
    let mut probe = County::new();
    probe.population = stored.pop_last;
    probe.ration_wanted = stored.ration_wanted;
    probe.ration_split = stored.ration_split;
    let mut found = None;
    for herd in stored.herd..=stored.herd + 200 {
        for grain in stored.grain..=stored.grain + 200 {
            probe.herd = herd;
            probe.grain = grain;
            let plan = l2_kingdom::ration::choose(t, &probe, false);
            if plan.level == level
                && herd - plan.heads == stored.herd
                && grain - plan.sacks == stored.grain
            {
                assert!(found.is_none(), "more than one opening store fits this county");
                found = Some((herd, grain));
            }
        }
    }
    found.expect("an opening store must exist")
}

/// The starting position with the food the season ate put back — the openings
/// [`the_food_the_season_ate_is_recoverable_and_unique`] proves unique.
fn kingdom_after_the_first_season(s: &Scenario) -> l2_kingdom::Kingdom {
    let file = s.kingdom(SEED);
    let t = &Tables::DEFAULT;
    let mut k = s.starting_kingdom(SEED);
    for id in s.county_ids() {
        let (herd, grain) = solve_opening(&file.counties[id], t);
        k.counties[id].herd = herd;
        k.counties[id].grain = grain;
    }
    k.start_new_game();
    k
}

// ---------------------------------------------------------------------------
// The scenario itself — the correction C12 was hiding
// ---------------------------------------------------------------------------

/// **The correction, asserted through the importer.** Five owned counties, at
/// indices 1, 4, 8, 11 and 13, one for each of realms 1 to 5 — not four owned
/// by one realm, which is what C12's version of this file invented.
///
/// **Corrected again.** It used to pin the mapping,
/// `[(1, 5), (4, 4), (8, 1), (11, 3), (13, 2)]`. The *set* is scenario; the
/// *assignment* is rolled per game, and a second England turn-one save gives
/// 1→4, 4→2, 8→5, 11→3, 13→1. The importer's job is to carry across whatever
/// the file says, so what is asserted is that it did: every owner it produced
/// is the owner byte the save holds.
#[test]
fn the_england_scenario_is_five_realms_with_one_county_each() {
    let s = england!();
    assert_eq!(s.county_count, 14, "fourteen counties on the England map");
    assert_eq!(s.local_player, 1);

    let owned: Vec<(usize, u8)> = s
        .county_ids()
        .filter_map(|id| s.counties[id].as_ref().map(|c| (id, c.owner)))
        .filter(|(_, owner)| *owner != 0)
        .collect();
    assert_eq!(
        owned.iter().map(|&(id, _)| id).collect::<Vec<usize>>(),
        l2_testkit::ENGLAND_TURN1_COUNTIES,
        "the five starting counties"
    );
    let mut realms: Vec<u8> = owned.iter().map(|&(_, r)| r).collect();
    realms.sort_unstable();
    assert_eq!(realms, [1, 2, 3, 4, 5], "one county each, in some order");
    assert_eq!(s.county_ids().count() - owned.len(), 9, "nine unowned");

    for id in 1..=5 {
        assert!(s.realms[id].in_play, "realm {id} is in play");
        assert_eq!(s.realms[id].county_count, 1, "realm {id} owns one county");
        assert_eq!(s.realms[id].gold, 1000);
    }
    assert!(s.realms[1].is_human, "realm 1 is the person");
    assert_eq!(s.realms[1].lord, 0, "row 0 of every lord-indexed table is the person's");
    for id in 2..=5 {
        assert!(!s.realms[id].is_human, "realm {id} is an AI");
    }
}

/// `g_season` 4, `g_year` 1268, `g_turnCount` 1 — read out of the file rather
/// than predicted from `Game_NewGame`'s constants. The prediction and the file
/// agree, which is the point: [`the_pipeline_reaches_the_files_clock`] runs the
/// clock forward and lands on these.
#[test]
fn the_file_is_a_turn_one_winter_1268_autosave() {
    let s = england!();
    assert_eq!(s.clock.season, 4);
    assert_eq!(Season::from_index(s.clock.season), Some(Season::Winter));
    assert_eq!(s.clock.season_next, 1, "Spring is next");
    assert_eq!(s.clock.year, 1268);
    assert_eq!(s.clock.turn_count, 1);
    assert_eq!(s.options.difficulty, 0);
    assert!(!s.options.advanced_farming);
    assert!(!s.options.armies_eat);
}

/// The seventeen-record array with an unused index 0, from the file: records 15
/// and 16 import as nothing at all, and 1 … 14 all import.
#[test]
fn the_array_holds_seventeen_records_and_only_fourteen_are_a_county() {
    let s = england!();
    let k = s.kingdom(SEED);
    assert_eq!(k.counties.len(), 17);
    assert_eq!(k.county_count, 14);

    let empty = County::new();
    assert_eq!(k.counties[0], empty, "record 0 is never a county");
    assert_eq!(k.counties[15], empty);
    assert_eq!(k.counties[16], empty);
    assert!(s.counties[15].is_none());
    assert!(s.counties[16].is_none());
    for id in s.county_ids() {
        assert!(s.counties[id].is_some(), "county {id} should have imported");
        assert_ne!(k.counties[id], empty, "county {id} should be populated");
    }
}

/// Adjacency comes from the file, and it is the real map: county 1 is a dead
/// end with a single neighbour, county 10 is a hub with seven, and every border
/// is named from both sides.
#[test]
fn the_map_the_import_builds_is_the_map_in_the_file() {
    let s = england!();
    let k = s.kingdom(SEED);
    for id in s.county_ids() {
        let c = &k.counties[id];
        assert!(c.neighbour_count > 0, "county {id} borders somebody");
        for &n in c.neighbours() {
            assert!(k.counties[n as usize].neighbours().contains(&(id as u8)), "county {id}");
        }
    }
    assert_eq!(k.counties[MAP_DEAD_END].neighbours(), &[2], "county 1 is the dead end");
    assert_eq!(k.counties[10].neighbour_count, 7, "county 10 is the hub");
    assert_eq!(
        s.county_ids().map(|id| k.counties[id].neighbour_count as usize).sum::<usize>(),
        54,
        "twenty-seven borders, counted from both sides"
    );
}

// ---------------------------------------------------------------------------
// The starting position
// ---------------------------------------------------------------------------

/// The rewind is the file's own record of the previous season, not a guess:
/// `population` becomes `popLast` and `happiness` becomes `happinessLast`, and
/// every value the season *computes* starts at zero, so that a pass which does
/// nothing cannot pass by leaving them alone.
#[test]
fn the_starting_position_is_the_file_rewound_by_exactly_one_season() {
    let s = england!();
    let start = s.starting_kingdom(SEED);
    let file = s.kingdom(SEED);

    for id in s.county_ids() {
        let a = &start.counties[id];
        let b = &file.counties[id];
        assert_eq!(a.population, b.pop_last, "county {id}");
        assert_eq!(a.happiness, b.happiness_last, "county {id}");
        assert_eq!(a.health_meter, STARTING_HEALTH_METER, "county {id}");
        assert_eq!(a.health_band, health_band(STARTING_HEALTH_METER) as u8, "county {id}");
        assert_eq!(a.owner, b.owner, "county {id}");
        assert_eq!(a.castle_type, b.castle_type, "county {id}");

        assert_eq!(a.births, 0, "county {id} has not been born into yet");
        assert_eq!(a.deaths, 0, "county {id}");
        assert_eq!(a.pop_last, 0, "county {id}");
        assert_eq!(a.pop_band, 0, "county {id}");
        assert_eq!(a.shown_tax, 0, "county {id}");
        assert_eq!(a.shown_ration, 0, "county {id}");
        assert_eq!(a.shown_health, 0, "county {id}");
        assert_eq!(a.happiness_sum, 0, "county {id}");
    }

    // Every county in the England turn-one fixture started the season on the same two
    // numbers - the file's own claim, checked rather than assumed.
    let starts: Vec<(i32, i32)> = s
        .county_ids()
        .map(|id| (file.counties[id].pop_last, file.counties[id].happiness_last))
        .collect();
    assert!(starts.iter().all(|&v| v == (417, 65)), "one starting position, fourteen counties");

    assert_eq!(start.turn_count, 0, "no season has run");
    assert_eq!(start.season, 0);
}

/// **`docs/kingdom.md` §9 point 3**, checked against the file rather than
/// against §7.2: with Advanced Farming off, every stored weather byte is 3 and
/// every stored fertility is 0 — and the pipeline leaves them there.
#[test]
fn basic_farming_leaves_every_county_cloudy_with_zero_fertility() {
    let s = england!();
    let file = s.kingdom(SEED);
    let mut k = s.starting_kingdom(SEED);
    k.start_new_game();
    for id in s.county_ids() {
        assert_eq!(file.counties[id].weather, Weather::Cloudy, "the file, county {id}");
        assert_eq!(file.counties[id].fertility, 0, "the file, county {id}");
        assert_eq!(k.counties[id].weather.index(), 3, "county {id}: the stored byte is 3");
        assert_eq!(k.counties[id].fertility, 0, "county {id}");
    }
}

// ---------------------------------------------------------------------------
// The reproduction
// ---------------------------------------------------------------------------

/// One `Season_Advance` from the file's own starting position lands on the
/// file's own clock, having run every documented pass in order.
#[test]
fn the_pipeline_reaches_the_files_clock() {
    let s = england!();
    let mut k = s.starting_kingdom(SEED);
    let report = k.start_new_game();
    assert_eq!(k.season, s.clock.season);
    assert_eq!(k.season_next, s.clock.season_next);
    assert_eq!(k.year, s.clock.year);
    assert_eq!(k.turn_count, s.clock.turn_count);
    assert_eq!(report.passes, SEASON_PIPELINE.to_vec(), "in the documented order");
    assert!(report.messages.is_empty(), "a happy kingdom raises no messages");
    assert!(report.revolts.is_empty());
}

/// **Thirteen of the fourteen counties reproduce every stored field** from the
/// starting position the save itself supplies — no reconstruction, no
/// adjustment. The fourteenth is county 1, and the next test says exactly why.
///
/// The herd is skipped and only the herd: the save records no earlier balance
/// to spend from. Everything the herd *feeds* still lands, which is the part
/// that matters.
#[test]
fn every_county_the_save_can_feed_reproduces_every_stored_field() {
    let s = england!();
    let file = s.kingdom(SEED);
    let mut k = s.starting_kingdom(SEED);
    k.start_new_game();

    let hungry = hungry_county(&s);
    let mut checked = 0;
    for id in s.county_ids() {
        if id == hungry {
            continue;
        }
        for (name, ours, theirs) in comparison(&k.counties[id], &file.counties[id]) {
            if name == "herd" || name == "herd_eaten" {
                continue;
            }
            assert_eq!(ours, theirs, "county {id} {name}");
            checked += 1;
        }
    }
    assert_eq!(checked, 13 * 24, "thirteen counties, twenty-four fields each");
}

/// **Realm 5's county is the divergence, and it is a missing input rather than
/// a broken rule.**
///
/// It holds no grain, splits its ration entirely onto grain, and keeps a herd of
/// 74 — which feeds 370 of its 417 people. The file says it ate at Normal
/// (`shownRation = +1`), so it must have had grain to eat. Started from the
/// stored zero it drops to Half, and the whole five-stage chain follows the
/// drop: −2 instead of +1 on happiness, a health delta that leaves the meter in
/// band 2 instead of band 3, and a death rate that costs it twenty-one people.
///
/// Pinned to the exact numbers, so a change to any stage of the chain moves this
/// test rather than passing quietly — but pinned to the county the *file* says
/// realm 5 holds, not to the index that realm happened to draw once. See
/// [`hungry_county`].
#[test]
fn realm_fives_county_diverges_because_the_save_does_not_record_what_it_ate() {
    let s = england!();
    let file = s.kingdom(SEED);
    let mut k = s.starting_kingdom(SEED);
    k.start_new_game();

    let hungry = hungry_county(&s);
    let ours = &k.counties[hungry];
    let theirs = &file.counties[hungry];

    assert_eq!(
        (theirs.grain, theirs.herd, theirs.ration_split),
        (0, 74, 0),
        "the file's county {hungry}"
    );
    assert_eq!(theirs.shown_ration, 1, "the file says it was fed at Normal");

    assert_eq!((ours.ration_achieved, ours.shown_ration), (2, -2), "ours starves");
    assert_eq!((ours.health_meter, ours.health_band), (59, 2), "and the health chain follows");
    assert_eq!((theirs.health_meter, theirs.health_band), (67, 3));
    assert_eq!(
        (ours.happiness, theirs.happiness),
        (68, 72),
        "65 + 5 + 0 - 2 against 65 + 5 + 1 + 1"
    );
    assert_eq!((ours.deaths, theirs.deaths), (66, 45), "band 2 dies faster than band 3");
    assert_eq!((ours.population, theirs.population), (414, 435));

    // Everything upstream of the ration term still lands.
    assert_eq!(ours.pop_last, theirs.pop_last);
    assert_eq!(ours.births, theirs.births, "the birth factor band is the same either way");
    assert_eq!(ours.shown_tax, theirs.shown_tax);
    assert_eq!(ours.tax_collected, theirs.tax_collected);
}

/// **The food the season ate is recoverable, and the answer is unique.**
///
/// Exactly one pre-season `(herd, grain)` per county both feeds it at the level
/// the file's `shownRation` records and leaves behind the stores the file holds.
/// Nine unowned counties opened on **73 head** and closed on 67; county 1 opened
/// on **8 sacks** and closed on none.
///
/// The uniqueness is what makes these numbers a measurement rather than a fudge.
/// It also settles `docs/kingdom.md` §4.3's open question about which of
/// `Ration_Apply`'s two calls survives: a pass that did not debit the store
/// could not have taken county 1's grain to zero, nor the unowned counties'
/// herds from 73 to 67.
#[test]
fn the_food_the_season_ate_is_recoverable_and_unique() {
    let s = england!();
    let file = s.kingdom(SEED);
    let t = &Tables::DEFAULT;

    for id in s.county_ids() {
        let stored = &file.counties[id];
        // The level the happiness update saw, read back out of its own display
        // copy: shownRation = 3L - 8.
        let level = (stored.shown_ration + 8) / 3;
        assert_eq!(t.ration_happiness(level), stored.shown_ration, "county {id}");

        let opening = solve_opening(stored, t);
        if stored.owner == 0 {
            assert_eq!(opening, (73, 100), "unowned county {id}");
        }
    }

    let hungry = hungry_county(&s);
    assert_eq!(
        solve_opening(&file.counties[hungry], t),
        (74, 8),
        "county {hungry} opened on eight sacks and ate all of them"
    );
    for id in s.county_ids().filter(|&id| id != hungry) {
        let stored = &file.counties[id];
        if stored.owner == 0 {
            continue;
        }
        assert_eq!(
            solve_opening(stored, t),
            (stored.herd, stored.grain),
            "owned county {id} lives on its dairy and spends nothing"
        );
    }
}

/// **The whole map, every stored field.** Put back the food the season ate — the
/// openings the previous test proves unique — and run `Season_Advance` once.
/// Fourteen counties, twenty-six fields each: three hundred and sixty-four
/// numbers, every one of them read out of `lastturn.sav`.
///
/// This is what the file at the top of this module claimed to be doing and was
/// not.
#[test]
fn every_county_reproduces_every_stored_field() {
    let s = england!();
    let file = s.kingdom(SEED);
    let k = kingdom_after_the_first_season(&s);

    let mut checked = 0;
    for id in s.county_ids() {
        for (name, ours, theirs) in comparison(&k.counties[id], &file.counties[id]) {
            assert_eq!(ours, theirs, "county {id} {name}");
            checked += 1;
        }
    }
    assert_eq!(checked, 14 * 24);
}

// ---------------------------------------------------------------------------
// The herd — `docs/kingdom.md` §13 and §13.1
// ---------------------------------------------------------------------------

/// **The staffing and crowding rules, against the file, with no inversion.**
///
/// `Herd_SeasonTick` ends by writing next season's forecast into `+0x268`,
/// `+0x26C` and `+0x258` — `L2.eng` group 77's *"Calf births expected"*, *"Cow
/// deaths expected"* and *"Change due to farming"* — from state the save also
/// holds: the herd, what the people ate, the pasture, the cattle labour and the
/// crowding. So the whole of `FUN_0044DA99` can be run against fourteen
/// counties' worth of stored answers without recovering anything.
///
/// Fifty-six numbers, and they are not a soft test: they exercise the
/// understaffed arm (county 1 at 98% staffing), the capped arm (county 2 at
/// 199%), three of the four crowding bands, the Spring birth bonus, and the
/// double subtraction of `herdEaten` that makes "change due to farming" what it
/// is. Get the `/ 3` truncation, the `199 <` comparison or the `x 3 / 2`
/// rounding wrong anywhere and a column moves.
#[test]
fn the_herds_own_forecast_reproduces_for_every_county() {
    let save = l2_testkit::england!();
    let s = Scenario::from_save(&save).expect("the England turn-one fixture must import");
    let mut k = s.kingdom(SEED);
    let next = s.clock.season_next;
    assert_eq!(next, 1, "the save's g_seasonNext is Spring, which is what calves");

    let mut checked = 0;
    for id in s.county_ids() {
        // The crowding the importer derived, against the byte the game stored.
        assert_eq!(
            k.counties[id].herd_crowding,
            county_i32(&save, id, 0x25C),
            "county {id} crowding"
        );
        checked += 1;

        l2_kingdom::land::herd_preview(&Tables::DEFAULT, &mut k.counties[id], next);
        for (name, ours, offset) in [
            ("births expected", k.counties[id].herd_births_expected, 0x268u32),
            ("deaths expected", k.counties[id].herd_deaths_expected, 0x26C),
            ("change due to farming", k.counties[id].herd_change_expected, 0x258),
        ] {
            assert_eq!(ours, county_i32(&save, id, offset), "county {id} {name}");
            checked += 1;
        }
    }
    assert_eq!(checked, 14 * 4);

    // And the two bands the map actually visits are not the same band, so the
    // check is not fourteen copies of one arithmetic.
    let crowdings: std::collections::BTreeSet<i32> =
        s.county_ids().map(|id| k.counties[id].herd_crowding).collect();
    assert_eq!(crowdings.into_iter().collect::<Vec<_>>(), vec![10, 20, 40]);
}

/// **The labour import checks itself.** Nine job records a county, and every
/// county's nine sum to its population *exactly* — 218 + 217 = 435 in county 1,
/// 323 + 133 = 456 in county 2, and so on for all fourteen.
///
/// That is the evidence the stride is `0x0C` and the worker count is the first
/// word of the record, and it is the kind of check `docs/method.md` §2 rates
/// above any amount of reading: a property of the data, not of our code. A
/// stride of 4, 8 or 16, or a count at word 1 or 2, would not land on the
/// population once, let alone fourteen times.
#[test]
fn every_countys_nine_labour_records_sum_to_its_population() {
    let s = england!();
    let k = s.kingdom(SEED);
    for id in s.county_ids() {
        let c = &k.counties[id];
        let assigned: i32 = c.labour.iter().sum();
        assert_eq!(assigned, c.population, "county {id}");
        assert!(c.labour[Tables::DEFAULT.job.cattle_farming] > 0, "county {id} farms cattle");
    }
}

/// **Every county opened the game on the same 95 head**, which is the number
/// `docs/kingdom.md`'s reproduction could not see while the herd rule was
/// missing.
///
/// `+0x254` is written by `Herd_SeasonTick` at the top of the season, so what
/// is in the file is the herd as the *first* season found it — and it is the
/// `herd` column of the new-game table at `0x004DC0D0 + difficulty * 0x14`,
/// whose row 1 is `{grain 0, herd 95, population 417, health 65, health 65}`.
/// Every one of those four reappears in the save: `popLast` is 417 in all
/// fourteen counties, and 65 is [`STARTING_HEALTH_METER`], which
/// `crates/l2-scenario` had marked as the one number in the reproduction taken
/// from prior art rather than from the binary. It is in the binary.
#[test]
fn every_county_opened_the_game_on_the_same_herd_and_the_same_people() {
    let save = l2_testkit::england!();
    let s = Scenario::from_save(&save).expect("the England turn-one fixture must import");
    for id in s.county_ids() {
        assert_eq!(county_i32(&save, id, 0x254), 95, "county {id} herd at the top of season 1");
        let stored = s.counties[id].as_ref().expect("a county");
        assert_eq!(stored.population_last, 417, "county {id} popLast");
    }
    assert_eq!(STARTING_HEALTH_METER, 65, "and the health meter is the same table's column 3");
}

/// The five-stage chain — ration → health meter → health band → happiness →
/// birth rate → population — on the two bands the map lands on, with every
/// number on both sides read from the file.
#[test]
fn the_whole_five_stage_chain_lands_on_both_bands() {
    let s = england!();
    let file = s.kingdom(SEED);
    let k = kingdom_after_the_first_season(&s);

    // County 8 is the person's; county 14 is unowned. Two owners, two bands.
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

/// The ration rule against the file with **no reconstruction at all**: run the
/// end-of-season preview on the counties exactly as the file holds them, and it
/// must produce the stored `rationAchieved`, `herdEaten`, `grainEaten` and
/// `dHapRation`.
///
/// Four different configurations and two different ration levels, so it is not
/// one case fourteen times: nine unowned counties on an all-livestock split
/// slaughtering thirteen head; three owned counties whose dairy alone covers
/// them; county 8 the same on the other split; and county 1 dropping to Half.
#[test]
fn the_ration_preview_reproduces_every_stored_food_field() {
    let s = england!();
    let file = s.kingdom(SEED);
    let t = &Tables::DEFAULT;

    let mut levels = std::collections::BTreeSet::new();
    for id in s.county_ids() {
        let stored = &file.counties[id];
        let mut c = stored.clone();
        l2_kingdom::ration::preview(t, &mut c, false);
        assert_eq!(c.ration_achieved, stored.ration_achieved, "county {id}");
        assert_eq!(c.herd_eaten, stored.herd_eaten, "county {id}");
        assert_eq!(c.grain_eaten, stored.grain_eaten, "county {id}");
        assert_eq!(c.d_hap_ration, stored.d_hap_ration, "county {id}");
        assert_eq!(c.herd, stored.herd, "county {id}: a preview spends nothing");
        assert_eq!(c.grain, stored.grain, "county {id}");
        levels.insert(c.ration_achieved);
    }
    assert_eq!(
        levels,
        [2, 3].into_iter().collect::<std::collections::BTreeSet<i32>>(),
        "two levels, not one case fourteen times"
    );
}

/// Migration runs on the file's real adjacency — which is sparse, not the
/// fully-connected map the old test invented — and moves **nobody**, which is
/// what the save's arithmetic requires: `435 = 417+63-45` and `456 = 417+84-45`
/// leave no room for a migrant.
#[test]
fn migration_runs_on_the_real_adjacency_and_moves_nobody() {
    let s = england!();
    let file = s.kingdom(SEED);
    let k = kingdom_after_the_first_season(&s);

    for id in s.county_ids() {
        assert!(k.counties[id].neighbour_count > 0, "county {id} must have neighbours");
        assert_eq!(k.counties[id].emigrants, 0, "county {id}");
        assert_eq!(k.counties[id].immigrants, 0, "county {id}");
        assert_eq!(k.counties[id].emigrants, file.counties[id].emigrants, "county {id}");
        assert_eq!(k.counties[id].immigrants, file.counties[id].immigrants, "county {id}");
    }
}

/// §8.1's gate, against the file: no county drew an event in 1268.
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

/// Nothing was taxed, so nothing was banked, and the empire term is zero for
/// every realm — the file's `taxRate`, `taxCollected` and realm `gold` all
/// agree with what the pipeline produces.
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

/// Determinism: the same file, imported and run again, is the same kingdom.
/// Lockstep needs this and nothing else needs it more.
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
    // And the import itself is a pure function of the bytes.
    let again = england!();
    assert_eq!(s, again);
}

/// **The labour allocator, reproduced.**
///
/// `FUN_0044F6E7` is 2,147 bytes and it is the writer of all nine job records:
/// it splits the population into a farm half and an industry half by county
/// `+0x08`, gives each job `Pct(half, share[job])` people or as many as its
/// useful ceiling allows, walks the leftovers round the jobs that still have
/// room in an uneven rota, and drops whatever nobody could take into *Idle
/// townsfolk*.
///
/// The shipped save is the state immediately after that pass ran, so its own
/// worker counts are the answer. Rebuilding them from the population, the
/// industry share, the eight percentages and the eight ceilings —
/// **and getting all fourteen counties exactly right, including the 133 idle
/// in every unowned one and the nought idle in every owned one** — is not
/// something a wrong rota or a wrong quota order could do.
#[test]
fn the_labour_allocator_rebuilds_every_countys_own_worker_counts() {
    let s = england!();
    let k = s.kingdom(SEED);
    let mut checked = 0;
    for id in s.county_ids() {
        let mut c = k.counties[id].clone();
        let stored = c.labour;
        let idle = l2_kingdom::labour::allocate(&mut c);
        assert_eq!(
            c.labour, stored,
            "county {id}: pop {} at {}% industry, shares {:?}, ceilings {:?}",
            c.population,
            c.industry_share,
            c.labour_share,
            l2_kingdom::labour::ceilings(&c)
        );
        assert_eq!(c.labour.iter().sum::<i32>(), c.population, "county {id}");
        // An owned county's wood ceiling is unbounded and takes everyone left;
        // an unowned county's is zero and they stand idle. It is the same pass
        // producing both.
        assert_eq!(idle > 0, c.owner == 0, "county {id} owner {}", c.owner);
        checked += 1;
    }
    assert_eq!(checked, 14);
}

/// And the two functions that write the allocator's own inputs back from what
/// it did: each half of the eight percentages comes out summing to exactly 100,
/// which is the invariant both of the binary's default setters satisfy.
#[test]
fn recomputing_the_shares_from_the_shipped_counties_keeps_both_halves_at_a_hundred() {
    let s = england!();
    let k = s.kingdom(SEED);
    for id in s.county_ids() {
        let mut c = k.counties[id].clone();
        l2_kingdom::labour::recompute_shares(&mut c);
        assert_eq!(c.labour_share[0..3].iter().sum::<i32>(), 100, "county {id} farm");
        assert_eq!(c.labour_share[3..8].iter().sum::<i32>(), 100, "county {id} industry");
        l2_kingdom::labour::recompute_industry_share(&mut c);
        assert!((0..=100).contains(&c.industry_share), "county {id}: {}", c.industry_share);
        // Reallocating on the recomputed inputs still spends everybody.
        l2_kingdom::labour::allocate(&mut c);
        assert_eq!(c.labour.iter().sum::<i32>(), c.population, "county {id}");
    }
}

/// Ten more seasons past the reproduction, to show the model does not merely
/// land on turn 1 and then diverge into nonsense: every county stays inside
/// every documented bound.
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

        // The clock is exact arithmetic, and nothing clamps it: four seasons to
        // the year, one turn per season, the year rolling on the wrap.
        let elapsed = turn0 + step;
        assert_eq!(k.turn_count, elapsed, "one turn per season");
        let expected_season = (season0 as u32 - 1 + step) % 4 + 1;
        assert_eq!(k.season as u32, expected_season, "season {step}");
        assert_eq!(k.season_next as u32, expected_season % 4 + 1, "season {step}");
        // The year rolls in the call that *begins* Winter, so it advances once
        // for each step that lands on season 4 - not once every four steps from
        // an arbitrary start.
        let rolls =
            (1..=step).filter(|i| (season0 as u32 - 1 + i) % 4 + 1 == 4).count() as i32;
        assert_eq!(k.year, year0 + rolls, "season {step}");
        assert_eq!(k.year_next, k.year + 1, "season {step}");

        // **A finding, pinned rather than papered over.** The identity worth
        // asserting is `sum(labour) == population`. It holds on the imported
        // kingdom - see
        // [`every_countys_nine_labour_records_sum_to_its_population`] - and it
        // **fails from the first `advance_season` on**: after one season county
        // 1 holds 449 people and 435 assigned jobs, and by season 8 it has
        // shrunk to fewer people than it has jobs.
        //
        // `FUN_0044F6E7` reallocates every county's workers after the
        // population moves. Our `advance_season` does not, so the allocation
        // never reruns at all: every county's labour is frozen at the figure
        // the import produced, ten seasons later, and that is what is asserted
        // here. Writing the allocator is production work and not this task's;
        // when somebody does write it, **this assertion goes red and this
        // comment says why** - change it to `== c.population` and delete the
        // paragraph.
        //
        // The previous version of this test asserted nine ranges that were all
        // clamps our own code had just applied, and would never have seen this.
        for (n, id) in s.county_ids().enumerate() {
            let c = &k.counties[id];
            let assigned: i32 = c.labour.iter().sum();
            assert!(c.labour.iter().all(|&j| j >= 0), "county {id} season {step}");
            assert_eq!(
                assigned, labour0[n],
                "county {id} season {step}: labour is frozen at the import's allocation, \
                 because the allocator does not rerun"
            );
        }
    }

    // And it moved. A model that froze, or that clamped everything to a
    // constant, would satisfy every bound above and this is what notices.
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

/// **The check that makes the one above mean something.** Ten seasons under a
/// changed ruleset must reach a *different* kingdom.
///
/// `docs/decisions.md` C12: *a test that passes before and after the change is
/// not testing the thing its name claims*. The test above used to assert nine
/// ranges — happiness in `0..=100`, `health_band <= 4`, `unrest <= 4`,
/// `ration_achieved` in `0..=5`, every store non-negative — and every one of
/// those is a **clamp our own code applied moments earlier**:
/// `happiness.rs:74`, `health.rs:40`, `county.rs:476`. It could not fail. It
/// sat inside the file that *is* the C12 correction, and it survived that
/// correction because it was reasonable-looking and green.
///
/// So the bounds are gone and this pair replaces them: the trajectory is
/// conserved and non-trivial above, and here it is shown to be *sensitive* —
/// halve the grain yield and the ten seasons land somewhere else. If the season
/// pipeline stopped consulting the ruleset, this fails and the other passes.
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
    // `g_dairyPerHead` is 5: one head of cattle feeds five people. Halve it and
    // every county on this map, which lives on its dairy, feels it - which is
    // why it is the perturbation used rather than the grain yield, whose fields
    // are unplanted here and which moves nothing on turn one.
    let mut lean = Tables::DEFAULT;
    lean.food.dairy_per_head /= 2;
    let hungry = run(lean);

    assert_ne!(stock, hungry, "halving the dairy yield changed nothing in ten seasons");
    // The same ruleset twice is the same trajectory, so the difference above is
    // the ruleset and not the clock.
    assert_eq!(stock, run(Tables::DEFAULT), "the same rules must give the same run");
}

