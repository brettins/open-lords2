//! **The reproduction from the shipped save** — and this time it opens it.
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
//! slots; and the human owning county 8 alone. `crates/l2-formats/tests/save.rs`
//! asserts all of that against the bytes.
//!
//! # What it is now
//!
//! An oracle. `l2-scenario` imports `lastturn.sav` twice — once as the state the
//! file holds, once rewound to the position it was taken from — and every
//! assertion below compares this crate's output against the *file's* numbers.
//! Nothing here is quoted from a document. If the pipeline's order, the health
//! ladder's comparison sense, the delta table's indexing, the 1-based season
//! array, the birth ladder's pairing or the unowned happiness bonus were wrong,
//! a number read out of `lastturn.sav` would say so.
//!
//! **No rule was changed to make this pass.** Fourteen counties reproduce
//! twenty-six stored fields each — see
//! [`every_county_reproduces_every_stored_field`].
//!
//! # The one thing the save does not record
//!
//! It stores `popLast` and `happinessLast`, so the previous season's population
//! and happiness are exact. It stores **no previous herd and no previous
//! grain**: the ration pass ate some of both and only the remainder survives.
//! So the starting position `l2-scenario` builds carries the stored stores
//! forward, and one county of fourteen — county 1, the map's dead end — then
//! starves where the real one did not. That is not a rule failing; it is a
//! missing input, and [`the_food_the_season_ate_is_recoverable_and_unique`]
//! recovers it: there is **exactly one** pre-season store per county that
//! reproduces the file, and putting it back makes the whole map land.
//!
//! Recovering it also answers a question `docs/kingdom.md` §4.3 left open.
//! County 1 holds no grain and its `shownRation` is `+1`, so it was fed at
//! Normal by the pass that ran *before* happiness — and with an all-grain split
//! that costs eight sacks. A `Ration_Apply` that did not debit the store would
//! have left those eight sacks in it. **It debits**, which is the reading
//! `l2_kingdom::ration::apply` already implements.
//!
//! # Running it
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-kingdom --test reproduction
//! ```
//!
//! Every test skips cleanly when no install is present, like the rest of the
//! suite.

use l2_formats::save::Save;
use l2_kingdom::county::County;
use l2_kingdom::phase::SEASON_PIPELINE;
use l2_kingdom::tables::{health_band, Season, Tables, Weather};
use l2_scenario::{Scenario, STARTING_HEALTH_METER};
use std::path::{Path, PathBuf};

/// The seed is irrelevant to everything asserted here — with Advanced Farming
/// off the weather is forced, and no county draws an event in 1268 — but it is
/// fixed anyway, because a test that *would* notice a different seed is a test
/// worth having notice.
const SEED: u64 = 0x10D_52;

fn install() -> Option<PathBuf> {
    std::env::var("LORDS2_DIR")
        .ok()
        .filter(|d| Path::new(d).is_dir())
        .map(PathBuf::from)
        .or_else(|| {
            let fallback = PathBuf::from(r"F:\games\Lords of the Realm II");
            fallback.is_dir().then_some(fallback)
        })
}

/// The shipped scenario, or `None` when there is no install to read it from.
fn scenario() -> Option<Scenario> {
    let dir = install()?;
    let exe = std::fs::read(dir.join("Lords2.exe")).ok()?;
    let sav = std::fs::read(dir.join("lastturn.sav")).ok()?;
    let save = Save::open(&exe, &sav).expect("the shipped save must read");
    Some(Scenario::from_save(&save).expect("the shipped save must import"))
}

macro_rules! shipped {
    () => {
        match scenario() {
            Some(s) => s,
            None => {
                eprintln!("LORDS2_DIR not set - skipping");
                return;
            }
        }
    };
}

/// The one county the imported starting position cannot feed, because the save
/// does not record what it ate. See the module documentation.
const DEAD_END: usize = 1;

/// Everything worth comparing between a county we computed and a county the
/// file holds. Twenty-six fields, named, so a failure says which one.
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
        ("herd_eaten", ours.herd_eaten, file.herd_eaten),
        ("grain_eaten", ours.grain_eaten, file.grain_eaten),
        ("herd", ours.herd, file.herd),
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

/// **The correction, asserted through the importer.** Five owned counties, one
/// for each of realms 1 to 5, at indices 1, 4, 8, 11 and 13 — not four owned by
/// one realm. The person holds county 8 and nothing else.
#[test]
fn the_shipped_scenario_is_five_realms_with_one_county_each() {
    let s = shipped!();
    assert_eq!(s.county_count, 14, "fourteen counties on the England map");
    assert_eq!(s.local_player, 1);

    let owned: Vec<(usize, u8)> = s
        .county_ids()
        .filter_map(|id| s.counties[id].as_ref().map(|c| (id, c.owner)))
        .filter(|(_, owner)| *owner != 0)
        .collect();
    assert_eq!(owned, vec![(1, 5), (4, 4), (8, 1), (11, 3), (13, 2)]);
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
    let s = shipped!();
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
    let s = shipped!();
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
    let s = shipped!();
    let k = s.kingdom(SEED);
    for id in s.county_ids() {
        let c = &k.counties[id];
        assert!(c.neighbour_count > 0, "county {id} borders somebody");
        for &n in c.neighbours() {
            assert!(k.counties[n as usize].neighbours().contains(&(id as u8)), "county {id}");
        }
    }
    assert_eq!(k.counties[DEAD_END].neighbours(), &[2], "county 1 is the dead end");
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
    let s = shipped!();
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

    // Every county in the shipped save started the season on the same two
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
    let s = shipped!();
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
    let s = shipped!();
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
    let s = shipped!();
    let file = s.kingdom(SEED);
    let mut k = s.starting_kingdom(SEED);
    k.start_new_game();

    let mut checked = 0;
    for id in s.county_ids() {
        if id == DEAD_END {
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

/// **County 1 is the divergence, and it is a missing input rather than a broken
/// rule.**
///
/// It holds no grain, splits its ration entirely onto grain, and keeps a herd of
/// 74 — which feeds 370 of its 417 people. The file says it ate at Normal
/// (`shownRation = +1`), so it must have had grain to eat. Started from the
/// stored zero it drops to Half, and the whole five-stage chain follows the
/// drop: −2 instead of +1 on happiness, a health delta that leaves the meter in
/// band 2 instead of band 3, and a death rate that costs it twenty-one people.
///
/// Pinned to the exact numbers, so a change to any stage of the chain moves this
/// test rather than passing quietly.
#[test]
fn county_one_diverges_because_the_save_does_not_record_what_it_ate() {
    let s = shipped!();
    let file = s.kingdom(SEED);
    let mut k = s.starting_kingdom(SEED);
    k.start_new_game();

    let ours = &k.counties[DEAD_END];
    let theirs = &file.counties[DEAD_END];

    assert_eq!((theirs.grain, theirs.herd, theirs.ration_split), (0, 74, 0), "the file's county 1");
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
    let s = shipped!();
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

    assert_eq!(
        solve_opening(&file.counties[DEAD_END], t),
        (74, 8),
        "county 1 opened on eight sacks and ate all of them"
    );
    for id in [4usize, 8, 11, 13] {
        let stored = &file.counties[id];
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
    let s = shipped!();
    let file = s.kingdom(SEED);
    let k = kingdom_after_the_first_season(&s);

    let mut checked = 0;
    for id in s.county_ids() {
        for (name, ours, theirs) in comparison(&k.counties[id], &file.counties[id]) {
            assert_eq!(ours, theirs, "county {id} {name}");
            checked += 1;
        }
    }
    assert_eq!(checked, 14 * 26);
}

/// The five-stage chain — ration → health meter → health band → happiness →
/// birth rate → population — on the two bands the map lands on, with every
/// number on both sides read from the file.
#[test]
fn the_whole_five_stage_chain_lands_on_both_bands() {
    let s = shipped!();
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
    let s = shipped!();
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
    let s = shipped!();
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
    let s = shipped!();
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
    let s = shipped!();
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
    let s = shipped!();
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
    let again = shipped!();
    assert_eq!(s, again);
}

/// Ten more seasons past the reproduction, to show the model does not merely
/// land on turn 1 and then diverge into nonsense: every county stays inside
/// every documented bound.
#[test]
fn ten_more_seasons_keep_every_value_inside_its_documented_range() {
    let s = shipped!();
    let mut k = kingdom_after_the_first_season(&s);
    for season in 1..=10 {
        k.advance_season();
        for id in s.county_ids() {
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
