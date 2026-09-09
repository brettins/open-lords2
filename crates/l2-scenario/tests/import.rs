//! The seam, against a real save.
//!
//! ```text
//! LORDS2_FIXTURES="E:\dev\lords2-fixtures" cargo test -p l2-scenario
//! ```
//!
//! Two halves, and they are gated differently, which is the point of the split:
//!
//! * **The import *is* the file** — every value the kingdom ends up holding can
//!   be pointed at a byte. Where the numbers are the England turn-one position's
//!   own, the test is gated on that named fixture (`l2_testkit::england_turn1`)
//!   and fails loudly if handed a different game.
//! * **A save this code has misread is refused** rather than half-loaded. Each
//!   refusal below corrupts one byte of a real save and checks which error comes
//!   back, which is the only way to know the guards are reachable at all — and
//!   that is a property of the importer, not of any scenario, so it runs over
//!   **every** save the machine can offer.
//!
//! Nothing here reads a save out of the game install by a hard-coded path. That
//! is what broke: the install's `lastturn.sav` is the rolling autosave and three
//! directories on this machine hold a different game under that name.

use l2_formats::save::{Layout, Save, COUNTY_BASE, COUNTY_STRIDE};
use l2_scenario::{ImportError, Scenario, STARTING_HEALTH_METER};
use l2_testkit::{saves, SaveFile};

/// Overwrite one byte of a *copy* of a save, at a runtime address, using the
/// layout the executable itself supplies. Nothing is written to any install.
fn poke(exe: &[u8], sav: &[u8], va: u32, value: u8) -> Vec<u8> {
    let layout = Layout::from_executable(exe).expect("block table");
    let at = layout.offset_of(va).expect("a saved address");
    let mut out = sav.to_vec();
    out[at] = value;
    out
}

/// Corrupt one byte of every reachable save and check the same error comes back
/// from all of them. A guard that is only reachable on one file is not a guard
/// anybody can rely on.
fn refusal_over_every_save(
    va: impl Fn(&SaveFile) -> u32,
    value: u8,
    expected: impl Fn(&SaveFile) -> ImportError,
) {
    let exe = l2_testkit::executable!();
    let saves = saves!();
    for s in &saves {
        let bytes = std::fs::read(&s.path).expect("re-read");
        let poked = poke(&exe, &bytes, va(s), value);
        let save = Save::open(&exe, &poked).expect("still the right length");
        assert_eq!(Scenario::from_save(&save), Err(expected(s)), "{}", s.label());
    }
    eprintln!("refusal reached on {} saves", saves.len());
}

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

/// The two kingdoms are different games, and the difference is exactly one
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
// machine offers rather than over the one file somebody happened to have. The
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

/// A weather byte outside 0..=5 is refused rather than defaulted to Cloudy.
#[test]
fn a_weather_byte_naming_nothing_is_refused() {
    refusal_over_every_save(
        |_| COUNTY_BASE + COUNTY_STRIDE as u32 + 0x21B,
        9,
        |_| ImportError::Weather { county: 1, byte: 9 },
    );
}

/// A neighbour id off the end of the map is refused: adjacency drives migration
/// and the regional weather swing, and a stale id would quietly point one of
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

/// More counties than `g_counties` can hold is refused rather than truncated.
#[test]
fn a_county_count_the_array_cannot_hold_is_refused() {
    refusal_over_every_save(|_| 0x0056_D5DC, 99, |_| ImportError::CountyCount(99));
}

/// And `g_localPlayer` outside the five realms.
#[test]
fn a_local_player_that_is_not_a_realm_is_refused() {
    refusal_over_every_save(|_| 0x0057_C8CC, 7, |_| ImportError::LocalPlayer(7));
}

/// **The other two words of the labour record**, which this crate read as
/// nothing until the village screen needed them.
///
/// The worker count is word 0 of a twelve-byte record; words 1 and 2 are a
/// *wanted floor* and a *useful ceiling*. Nothing but the right offsets
/// produces the pattern below, and the pattern is the whole argument:
///
/// * **Seven of the nine floors are −1 in all fourteen counties.** Only the
///   cattle estimate (`FUN_0044DD4D`) and the grain estimate (`FUN_0044D374`)
///   ever write a real floor, and the shipped save is a Winter save with no
///   grain sown, so cattle is the only one with a number in it. A misread
///   offset does not produce ninety-eight −1s.
/// * **Wood's ceiling is exactly 100,000 in every owned county and exactly 0
///   in every unowned one** — `FUN_0044F318` writes `LABOUR_UNBOUNDED` for an
///   industry the county has and 0 for one it does not.
/// * And that single byte explains the save's whole labour split: the
///   allocator fills each job up to its ceiling and drops the remainder into
///   *Idle townsfolk*, so an owned county has 217 foresters and nobody idle
///   while an unowned one has no forester and 133 idle.
#[test]
fn the_labour_records_other_two_words_are_a_wanted_floor_and_a_useful_ceiling() {
    use l2_kingdom::county::{LABOUR_NO_FLOOR, LABOUR_UNBOUNDED};
    use l2_kingdom::tables::{
        JOB_CATTLE_FARMING, JOB_COUNT, JOB_IDLE_TOWNSFOLK, JOB_WOOD_CUTTING,
    };

    let save = l2_testkit::england!();
    let s = Scenario::from_save(&save).unwrap();

    let mut floors = 0;
    for id in s.county_ids() {
        let Some(c) = s.counties[id].as_ref() else { continue };

        for job in 0..JOB_COUNT {
            if job == JOB_CATTLE_FARMING || job == JOB_IDLE_TOWNSFOLK {
                continue;
            }
            assert_eq!(
                c.labour_wanted[job], LABOUR_NO_FLOOR,
                "county {id} job {job}: only grain and cattle ever ask for a floor"
            );
            floors += 1;
        }
        // Idle townsfolk is the one slot no estimate ever touches, so both its
        // spare words are still the zero `FUN_00451150` cleared them to.
        assert_eq!(c.labour_wanted[JOB_IDLE_TOWNSFOLK], 0, "county {id}");
        assert_eq!(c.labour_useful[JOB_IDLE_TOWNSFOLK], 0, "county {id}");

        // Break-even staffing is never above growth-maximising staffing, and
        // both are real counts a county could actually field.
        let (want, useful) = (c.labour_wanted[JOB_CATTLE_FARMING], c.labour_useful[1]);
        assert!(want > 0 && want <= useful, "county {id}: cattle {want} .. {useful}");
        assert!(useful <= c.population, "county {id}: {useful} tenders of {} people", c.population);

        // The ceiling is what decides whether the county's spare people work.
        let owned = c.owner != 0;
        assert_eq!(
            c.labour_useful[JOB_WOOD_CUTTING],
            if owned { LABOUR_UNBOUNDED } else { 0 },
            "county {id} owner {}", c.owner
        );
        assert_eq!(
            c.labour[JOB_WOOD_CUTTING] > 0,
            owned,
            "county {id}: an unbounded ceiling is why anyone cuts wood"
        );
        assert_eq!(
            c.labour[JOB_IDLE_TOWNSFOLK] > 0,
            !owned,
            "county {id}: and a ceiling of zero is why the rest stand idle"
        );
    }
    assert_eq!(floors, 14 * 7, "fourteen counties, seven floorless jobs each");
}

/// **The field counts we derive are the counts every reachable save stores.**
///
/// The importer no longer carries `+0x1FF`, `+0x200` and `+0x201` across: it
/// reads the twenty tiles in `g_countyFieldTiles`, applies
/// `County_RecountFields`' terrain ladder to the tile planes, and lets the
/// kingdom hold what that makes. `CountyState` still carries what the file
/// said, so the two are independent readings of the same thing and this diffs
/// them.
///
/// It runs over **every** save the machine can offer rather than over the one
/// named fixture, because `docs/decisions.md` C26 is what happens when a rule
/// is checked against one value of its input — and the battle saves are a
/// different map with four counties, which is exactly the second value.
#[test]
fn the_field_counts_are_derived_and_they_match_every_save_that_stores_them() {
    let saves = saves!();
    let mut counties_checked = 0;
    for SaveFile { name, save, .. } in &saves {
        let Ok(scenario) = Scenario::from_save(save) else { continue };
        let kingdom = scenario.kingdom(1);
        for id in scenario.county_ids() {
            let Some(stored) = &scenario.counties[id] else { continue };
            let ours = &kingdom.counties[id];
            counties_checked += 1;
            assert_eq!(
                (ours.fields_fallow, ours.fields_cattle, ours.fields_grain),
                (stored.fields_fallow, stored.fields_cattle, stored.fields_grain),
                "{name} county {id}: recounting its {} field tiles disagrees with the file",
                ours.field_slots_used()
            );
        }
    }
    assert!(counties_checked > 14, "only {counties_checked} counties reached");
}

// --- the campaign layer -----------------------------------------------------

/// **Every unit in every save survives the seam**, slot for slot and field for
/// field.
///
/// The importer used to read the counties, the realms and the map and stop
/// there, so a loaded game had an economy and an empty board. This is the
/// assertion that fails if the block goes back on the floor, and it runs over
/// every save because one file agreeing proves nothing: the England fixture is
/// six merchants, and the battle and siege saves are armies, a garrison, a
/// besieger and a levied defence.
#[test]
fn every_unit_in_every_save_is_imported_slot_for_slot() {
    let saves = saves!();
    let mut checked = 0;
    for SaveFile { name, save, .. } in &saves {
        let scenario = Scenario::from_save(save).expect("the save imports");
        let stored: Vec<_> =
            save.units().unwrap().into_iter().filter(|u| u.is_live()).collect();
        assert_eq!(
            scenario.units.len(),
            stored.len(),
            "{name}: {} units in the file, {} imported",
            stored.len(),
            scenario.units.len()
        );
        let kingdom = scenario.kingdom(1);
        for (n, f) in stored.iter().enumerate() {
            let (slot, ours) = &scenario.units[n];
            let at = format!("{name} unit {}", f.index);
            assert_eq!(*slot, f.index, "{at}: the slot moved");
            assert_eq!(ours.owner, f.owner, "{at}: owner");
            assert_eq!(ours.kind.byte(), f.kind, "{at}: kind");
            assert_eq!((ours.x, ours.y), (f.x, f.y), "{at}: tile");
            assert_eq!(ours.county, f.county, "{at}: county");
            assert_eq!(ours.home_county, f.home_county, "{at}: home county");
            assert_eq!(ours.men, f.men, "{at}: men");
            assert_eq!(ours.wages, f.wages, "{at}: wages");
            assert_eq!(ours.morale, f.morale as i32, "{at}: morale");
            assert_eq!(ours.name_index, f.name_index, "{at}: +0x14F");
            assert_eq!(ours.year_formed, f.year_formed as i32, "{at}: +0x164");
            // `+0x167` is one byte with two meanings, and `Unit` models it as
            // two fields: the defence mark on an army or a mob, the cargo county
            // on a transport — and, on a merchant, the county it was spawned in,
            // which lands in the same non-army field.
            let (mark, cargo) =
                if ours.kind.is_combatant() { (f.role, 0) } else { (0, f.role) };
            assert_eq!(ours.defence_mark, mark, "{at}: +0x167 as a defence mark");
            assert_eq!(ours.cargo_county, cargo, "{at}: +0x167 as a cargo county");
            assert_eq!(ours.garrison_county, f.garrison_county, "{at}: garrison");
            assert_eq!(ours.besieging_county, f.besieging_county, "{at}: besieging");
            assert_eq!(ours.needs_destination, f.needs_destination, "{at}: idle");
            assert_eq!(ours.dest_county, f.dest_county, "{at}: dest county");
            assert_eq!(ours.path.len(), f.path_len as usize, "{at}: path length");
            assert_eq!(ours.moves_used, f.moves_used as i32, "{at}: moves used");
            // **Not `kind.move_allowance()`.** The tick handler writes it, so
            // the file's zero is the truth for a unit that has not been ticked.
            assert_eq!(ours.move_allowance, f.move_allowance as i32, "{at}: allowance");
            for t in 0..7 {
                assert_eq!(ours.troops[t], f.troops[t] as i32, "{at}: troop {t}");
            }
            // And the kingdom got it in the same slot, which is what
            // `garrison_unit`, `besieged_by` and the merchant route index all
            // depend on.
            assert_eq!(kingdom.campaign.units.get(*slot), Some(ours), "{at}: in the kingdom");
            checked += 1;
        }
    }
    eprintln!("{checked} units imported across {} saves", saves.len());
    assert!(checked > 6, "only {checked} units reached; the battle saves add armies");
}

/// The route table reaches the kingdom unchanged, row for row.
///
/// `l2-kingdom` has had `MerchantRoutes` and `Merchant_AdvanceAll` since the
/// turn movers landed and **nothing but a test had ever filled the table**, so
/// a game loaded from a save arrived with six empty rows. This is the assertion
/// that they arrive full.
#[test]
fn the_merchant_routes_reach_the_kingdom() {
    let saves = saves!();
    for SaveFile { name, save, .. } in &saves {
        let scenario = Scenario::from_save(save).expect("imports");
        let k = scenario.kingdom(1);
        let stored = save.merchant_routes().unwrap();
        for route in 0..l2_kingdom::merchant::ROUTES {
            assert_eq!(k.campaign.routes.row(route), &stored[route], "{name}: route {route}");
        }
        assert_eq!(
            scenario.merchant_start,
            save.merchant_start_counties().unwrap(),
            "{name}: start counties"
        );
        // One merchant per start county **before the first zero** —
        // `Merchant_SpawnAll` breaks rather than skipping, so a later non-zero
        // entry never spawns anything.
        let spawned = scenario.merchant_start.iter().take_while(|&&c| c != 0).count();
        assert_eq!(
            spawned as i32,
            save.globals().unwrap().merchant_count,
            "{name}: merchants against start counties"
        );
        assert_eq!(
            k.campaign.units.iter().filter(|(_, u)| u.kind == l2_kingdom::UnitKind::Merchant).count(),
            spawned,
            "{name}: and against the units that arrived"
        );
    }
}

/// The England position imports as six merchants owned by nobody, in the six
/// counties `docs/formats/plane4.md` predicted, each with the route it will
/// walk.
#[test]
fn the_england_fixture_imports_six_merchants_and_no_armies() {
    let save = l2_testkit::england!();
    let s = Scenario::from_save(&save).unwrap();
    assert_eq!(s.units.len(), 6);
    assert_eq!(s.merchant_start, [14, 5, 13, 11, 12, 4]);

    for (n, (slot, u)) in s.units.iter().enumerate() {
        assert_eq!(*slot, n + 1);
        assert_eq!(u.kind, l2_kingdom::UnitKind::Merchant);
        assert_eq!(u.owner, 6, "a merchant belongs to nobody");
        assert_eq!(u.county, s.merchant_start[n]);
        assert_eq!(u.cargo_county, s.merchant_start[n], "+0x167 is where it was born");
        assert_eq!(u.move_allowance, 0, "the file's zero, not the type's ten");
        assert_eq!(u.year_formed, 1, "the route cursor ships at 1, not 0");
        assert!(u.needs_destination);
        // Each merchant is standing in a county on its own route, and the row
        // it walks is its **slot** minus one — the coupling `Merchant_AdvanceAll`
        // rests on.
        let route = s.routes.row(slot - 1);
        assert!(
            route.contains(&u.county),
            "merchant {slot} started in county {}, which is not on route {route:?}",
            u.county,
        );
    }
    assert!(s.units.iter().all(|(_, u)| u.men == 0), "the England position has no troops on the map");
}

/// A unit whose `+0x0C` disagrees with its `x`/`y` is refused, because the two
/// can only disagree if the array is being read at the wrong stride.
#[test]
fn a_unit_whose_tile_offset_disagrees_with_its_tile_is_refused() {
    let exe = l2_testkit::executable!();
    for s in &saves!() {
        let bytes = std::fs::read(&s.path).expect("re-read");
        // Slot 1 exists in every save the machine has: the merchants take the
        // low slots. `+0x0A` is its x.
        let va = l2_formats::save::UNIT_BASE + l2_formats::save::UNIT_STRIDE as u32 + 0x0A;
        let x = s.save.u8_at(va).unwrap();
        let poked = poke(&exe, &bytes, va, x.wrapping_add(1));
        let save = Save::open(&exe, &poked).expect("still the right length");
        assert!(
            matches!(Scenario::from_save(&save), Err(ImportError::UnitTile { unit: 1, .. })),
            "{}: a moved unit was accepted",
            s.label()
        );
    }
}

/// A type byte naming no handler is refused rather than imported as something.
#[test]
fn a_unit_type_that_names_no_handler_is_refused() {
    let exe = l2_testkit::executable!();
    for s in &saves!() {
        let bytes = std::fs::read(&s.path).expect("re-read");
        let va = l2_formats::save::UNIT_BASE + l2_formats::save::UNIT_STRIDE as u32 + 0x08;
        let poked = poke(&exe, &bytes, va, 5);
        let save = Save::open(&exe, &poked).expect("still the right length");
        assert_eq!(
            Scenario::from_save(&save),
            Err(ImportError::UnitKind { unit: 1, byte: 5 }),
            "{}",
            s.label()
        );
    }
}

/// **Realm `+0x0A` is the shield index, and a default game sets it to the realm
/// id.** Read rather than assumed: the importer used to fill this field with the
/// realm id on the strength of `Game_SetupRealmsAndCounties` doing so, which is
/// true of a *default* game and not of one whose colour picker has run
/// (`0x0049CE1F` walks a free-slot pool). Now that `l2-formats` reads the byte,
/// this asserts the assumption it replaced — so if a fixture ever carries a
/// permuted set, it says so here rather than silently changing every flag.
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
