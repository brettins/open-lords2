//! The seam, against the shipped save.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-scenario
//! ```
//!
//! Two halves. The first is that the import *is* the file — every value the
//! kingdom ends up holding can be pointed at a byte. The second is that a save
//! this code has misread is **refused** rather than half-loaded: each test below
//! corrupts one byte of a real save and checks which error comes back, which is
//! the only way to know the guards are reachable at all.

use l2_formats::save::{Layout, Save, COUNTY_BASE, COUNTY_STRIDE};
use l2_scenario::{ImportError, Scenario, STARTING_HEALTH_METER};
use std::path::{Path, PathBuf};

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

fn bytes() -> Option<(Vec<u8>, Vec<u8>)> {
    let dir = install()?;
    Some((
        std::fs::read(dir.join("Lords2.exe")).ok()?,
        std::fs::read(dir.join("lastturn.sav")).ok()?,
    ))
}

/// Overwrite one byte of a *copy* of the save, at a runtime address, using the
/// layout the executable itself supplies. Nothing is written to the install.
fn poke(exe: &[u8], sav: &[u8], va: u32, value: u8) -> Vec<u8> {
    let layout = Layout::from_executable(exe).expect("block table");
    let at = layout.offset_of(va).expect("a saved address");
    let mut out = sav.to_vec();
    out[at] = value;
    out
}

macro_rules! shipped {
    () => {
        match bytes() {
            Some(b) => b,
            None => {
                eprintln!("LORDS2_DIR not set - skipping");
                return;
            }
        }
    };
}

#[test]
fn the_shipped_save_imports_as_fourteen_counties_and_five_realms() {
    let (exe, sav) = shipped!();
    let save = Save::open(&exe, &sav).unwrap();
    let s = Scenario::from_save(&save).unwrap();

    assert_eq!(s.county_count, 14);
    assert_eq!(s.county_ids().count(), 14);
    assert_eq!(s.local_player, 1);
    assert_eq!(s.weather_county, 2);
    assert_eq!(s.realms.len(), 6);
    assert!(!s.realms[0].in_play, "realm 0 is an array slot");
    assert_eq!(s.realms.iter().filter(|r| r.in_play).count(), 5);
}

/// The imported kingdom carries the file's numbers, county by county. This is
/// the assertion that fails if the seam drops a field on the floor.
#[test]
fn every_imported_county_holds_what_the_file_holds() {
    let (exe, sav) = shipped!();
    let save = Save::open(&exe, &sav).unwrap();
    let s = Scenario::from_save(&save).unwrap();
    let k = s.kingdom(1);

    for c in save.counties().unwrap().iter().filter(|c| c.is_county()) {
        let ours = &k.counties[c.index];
        let at = c.index;
        assert_eq!(ours.owner, c.owner, "county {at}");
        assert_eq!(ours.population, c.population, "county {at}");
        assert_eq!(ours.pop_last, c.pop_last, "county {at}");
        assert_eq!(ours.happiness, c.happiness as i32, "county {at}");
        assert_eq!(ours.happiness_last, c.happiness_last as i32, "county {at}");
        assert_eq!(ours.health_meter, c.health_meter as i32, "county {at}");
        assert_eq!(ours.health_band, c.health_band as u8, "county {at}");
        assert_eq!(ours.births, c.births, "county {at}");
        assert_eq!(ours.deaths, c.deaths, "county {at}");
        assert_eq!(ours.pop_band, c.pop_band as i32, "county {at}");
        assert_eq!(ours.castle_type, c.castle_type, "county {at}");
        assert_eq!(ours.grain, c.grain, "county {at}");
        assert_eq!(ours.herd, c.herd, "county {at}");
        assert_eq!(ours.herd_eaten, c.herd_eaten, "county {at}");
        assert_eq!(ours.grain_eaten, c.grain_eaten, "county {at}");
        assert_eq!(ours.ration_wanted, c.ration_wanted as i32, "county {at}");
        assert_eq!(ours.ration_split, c.ration_split as i32, "county {at}");
        assert_eq!(ours.fields_fallow, c.fields_fallow as i32, "county {at}");
        assert_eq!(ours.fields_cattle, c.fields_cattle as i32, "county {at}");
        assert_eq!(ours.fields_grain, c.fields_grain as i32, "county {at}");
        assert_eq!(ours.weather.index(), c.weather, "county {at}");
        assert_eq!(ours.dryness, c.dryness as i32, "county {at}");
        assert_eq!(ours.anchor_x, c.anchor_x, "county {at}");
        assert_eq!(ours.anchor_y, c.anchor_y, "county {at}");
        assert_eq!(ours.neighbours(), c.neighbours(), "county {at}");
    }
    assert_eq!(k.season, 4);
    assert_eq!(k.year, 1268);
    assert_eq!(k.turn_count, 1);
}

/// The two kingdoms are different games, and the difference is exactly one
/// season: the map, the owners and the food stores are shared; the population,
/// happiness and health are not.
#[test]
fn the_starting_kingdom_differs_from_the_saved_one_by_one_season() {
    let (exe, sav) = shipped!();
    let save = Save::open(&exe, &sav).unwrap();
    let s = Scenario::from_save(&save).unwrap();
    let saved = s.kingdom(1);
    let start = s.starting_kingdom(1);

    for id in s.county_ids() {
        let (a, b) = (&start.counties[id], &saved.counties[id]);
        assert_eq!(a.owner, b.owner, "county {id}");
        assert_eq!(a.neighbours(), b.neighbours(), "county {id}");
        assert_eq!(a.herd, b.herd, "county {id}: the save records no earlier herd");
        assert_eq!(a.grain, b.grain, "county {id}");
        assert_eq!(a.population, b.pop_last, "county {id}");
        assert_eq!(a.happiness, b.happiness_last, "county {id}");
        assert_eq!(a.health_meter, STARTING_HEALTH_METER, "county {id}");
        assert_ne!(a.population, b.population, "county {id} grew over the season");
    }
    assert_eq!(start.turn_count, 0, "the starting position has run no season");
    assert_eq!(saved.turn_count, 1);
}

/// A ruleset reaches an imported scenario the same way it reaches any other
/// kingdom, and the seam never learns which one it handed over.
#[test]
fn a_mod_reaches_an_imported_scenario() {
    let (exe, sav) = shipped!();
    let save = Save::open(&exe, &sav).unwrap();
    let s = Scenario::from_save(&save).unwrap();
    let mut rules = l2_kingdom::tables::Tables::DEFAULT;
    rules.grain.yield_per_sack = 24;
    let k = s.kingdom_with_tables(1, rules);
    assert_eq!(k.tables.grain.yield_per_sack, 24);
    assert_eq!(s.kingdom(1).tables, l2_kingdom::tables::Tables::DEFAULT);
}

// --- refusals --------------------------------------------------------------

/// An owner byte naming a realm that does not exist is refused. A save whose
/// arithmetic closed and whose owner byte is 9 has been misread, and half of a
/// misread scenario is worse than none of one.
#[test]
fn an_owner_byte_naming_no_realm_is_refused() {
    let (exe, sav) = shipped!();
    let poked = poke(&exe, &sav, COUNTY_BASE + (2 * COUNTY_STRIDE) as u32 + 0x05, 9);
    let save = Save::open(&exe, &poked).unwrap();
    assert_eq!(Scenario::from_save(&save), Err(ImportError::Owner { county: 2, owner: 9 }));
}

/// A weather byte outside 0..=5 is refused rather than defaulted to Cloudy.
#[test]
fn a_weather_byte_naming_nothing_is_refused() {
    let (exe, sav) = shipped!();
    let poked = poke(&exe, &sav, COUNTY_BASE + (3 * COUNTY_STRIDE) as u32 + 0x21B, 9);
    let save = Save::open(&exe, &poked).unwrap();
    assert_eq!(Scenario::from_save(&save), Err(ImportError::Weather { county: 3, byte: 9 }));
}

/// A neighbour id off the end of the map is refused: adjacency drives migration
/// and the regional weather swing, and a stale id would quietly point one of
/// them at a record that is not a county.
#[test]
fn a_neighbour_off_the_end_of_the_map_is_refused() {
    let (exe, sav) = shipped!();
    let poked = poke(&exe, &sav, COUNTY_BASE + (5 * COUNTY_STRIDE) as u32 + 0x5C, 15);
    let save = Save::open(&exe, &poked).unwrap();
    assert_eq!(Scenario::from_save(&save), Err(ImportError::Neighbour { county: 5, id: 15 }));
}

/// More counties than `g_counties` can hold is refused rather than truncated.
#[test]
fn a_county_count_the_array_cannot_hold_is_refused() {
    let (exe, sav) = shipped!();
    let poked = poke(&exe, &sav, 0x0056_D5DC, 99);
    let save = Save::open(&exe, &poked).unwrap();
    assert_eq!(Scenario::from_save(&save), Err(ImportError::CountyCount(99)));
}

/// And `g_localPlayer` outside the five realms.
#[test]
fn a_local_player_that_is_not_a_realm_is_refused() {
    let (exe, sav) = shipped!();
    let poked = poke(&exe, &sav, 0x0057_C8CC, 7);
    let save = Save::open(&exe, &poked).unwrap();
    assert_eq!(Scenario::from_save(&save), Err(ImportError::LocalPlayer(7)));
}
