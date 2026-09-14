#![allow(unused_imports)]
use super::*;
use super::economy::*;
use super::units::*;
use l2_formats::save::{Layout, Save, COUNTY_BASE, COUNTY_STRIDE};
use l2_scenario::{ImportError, Scenario, STARTING_HEALTH_METER};
use l2_testkit::{saves, SaveFile};

/// **Corrected.** `assert_eq!(s.weather_county, 2)` was per-game noise: a second
/// England turn-one save rolls 10. What the importer owes is that it carried
/// across a real county, which is what is checked now.
#[test]
fn the_england_fixture_imports_as_fourteen_counties_and_five_realms() {
    let save = l2_testkit::england!();
    let s = Scenario::from_save(&save).unwrap();

    assert_eq!(s.county_count, 14);
    assert_eq!(s.county_ids().count(), 14);
    assert_eq!(s.local_player, 1);
    assert_eq!(
        s.weather_county as i32, save.globals().unwrap().weather_county,
        "the weather county is whatever the file rolled"
    );
    assert!((1..=s.county_count).contains(&s.weather_county));
    assert_eq!(s.realms.len(), 6);
    assert!(!s.realms[0].in_play, "realm 0 is an array slot");
    assert_eq!(s.realms.iter().filter(|r| r.in_play).count(), 5);
}

/// The imported kingdom carries the file's numbers, county by county. This is
/// the assertion that fails if the seam drops a field on the floor.
#[test]
fn every_imported_county_holds_what_the_file_holds() {
    let save = l2_testkit::england!();
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

/// The two kingdoms are different games
/// season: the map, the owners and the food stores are shared; the population,
/// happiness and health are not.
#[test]
fn the_starting_kingdom_differs_from_the_saved_one_by_one_season() {
    let save = l2_testkit::england!();
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
    let save = l2_testkit::england!();
    let s = Scenario::from_save(&save).unwrap();
    let mut rules = l2_kingdom::tables::Tables::DEFAULT;
    rules.grain.yield_per_sack = 24;
    let k = s.kingdom_with_tables(1, rules);
    assert_eq!(k.tables.grain.yield_per_sack, 24);
    assert_eq!(s.kingdom(1).tables, l2_kingdom::tables::Tables::DEFAULT);
}

// --- refusals --------------------------------------------------------------
//
// These are invariants of the importer, so each runs over **every** save the
// machine offers. The
// old versions poked counties 2, 3 and 5 of `lastturn.sav`; county 5 does not
// exist on every map, which is the same mistake in miniature.

/// An owner byte naming a realm that does not exist is refused. A save whose
/// arithmetic closed and whose owner byte is 9 has been misread, and half of a
/// misread scenario is worse than none of one.
#[test]
fn an_owner_byte_naming_no_realm_is_refused() {
    refusal_over_every_save(
        |_| COUNTY_BASE + COUNTY_STRIDE as u32 + 0x05,
        9,
        |_| ImportError::Owner { county: 1, owner: 9 },
    );
}

/// A weather byte outside 0..=5 is refused.
#[test]
fn a_weather_byte_naming_nothing_is_refused() {
    refusal_over_every_save(
        |_| COUNTY_BASE + COUNTY_STRIDE as u32 + 0x21B,
        9,
        |_| ImportError::Weather { county: 1, byte: 9 },
    );
}

/// A neighbour id off the end of the map is refused: adjacency drives migration
/// and the regional weather swing
/// them at a record that is not a county.
///
/// The poked id is one past *that* save's county count, which differs between
/// maps — the England fixture has fourteen counties and the battle fixtures
/// four.
#[test]
fn a_neighbour_off_the_end_of_the_map_is_refused() {
    let exe = l2_testkit::executable!();
    let saves = saves!();
    for s in &saves {
        let past_the_end = s.save.globals().unwrap().county_count as u8 + 1;
        let bytes = std::fs::read(&s.path).expect("re-read");
        let poked =
            poke(&exe, &bytes, COUNTY_BASE + COUNTY_STRIDE as u32 + 0x5C, past_the_end);
        let save = Save::open(&exe, &poked).unwrap();
        assert_eq!(
            Scenario::from_save(&save),
            Err(ImportError::Neighbour { county: 1, id: past_the_end }),
            "{}",
            s.label()
        );
    }
}

/// More counties than `g_counties` can hold is refused.
#[test]
fn a_county_count_the_array_cannot_hold_is_refused() {
    refusal_over_every_save(|_| 0x0056_D5DC, 99, |_| ImportError::CountyCount(99));
}

/// And `g_localPlayer` outside the five realms.
#[test]
fn a_local_player_that_is_not_a_realm_is_refused() {
    refusal_over_every_save(|_| 0x0057_C8CC, 7, |_| ImportError::LocalPlayer(7));
}

/// **Realm `+0x0A` is the shield index, and a default game sets it to the realm
/// id.** Read: the importer used to fill this field with the
/// realm id on the strength of `Game_SetupRealmsAndCounties` doing so, which is
/// true of a *default* game and not of one whose colour picker has run. Now
/// that `l2-formats` reads the byte, this asserts the assumption it replaced —
/// so if a fixture ever carries a permuted set, it says so here
/// silently changing every flag.
///
/// **The free-slot pool this used to cite as `0x0049CE1F` is inside
/// `Realms_AssignLords` (`0x0049CAAA`, 935 bytes, so `0x0049CE1F` is its tail
/// and not a function).** It is now written out in
/// `l2_scenario::newgame::assign_lords`, and `docs/rules.md` §7a is what it
/// produces for each of the five colours a person can take. An interior address
/// with no name beside it is a citation nobody can follow, so this one
/// went five months without anybody noticing the walk it named was unwritten.
#[test]
fn the_shield_index_of_a_default_game_is_the_realm_id() {
    let save = l2_testkit::england!();
    let s = Scenario::from_save(&save).unwrap();
    for (id, realm) in s.realms.iter().enumerate().take(6).skip(1) {
        if !realm.in_play {
            continue;
        }
        assert_eq!(
            realm.shield_index, id as u8,
            "realm {id} flies shield {} - a permuted colour set, which is legal but has \
             never been seen in a fixture",
            realm.shield_index
        );
    }
}

/// **The three fields the county panels draw and the importer dropped.**
///
/// `+0x0F` and `+0xC0` are the tax panel's *This county* and *People pay*;
/// `+0x10` is the health term the ration panel draws beside the band. All three
/// arrived as `County::new()`'s zero on every loaded game
/// the tax panel was told *"People pay 0 crowns"* whatever the rate and read
/// `( 0 ☺ )` where the original shows `( +5 ☺ )` at rate 0.
///
/// **Ablation, run:** delete `c.tax_shown = *tax_shown;` from
/// `Scenario::apply_counties` and the third clause fails on every save; delete
/// `c.d_hap_tax_local = *d_hap_tax_local;` and the first fails on England
/// where every county stores 5.
#[test]
fn the_county_panels_three_numbers_survive_the_import() {
    let mut checked = 0usize;
    for f in l2_testkit::saves!() {
        let Ok(s) = Scenario::from_save(&f.save) else { continue };
        let k = s.kingdom(1);
        for id in s.county_ids() {
            let base = COUNTY_BASE + (id * COUNTY_STRIDE) as u32;
            let want_local = f.save.i8_at(base + 0x0F).unwrap() as i32;
            let want_health = f.save.i8_at(base + 0x10).unwrap() as i32;
            let want_shown = f.save.i32_at(base + 0xC0).unwrap();
            let c = &k.counties[id];
            assert_eq!(c.d_hap_tax_local, want_local, "{}: county {id} +0x0F", f.label());
            assert_eq!(c.d_hap_health, want_health, "{}: county {id} +0x10", f.label());
            assert_eq!(c.tax_shown, want_shown, "{}: county {id} +0xC0", f.label());
            checked += 1;
        }
    }
    assert!(checked >= 5, "only {checked} counties were reached");
}

// ------------------------------------------------------------ the mercenaries

