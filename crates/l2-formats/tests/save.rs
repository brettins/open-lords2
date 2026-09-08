//! Reading the shipped save, against the shipped save.
//!
//! These are install-gated, like the rest of this crate's corpus tests:
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-formats --test save
//! ```
//!
//! **Why this file exists at all.** `crates/l2-kingdom` has a test headed *"the
//! reproduction from the shipped save"* which never opens the save. It builds a
//! scenario out of `docs/kingdom.md` — four counties owned by the human realm,
//! ten unowned — and then checks that scenario against the rules it was built
//! from. It cannot fail, which is correction C12, and the scenario is wrong: the
//! file holds **five owned counties, one for each of five realms, and nine
//! unowned**. Everything asserted below is read out of `lastturn.sav`, so these
//! tests fail if the reading is wrong.

use l2_formats::save::{Save, SaveError, COUNTY_RECORDS};
use std::env;
use std::path::{Path, PathBuf};

fn install() -> Option<PathBuf> {
    env::var("LORDS2_DIR")
        .ok()
        .filter(|d| Path::new(d).is_dir())
        .map(PathBuf::from)
        .or_else(|| {
            let fallback = PathBuf::from(r"F:\games\Lords of the Realm II");
            fallback.is_dir().then_some(fallback)
        })
}

fn open() -> Option<Save> {
    let dir = install()?;
    let exe = std::fs::read(dir.join("Lords2.exe")).ok()?;
    let sav = std::fs::read(dir.join("lastturn.sav")).ok()?;
    Some(Save::open(&exe, &sav).expect("the shipped save must read"))
}

/// The arithmetic that validates the whole schema. If any block length were
/// misread this would not close, so it is the reason to trust every other
/// number in this file.
#[test]
fn the_block_table_accounts_for_the_file_exactly() {
    let Some(dir) = install() else {
        eprintln!("LORDS2_DIR not set - skipping");
        return;
    };
    let exe = std::fs::read(dir.join("Lords2.exe")).unwrap();
    let sav = std::fs::read(dir.join("lastturn.sav")).unwrap();

    let layout = l2_formats::save::Layout::from_executable(&exe).expect("block table");
    let blocks: u32 = layout.blocks().iter().map(|b| b.len).sum();

    // 267,028 bytes of live memory, then castles.dat as 16 x 12,800.
    assert_eq!(blocks as usize, 267_028, "the saved regions");
    assert_eq!(layout.expected_len(), 471_828, "plus castles.dat");
    assert_eq!(sav.len(), 471_828, "and that is the file");
    assert_eq!(layout.expected_len(), sav.len());
}

/// A save whose size does not match the schema is refused rather than read from
/// the wrong offsets. The failure mode this guards against is silent: wrong
/// offsets still yield numbers.
#[test]
fn a_save_of_the_wrong_size_is_refused() {
    let Some(dir) = install() else {
        eprintln!("LORDS2_DIR not set - skipping");
        return;
    };
    let exe = std::fs::read(dir.join("Lords2.exe")).unwrap();
    let mut sav = std::fs::read(dir.join("lastturn.sav")).unwrap();
    sav.push(0);
    match Save::open(&exe, &sav) {
        Err(SaveError::SizeMismatch { expected, actual }) => {
            assert_eq!(expected, 471_828);
            assert_eq!(actual, 471_829);
        }
        other => panic!("expected a size mismatch, got {other:?}"),
    }
}

/// **The correction.** Five owned counties, one per realm — not four owned by
/// one. The human holds county 8 alone.
#[test]
fn the_shipped_scenario_has_five_owned_counties_one_for_each_realm() {
    let Some(save) = open() else {
        eprintln!("LORDS2_DIR not set - skipping");
        return;
    };
    let counties = save.counties().unwrap();
    assert_eq!(counties.len(), COUNTY_RECORDS);

    let owners: Vec<(usize, u8)> =
        counties.iter().filter(|c| c.is_owned()).map(|c| (c.index, c.owner)).collect();
    assert_eq!(
        owners,
        vec![(1, 5), (4, 4), (8, 1), (11, 3), (13, 2)],
        "five owned counties, one for each of realms 1..=5"
    );

    let real: Vec<_> = counties.iter().filter(|c| c.is_county()).collect();
    assert_eq!(real.len(), 14, "fourteen counties on the England map");
    assert_eq!(real.iter().filter(|c| !c.is_owned()).count(), 9, "nine unowned");

    // Records 0, 15 and 16 are array slots, not places.
    for i in [0, 15, 16] {
        assert!(!counties[i].is_county(), "record {i} is not a county");
    }
}

/// The happiness split the kingdom crate reproduces, read from the file rather
/// than assumed: owned counties store 72, unowned 77, and the difference is the
/// unowned bonus showing up in `shownEvents`.
#[test]
fn owned_counties_store_seventy_two_and_unowned_seventy_seven() {
    let Some(save) = open() else {
        eprintln!("LORDS2_DIR not set - skipping");
        return;
    };
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

/// Population, likewise: every county grew from the same 417, and the birth
/// rate differs only because happiness does.
#[test]
fn population_grew_from_the_same_starting_number_everywhere() {
    let Some(save) = open() else {
        eprintln!("LORDS2_DIR not set - skipping");
        return;
    };
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
    let Some(save) = open() else {
        eprintln!("LORDS2_DIR not set - skipping");
        return;
    };
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
        assert!(c.neighbour_count > 0, "county {} borders somebody", c.index);
    }
}

/// The food split `docs/kingdom.md` §4.3 derives: an unowned county at
/// population 456 with a herd of 67 slaughters 13 head. Read here rather than
/// computed, which is what makes it evidence.
#[test]
fn the_documented_food_split_is_what_the_file_stores() {
    let Some(save) = open() else {
        eprintln!("LORDS2_DIR not set - skipping");
        return;
    };
    let unowned: Vec<_> =
        save.counties().unwrap().into_iter().filter(|c| c.is_county() && !c.is_owned()).collect();
    assert!(!unowned.is_empty());
    for c in &unowned {
        assert_eq!(c.population, 456);
        assert_eq!(c.herd, 67, "county {}", c.index);
        assert_eq!(c.herd_eaten, 13, "county {}: DivCeil(456 - 67*5, 10)", c.index);
        assert_eq!(c.ration_wanted, 3, "normal rations");
    }
}

/// The realm records say the same thing the county owner bytes do, from a
/// different field: **five realms in play, one county each.** `+0x29` is the
/// realm's own count of what it owns, and it is 1 for every one of them — so
/// the "four counties owned by the human realm" scenario is contradicted twice
/// over by the file.
#[test]
fn five_realms_are_in_play_and_each_owns_exactly_one_county() {
    let Some(save) = open() else {
        eprintln!("LORDS2_DIR not set - skipping");
        return;
    };
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

    // Realm 1 is the person; the other four are lords 1, 2, 4 and 3.
    let human: Vec<usize> = realms.iter().filter(|r| r.is_human).map(|r| r.index).collect();
    assert_eq!(human, vec![1], "one human realm");
    assert_eq!(realms[1].lord, 0, "row 0 of every lord-indexed table is the person's");
    assert_eq!(
        realms.iter().skip(2).map(|r| r.lord).collect::<Vec<u8>>(),
        vec![1, 2, 4, 3],
        "four AI lords, and not in realm order"
    );
}

/// The clock, the options and who is playing — the scalars outside the two
/// arrays. `g_localPlayer` is **1**, and realm 1 owns county 8.
#[test]
fn the_globals_are_a_turn_one_winter_game_driven_by_realm_one() {
    let Some(save) = open() else {
        eprintln!("LORDS2_DIR not set - skipping");
        return;
    };
    let g = save.globals().unwrap();
    assert_eq!(g.county_count, 14);
    assert_eq!(g.scenario_index, 0, "the England map");
    assert_eq!(g.local_player, 1);
    assert_eq!((g.season, g.season_next, g.year, g.turn_count), (4, 1, 1268, 1));
    assert_eq!((g.turn_phase, g.turn_phase_step), (1, 0), "parked at the start of phase 1");
    assert_eq!((g.opt_difficulty, g.opt_advanced_farming, g.opt_armies_eat), (0, 0, 0));
    assert_eq!(g.merchant_count, 6);
    assert_eq!(g.weather_county, 2);

    let counties = save.counties().unwrap();
    assert_eq!(counties[8].owner as i32, g.local_player, "the person holds county 8");
}

/// Adjacency is real, and it is **symmetric**: every neighbour list names a
/// county that names it back. That is a property of the data rather than of
/// this reader, so it fails if the ids are being read from the wrong offset.
#[test]
fn the_neighbour_lists_are_symmetric_and_name_only_real_counties() {
    let Some(save) = open() else {
        eprintln!("LORDS2_DIR not set - skipping");
        return;
    };
    let counties = save.counties().unwrap();
    let real: Vec<&l2_formats::save::County> =
        counties.iter().filter(|c| c.is_county()).collect();
    assert_eq!(real.len(), 14);

    for c in &real {
        assert!(!c.neighbours().is_empty(), "county {} borders somebody", c.index);
        for &n in c.neighbours() {
            let n = n as usize;
            assert!((1..=14).contains(&n), "county {} names {n}", c.index);
            assert_ne!(n, c.index, "county {} cannot border itself", c.index);
            assert!(
                counties[n].neighbours().contains(&(c.index as u8)),
                "county {} names {n}, which does not name it back",
                c.index
            );
        }
    }
    // The map is one piece, and the trailing slots really are empty.
    for c in &real {
        for &spare in &c.neighbours[c.neighbour_count as usize..] {
            assert_eq!(spare, 0, "county {} has a stale id past its count", c.index);
        }
    }
    // County 1 is the map's dead end - one neighbour - and it matters later:
    // it is the one county whose ration term does not reproduce.
    assert_eq!(counties[1].neighbours(), &[2]);
}

/// The food configuration, which is where the shipped scenario stops being
/// uniform. Three shapes, and the third is one county on its own:
///
/// * the nine unowned counties: 100 sacks, 67 head, split **100** (all
///   livestock) — and 13 head slaughtered;
/// * four of the five owned counties: no grain at all, a herd big enough that
///   five people per head covers the whole population, and nothing eaten;
/// * **county 1**: no grain, a split of **0** (all grain), and a stored
///   `rationAchieved` of **2** — the only county on the map not on Normal.
///
/// County 1 is also the only county whose `dHapRation` (−2) disagrees with its
/// `shownRation` (+1), which is the fingerprint of the ration pass running
/// twice per season.
#[test]
fn the_food_configuration_has_three_shapes_and_county_one_is_alone_in_the_third() {
    let Some(save) = open() else {
        eprintln!("LORDS2_DIR not set - skipping");
        return;
    };
    let counties = save.counties().unwrap();

    for c in counties.iter().filter(|c| c.is_county() && !c.is_owned()) {
        assert_eq!((c.grain, c.herd, c.ration_split), (100, 67, 100), "county {}", c.index);
        assert_eq!((c.grain_eaten, c.herd_eaten), (0, 13), "county {}", c.index);
        assert_eq!(c.ration_achieved, 3, "county {}", c.index);
        assert_eq!(c.d_hap_ration, 1, "county {}", c.index);
    }

    for c in counties.iter().filter(|c| c.is_owned() && c.index != 1) {
        assert_eq!(c.grain, 0, "owned county {} holds no grain", c.index);
        assert!(c.herd * 5 >= c.population, "owned county {} lives on cheese", c.index);
        assert_eq!((c.grain_eaten, c.herd_eaten), (0, 0), "county {}", c.index);
        assert_eq!(c.ration_achieved, 3, "county {}", c.index);
        assert_eq!(c.d_hap_ration, 1, "county {}", c.index);
    }

    let one = counties[1];
    assert_eq!((one.owner, one.grain, one.herd, one.ration_split), (5, 0, 74, 0));
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
    let Some(save) = open() else {
        eprintln!("LORDS2_DIR not set - skipping");
        return;
    };
    for c in save.counties().unwrap().iter().filter(|c| c.is_county()) {
        assert_eq!(c.grain_available, c.grain, "county {}", c.index);
        assert_eq!(c.herd_available, c.herd, "county {}", c.index);
    }
}
