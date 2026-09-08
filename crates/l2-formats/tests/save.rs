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
