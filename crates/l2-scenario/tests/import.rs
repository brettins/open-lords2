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

/// **A castle garrison survives the import, and it does so on every save that
/// has one.**
///
/// The relation has two halves in the original — county `+0x1BC` names the unit,
/// unit `+0x198` names the county — and this importer reads only the unit's,
/// then derives the county's. It had not derived it at all: `County::new` seeds
/// `garrison_unit: 0`, nothing overwrote it, and **every loaded game arrived
/// with no castle garrisoned**. `conquest`'s ownership test, `divide`, `siege`
/// and the campaign map's castle flag are all downstream of that field, and the
/// flag is what exposed it. C59.
///
/// Run over **every** save the machine can offer rather than one fixture,
/// because the failure was silent on all of them: ten of the eleven in the tree
/// carry a garrison and the eleventh is England turn one, which has none because
/// it is turn one. Asserting "both halves agree" on each is what makes this a
/// check of the derivation and not of one file.
#[test]
fn a_castle_garrison_reaches_the_county_it_is_standing_in() {
    let saves = saves!();
    let mut with_a_garrison = 0;
    for s in &saves {
        let scenario = Scenario::from_save(&s.save).expect("a save this code can read");
        let kingdom = scenario.kingdom(1);
        for (slot, unit) in &scenario.units {
            let county = unit.garrison_county as usize;
            if county == 0 {
                continue;
            }
            with_a_garrison += 1;
            assert_eq!(
                kingdom.counties[county].garrison_unit, *slot,
                "{}: unit {slot} says it garrisons county {county}, and the county does not \
                 say so back — the derivation in `skeleton` ran before the county loop \
                 overwrote it, or not at all",
                s.name
            );
            assert_ne!(
                kingdom.counties[county].castle_type, 0,
                "{}: county {county} holds a garrison and has no castle to hold it",
                s.name
            );
        }
    }
    assert!(
        with_a_garrison > 0,
        "no save on this machine has a garrison, so this test asserted nothing"
    );
}

/// **Realm `+0x0A` is the shield index, and a default game sets it to the realm
/// id.** Read rather than assumed: the importer used to fill this field with the
/// realm id on the strength of `Game_SetupRealmsAndCounties` doing so, which is
/// true of a *default* game and not of one whose colour picker has run. Now
/// that `l2-formats` reads the byte, this asserts the assumption it replaced —
/// so if a fixture ever carries a permuted set, it says so here rather than
/// silently changing every flag.
///
/// **The free-slot pool this used to cite as `0x0049CE1F` is inside
/// `Realms_AssignLords` (`0x0049CAAA`, 935 bytes, so `0x0049CE1F` is its tail
/// and not a function).** It is now written out in
/// `l2_scenario::newgame::assign_lords`, and `docs/rules.md` §7a is what it
/// produces for each of the five colours a person can take. An interior address
/// with no name beside it is a citation nobody can follow, which is why this one
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

/// **The realm's treasury and its armoury arrive**, both of them field for
/// field out of the file.
///
/// This test exists because deleting the importer's `realm.weapons = r.weapons`
/// broke **nothing in the workspace**. Every weapon assertion in the tree was
/// downstream of a fixture the test had written itself: `military.rs` sets
/// `weapons = [200; 6]` before it equips anyone, `merchant.rs` reads a stock
/// back after buying it, and `setup.rs` checks the *new-game* table. Nothing
/// read a stock that came off a disk — which is `docs/agents.md`'s rule to the
/// letter: *a field is only tested if something a test reads was written by
/// something the game runs.*
///
/// It matters now because the armoury is the screen that spends them. A levy
/// whose realm imports with an empty armoury is a levy that can only ever be
/// peasants, and the picture would say so — the six weapons hang on the wall
/// only when the realm owns one — so the failure would have been visible and
/// unexplained rather than invisible.
///
/// Asserted against the file rather than against a literal, for the reason the
/// weather-county correction above records: a regenerated fixture is a
/// different game, and `docs/kingdom.md`'s *"50 swords, 50 pikes and 50 bows in
/// all five realms"* is one roll of `g_startArmoury`, not a law.
#[test]
fn every_imported_realm_holds_the_stocks_the_file_holds() {
    let save = l2_testkit::england!();
    let s = Scenario::from_save(&save).unwrap();
    let k = s.kingdom(1);

    let mut checked = 0;
    let mut armed = 0;
    for r in save.realms().unwrap().iter() {
        let ours = &k.realms[r.index];
        assert_eq!(ours.gold, r.gold, "realm {}", r.index);
        assert_eq!(ours.iron, r.iron, "realm {}", r.index);
        assert_eq!(ours.stone, r.stone, "realm {}", r.index);
        assert_eq!(ours.wood, r.wood, "realm {}", r.index);
        assert_eq!(
            ours.weapons.as_slice(),
            r.weapons.as_slice(),
            "realm {}'s armoury did not survive the seam",
            r.index,
        );
        checked += 1;
        if r.weapons.iter().any(|&w| w > 0) {
            armed += 1;
        }
    }
    assert!(checked >= 5, "only {checked} realms were compared");
    // Not a vacuous agreement: a fixture whose realms all had empty armouries
    // would pass the loop above while proving nothing about the field.
    assert!(armed > 0, "every realm in the fixture has an empty armoury: the test proves nothing");
}

/// **A county's four `hasResource` bytes agree with the map, 56 times out of
/// 56** — and until this test existed, none of them was read at all.
///
/// `County_PlaceResourceSites` (`0x00468E61`) writes `+0x295 + c*0x18` at load
/// from the county's `Town`-bank tiles, and `County_PlaceBlacksmith` does the
/// weapons record. So the byte and the map are two recordings of one fact and
/// have to match: an industry has its resource exactly when the county owns a
/// settlement tile (plane-0 bit `0x80`) whose terrain falls in that industry's
/// rung of `Map_Click`'s ladder — 0…3 iron, 4…6 stone, 7…9 weapons, 10…12 wood.
///
/// It is not a vacuous agreement. On the England fixture the answer is *false*
/// for 15 of the 56, iron and stone are complementary in thirteen of the
/// fourteen counties, and county 5 has neither — so a defaulted `true`, which
/// is what the importer used to supply, fails this fifteen times.
#[test]
fn every_industrys_resource_byte_agrees_with_the_tiles_the_map_puts_it_on() {
    let save = l2_testkit::england!();
    let s = Scenario::from_save(&save).unwrap();
    let mut checked = 0;
    let mut without = 0;
    for id in s.county_ids() {
        let Some(c) = &s.counties[id] else { continue };
        // What the map says: the terrain of every settlement tile of this
        // county, run through the same ladder the map click uses.
        let mut from_map = [false; 4];
        for tile in 0..l2_kingdom::map::MAP_TILES {
            if s.map.county[tile] != id as u8
                || s.map.flags[tile] & l2_kingdom::map::flags::SETTLEMENT == 0
            {
                continue;
            }
            if let Some(l2_kingdom::industry::MapToggle::Industry(what)) =
                l2_kingdom::industry::map_toggle_for_graphic(s.map.terrain[tile])
            {
                from_map[what.index()] = true;
            }
        }
        for slot in 0..4 {
            assert_eq!(
                c.industry[slot].has_resource, from_map[slot],
                "county {id} industry {slot}: the record says {} and the map says {}",
                c.industry[slot].has_resource, from_map[slot]
            );
            checked += 1;
            without += usize::from(!from_map[slot]);
        }
    }
    assert_eq!(checked, 56, "fourteen counties, four industries each");
    assert_eq!(without, 15, "and fifteen of the fifty-six have no resource at all");
}

/// **Turn one has exactly five industries switched on: the wood cutting of the
/// five counties that start owned.** Every other switch of all fourteen
/// counties is off.
///
/// This is the byte `Industry_ToggleFromMap` XORs and the one
/// `Labour_Allocate` gates each mining job on, so importing it as a defaulted
/// `true` — which is what happened until C57 — starts the player with four
/// industries running in every county he owns and, worse, puts every one of the
/// map's toggles in the opposite position to the one the player sees.
#[test]
fn only_wood_is_switched_on_at_the_start_and_only_in_an_owned_county() {
    let save = l2_testkit::england!();
    let s = Scenario::from_save(&save).unwrap();
    let mut on = Vec::new();
    for id in s.county_ids() {
        let Some(c) = &s.counties[id] else { continue };
        for slot in 0..4 {
            if c.industry[slot].enabled {
                on.push((id, slot, c.owner));
            }
            assert_eq!(
                c.industry[slot].disabled_seasons, 0,
                "nothing is out of action on turn one"
            );
        }
    }
    assert!(
        on.iter().all(|&(_, slot, owner)| slot == 0 && owner != 0),
        "every switch that is on is wood, in an owned county: {on:?}"
    );
    assert_eq!(on.len(), 5, "five counties start owned and each has its forestry running");
}

// ------------------------------- the three numbers the county panels read back

/// `Pct` — `Tax_RecomputePreview`'s own rounding, which is truncation.
fn pct(v: i32, p: i32) -> i32 {
    v * p / 100
}

/// **`g_castleTaxBase`, written out rather than imported.** The multiplier for
/// castle types 0 … 5, immediates in `Tax_CollectAll`'s instruction stream
/// (`docs/kingdom.md` §10). Spelled here so that the assertion below does not
/// compute its expected value from the table it is checking — the trap
/// `docs/agents.md` records as *ablating a constant while computing your probe
/// from that same constant*.
const CASTLE_TAX_BASE: [i32; 6] = [320, 480, 560, 640, 720, 800];

/// The one moment on this machine where `+0xC0` is **not** the current
/// population's answer, named with its reason rather than filtered out.
///
/// It is the middle save of the battle triple, and the fixture's own name is
/// the explanation: `battle-during.sav` (the install calls the same game
/// `incombat.sav`) is taken with a battle open. County 2's population has
/// already fallen to 588 and the stored preview is still **245**, which is
/// `Pct(Pct(638, 480), 8)` — the answer for the population the county had
/// before the fighting. `battle-after.sav` stores **225** for the same county,
/// which *is* `Pct(Pct(588, 480), 8)`.
///
/// **That single disagreement is the argument for reading the byte instead of
/// recomputing it on load.** No recompute can produce 245; the original
/// restores a memory image, and `Tax_RecomputePreview` runs on a control or at
/// the end of a season, not on a load.
const PREVIEW_NOT_YET_REFRESHED: &[&str] = &["battle-during.sav", "incombat.sav"];

/// **`+0x0F` is `5 - taxRate` in every owned county of every save**, which is
/// `Tax_RecomputePreview` (`0x0044B80B`)'s second statement and is what says
/// the offset is the right one. It is a different field from `+0x0E`, which
/// carries the realm's empire term as well.
#[test]
fn the_local_tax_happiness_byte_is_five_minus_the_rate_in_every_save() {
    let mut checked = 0usize;
    for f in l2_testkit::saves!() {
        let s = &f.save;
        for id in 1..17usize {
            let base = COUNTY_BASE + (id * COUNTY_STRIDE) as u32;
            let Ok(owner) = s.u8_at(base + 0x05) else { continue };
            if owner == 0 {
                continue;
            }
            let rate = s.u8_at(base + 0xB9).unwrap() as i32;
            let local = s.i8_at(base + 0x0F).unwrap() as i32;
            assert_eq!(
                local,
                5 - rate,
                "{}: county {id} stores +0x0F = {local} at rate {rate}",
                f.label()
            );
            checked += 1;
        }
    }
    assert!(checked >= 5, "only {checked} counties were reached");
}

/// **The industry row's forecast is its own record's `+0x14`, in every save** —
/// and that is what says the `Industry` array starts at county `+0x294`.
///
/// `docs/records.json` had the array at `+0x290`. Under that base the word the
/// sidebar draws for commodity `c`, county `+0x2A8 + c*0x18`, is the head of
/// record `c + 1`, and stone's `+0x2F0` is past the end. Under `+0x294` it is
/// the last field of record `c` itself. **The two readings predict different
/// numbers**, and the original's own saves can say which:
///
/// `Industry_LabourEstimate` (`0x0044F318`) writes the word as
/// `min(limit, Pct(workers / divisor, ramp))`, zeroed first, and only when the
/// county is owned, has a `popBand`, and its resource limit is positive — which
/// for wood, iron and stone is the switch, the seam and a zero countdown. With
/// *Advanced Farming* off the ramp is a flat 80 and the limit is 999. So this
/// computes, from bytes of **record `c`** and the job record `c` draws on, the
/// number the word must hold, and requires the file to hold exactly it.
///
/// **It is not vacuous between the two bases.** Where record `c + 1`'s guards
/// and workers would give a different number, the file sides with record `c` —
/// siege-lastturn county 4 stores 74 at `+0x2A8`, which is 93 woodcutters × 80%,
/// not iron's 92 × 80% = 73 — and the test counts those cases and requires
/// some. Stone is switched off in every save on this machine, so its word is
/// only ever checked at zero; that is a limit of the corpus and is said here.
///
/// Weapons is excluded from the equality, as in
/// `crates/l2-kingdom/tests/industry_forecast.rs`: its limit is a realm-wide
/// share of wood and iron. It is still checked to be zero when a guard fails
/// and never above the unlimited figure.
///
/// Every offset and constant below is a literal, so ablating one in the
/// importer cannot move this expectation with it. **Ablations, both run:**
/// putting `INDUSTRY_BASE` back to `0x290` (offsets unchanged) turns this red
/// at the import half and `every_industrys_resource_byte_agrees…` red with it;
/// dropping the `next_season` line from `Scenario::kingdom` turns the
/// import-reaches-the-kingdom half red.
#[test]
fn every_saved_industry_forecast_is_what_its_own_records_workers_make() {
    // Wood, iron, weapons, stone: the job slot each draws on and
    // `Industry_Produce`'s divisor. `County_RefreshEstimates`' four calls.
    const JOB: [u32; 4] = [6, 4, 7, 5];
    const DIVISOR: [i32; 4] = [1, 1, 4, 2];
    const WEAPONS: u32 = 2;

    let mut checked = 0usize;
    let mut non_zero = 0usize;
    let mut discriminating = 0usize;
    for f in l2_testkit::saves!() {
        let s = &f.save;
        assert_eq!(
            s.globals().unwrap().opt_advanced_farming,
            0,
            "{}: Advanced Farming is on, so the flat 80 below is not the rule",
            f.label()
        );
        let scenario = Scenario::from_save(s).expect("import");
        let kingdom = scenario.kingdom(1);
        for id in 1..17usize {
            let base = COUNTY_BASE + (id * COUNTY_STRIDE) as u32;
            let Ok(owner) = s.u8_at(base + 0x05) else { continue };
            let band = s.u8_at(base + 0xB8).unwrap();
            // What `Industry_LabourEstimate` writes for record `r`, from record
            // `r`'s bytes. `None` for a record that does not exist.
            let made = |r: u32| -> Option<i32> {
                if r > 3 {
                    return None;
                }
                let rec = base + 0x294 + r * 0x18;
                let has = s.u8_at(rec + 0x01).unwrap() != 0;
                let countdown = s.u8_at(rec + 0x02).unwrap();
                let on = s.u8_at(rec + 0x03).unwrap() != 0;
                let workers = s.i32_at(base + 0xC4 + JOB[r as usize] * 0x0C).unwrap();
                Some(if owner == 0 || band == 0 || !has || !on || countdown != 0 {
                    0
                } else {
                    ((workers / DIVISOR[r as usize]) * 80 / 100).min(999)
                })
            };
            for c in 0..4u32 {
                let stored = s.i32_at(base + 0x2A8 + c * 0x18).unwrap();
                let own = made(c).unwrap();
                if c == WEAPONS {
                    assert!(
                        (own == 0 && stored == 0) || (own != 0 && (0..=own).contains(&stored)),
                        "{}: county {id} weapons stores {stored}, its own record allows {own}",
                        f.label()
                    );
                } else {
                    assert_eq!(
                        stored,
                        own,
                        "{}: county {id} commodity {c} stores {stored} at +{:#X}, and record \
                         {c}'s own workers make {own}",
                        f.label(),
                        0x2A8 + c * 0x18
                    );
                    if made(c + 1) != Some(own) {
                        discriminating += 1;
                    }
                }
                // The importer carries it, and the kingdom a loaded game runs on
                // receives it — rather than `Industry::new()`'s zero.
                if let Some(state) = &scenario.counties[id] {
                    assert_eq!(state.industry[c as usize].next_season, stored, "{}", f.label());
                    assert_eq!(
                        kingdom.counties[id].industry[c as usize].next_season,
                        stored,
                        "{}: county {id} commodity {c} was imported and did not reach the kingdom",
                        f.label()
                    );
                }
                checked += 1;
                non_zero += usize::from(stored != 0);
            }
        }
    }
    assert!(checked >= 4 * 14, "only {checked} forecasts were reached");
    assert!(non_zero > 0, "every stored forecast is zero, so nothing was compared");
    assert!(
        discriminating > 0,
        "no county anywhere tells record c from record c + 1, so this cannot tell the bases apart"
    );
    eprintln!("{checked} forecasts, {non_zero} non-zero, {discriminating} discriminating");
}

/// **The tax panel's *People pay* line, against the original's own answer.**
///
/// `+0xC0` is `Pct(Pct(population, castleBase), taxRate)` — the third statement
/// of `Tax_RecomputePreview` — and the saves on this machine carry rates 2, 3,
/// 6 and 8, so the arithmetic can be checked against a number the original
/// wrote rather than against ourselves. `docs/plan.md` §2.5 says every county in
/// every fixture sits at rate 0; that is true of the England fixture and false
/// of the turn pair and the six siege saves.
///
/// It is still true of any rate above 19, where `g_taxHappinessOther` starts to
/// bite — so this promotes the *preview*, not the empire term.
#[test]
fn the_tax_preview_byte_is_the_arithmetic_we_implement() {
    let mut agreed = 0usize;
    for f in l2_testkit::saves!() {
        if PREVIEW_NOT_YET_REFRESHED.contains(&f.name.as_str()) {
            continue;
        }
        let s = &f.save;
        for id in 1..17usize {
            let base = COUNTY_BASE + (id * COUNTY_STRIDE) as u32;
            let Ok(owner) = s.u8_at(base + 0x05) else { continue };
            if owner == 0 {
                continue;
            }
            let rate = s.u8_at(base + 0xB9).unwrap() as i32;
            if rate == 0 {
                continue;
            }
            let pop = s.i32_at(base + 0x24).unwrap();
            let castle = s.u8_at(base + 0x1C0).unwrap() as usize;
            let shown = s.i32_at(base + 0xC0).unwrap();
            let base_mult = CASTLE_TAX_BASE[castle.min(5)];
            assert_eq!(
                pct(pct(pop, base_mult), rate),
                shown,
                "{}: county {id}, {pop} people at rate {rate} behind castle {castle}",
                f.label()
            );
            agreed += 1;
        }
    }
    if agreed == 0 {
        l2_testkit::skip!(
            "no reachable save carries a county at a non-zero tax rate, so there is \
             nothing to check the preview against"
        );
    }
}

// ------------------------------------ what docs/stored-fields.json found dropped

/// **`+0x258` is `+0x268 − +0x26C − herdEaten` in every county of every save** —
/// the cattle row's *"Overall change"*, calf births expected, cow deaths expected,
/// and what the people ate (`docs/decisions.md` C128 for why the eating is in it).
///
/// Three fields that were dropped together by the importer, and one relation
/// that pins all three offsets at once: a wrong offset for any of them would have
/// to land on a word that happens to close this sum in every county of every
/// save. It is not vacuous — the England turn-one fixture's ten neutral counties
/// eat thirteen head each, so the eating term is exercised, and births differ
/// from deaths almost everywhere.
///
/// `tests/stored_fields.rs` is what holds the kingdom to these bytes; this is
/// what says the bytes are the fields.
#[test]
fn every_saved_cattle_forecast_is_births_less_deaths_less_what_was_eaten() {
    let (mut checked, mut eaten_counted) = (0usize, 0usize);
    for f in l2_testkit::saves!() {
        let s = &f.save;
        for id in 1..17usize {
            let base = COUNTY_BASE + (id * COUNTY_STRIDE) as u32;
            if s.i32_at(base + 0x24).unwrap() == 0 {
                continue;
            }
            let change = s.i32_at(base + 0x258).unwrap();
            let births = s.i32_at(base + 0x268).unwrap();
            let deaths = s.i32_at(base + 0x26C).unwrap();
            let eaten = s.i32_at(base + 0x17C).unwrap();
            assert_eq!(
                change,
                births - deaths - eaten,
                "{}: county {id} stores +0x258 {change}, and +0x268 {births} − +0x26C {deaths} − \
                 +0x17C {eaten} is {}",
                f.label(),
                births - deaths - eaten
            );
            checked += 1;
            eaten_counted += usize::from(eaten != 0);
        }
    }
    assert!(checked >= 14, "only {checked} counties reached");
    assert!(eaten_counted > 0, "no county anywhere ate cattle, so the eating term was never tested");
}

/// **`+0x22C` is `Grain_LabourEstimate`'s tail in every county of every save**:
/// `−sown − eaten` when the season is Spring, `harvest − eaten` in Winter,
/// `−eaten` otherwise, with `+0x230` the sowing and `crop[2]` the harvest.
///
/// **What the corpus can and cannot settle, said beside the assertion.** Every
/// save on this machine stores `+0x230 == 0` and `crop[2] == 0` in every county,
/// so the sowing and harvest arms are only checked at zero, and the tail's
/// `season` argument — `l2_kingdom::land::grain_preview` reads it as next season
/// — cannot be told from this season here. What *is* settled is that `+0x22C`
/// is minus the county's grain eaten wherever those arms are empty, in five
/// counties where that is not zero (`old_turn.sav` county 2 stores −73).
#[test]
fn every_saved_grain_forecast_is_its_seasons_tail() {
    let (mut checked, mut non_zero, mut arms) = (0usize, 0usize, 0usize);
    for f in l2_testkit::saves!() {
        let s = &f.save;
        let season_next = s.globals().unwrap().season_next;
        for id in 1..17usize {
            let base = COUNTY_BASE + (id * COUNTY_STRIDE) as u32;
            if s.i32_at(base + 0x24).unwrap() == 0 {
                continue;
            }
            let change = s.i32_at(base + 0x22C).unwrap();
            let sown = s.i32_at(base + 0x230).unwrap();
            let harvest = s.i32_at(base + 0x248).unwrap();
            let eaten = s.i32_at(base + 0x178).unwrap();
            let tail = match season_next {
                1 => -sown - eaten,
                4 => harvest - eaten,
                _ => -eaten,
            };
            assert_eq!(
                change,
                tail,
                "{}: county {id} stores +0x22C {change}; next season {season_next}, sown {sown}, \
                 harvest {harvest}, eaten {eaten}",
                f.label()
            );
            checked += 1;
            non_zero += usize::from(change != 0);
            arms += usize::from(sown != 0 || harvest != 0);
        }
    }
    assert!(checked >= 14, "only {checked} counties reached");
    assert!(non_zero > 0, "every stored grain forecast is zero, so nothing was compared");
    eprintln!("{checked} grain forecasts, {non_zero} non-zero, {arms} with a sowing or a harvest");
}

/// **`+0x18` is `+0x1C / g_turnCount` in every county of every save** —
/// `Happiness_UpdateAll` (`0x0044BAEA`) banks the season's happiness into the sum
/// and divides by the turn count, into a signed byte.
///
/// The importer used to set both to this season's happiness, which this test
/// measures the cost of: it counts the counties where the stored average is not
/// the current happiness, which is every county past turn one whose mood has
/// moved — `siege-aftersie.sav` county 2 stores 54 and is at 95 today.
#[test]
fn the_happiness_average_is_the_running_sum_over_the_turn_count_in_every_save() {
    let (mut checked, mut differs) = (0usize, 0usize);
    for f in l2_testkit::saves!() {
        let s = &f.save;
        let turns = s.globals().unwrap().turn_count;
        assert!(turns > 0, "{}: turn count {turns}", f.label());
        for id in 1..17usize {
            let base = COUNTY_BASE + (id * COUNTY_STRIDE) as u32;
            if s.i32_at(base + 0x24).unwrap() == 0 {
                continue;
            }
            let avg = s.i8_at(base + 0x18).unwrap() as i32;
            let sum = s.i32_at(base + 0x1C).unwrap();
            assert_eq!(avg, (sum / turns) as i8 as i32, "{}: county {id}, sum {sum} over {turns} turns", f.label());
            checked += 1;
            differs += usize::from(avg != s.i8_at(base + 0x0C).unwrap() as i32);
        }
    }
    assert!(checked >= 14, "only {checked} counties reached");
    assert!(differs > 0, "the average equals the current happiness everywhere, so the old import passed too");
}

/// **`+0x5B` is set exactly where `+0x2C` reaches 6** — `Population_UpdateAll`
/// writes the change percentage and then attributes it only when it is at least
/// six (`docs/kingdom.md` §1.2). Both bytes are written in the same pass, so the
/// relation holds whatever happened to the population afterwards, which is why
/// this does not also check the percentage against the population.
///
/// The six is written as a literal rather than read from
/// `l2_kingdom::county::CHANGE_REASON_MIN_PCT`: a probe computed from the constant
/// under test is `docs/agents.md`'s first way to ablate wrongly.
#[test]
fn a_population_change_reason_is_recorded_exactly_where_the_change_reaches_six_percent() {
    let (mut checked, mut reasons) = (0usize, 0usize);
    for f in l2_testkit::saves!() {
        let s = &f.save;
        for id in 1..17usize {
            let base = COUNTY_BASE + (id * COUNTY_STRIDE) as u32;
            if s.i32_at(base + 0x24).unwrap() == 0 {
                continue;
            }
            let pct = s.i32_at(base + 0x2C).unwrap();
            let reason = s.u8_at(base + 0x5B).unwrap();
            assert_eq!(reason != 0, pct >= 6, "{}: county {id}, change {pct}%, reason {reason}", f.label());
            assert!(reason <= 4, "{}: county {id}, reason {reason} names no L2.eng group 65 string", f.label());
            checked += 1;
            reasons += usize::from(reason != 0);
        }
    }
    assert!(checked >= 14 && reasons > 0, "{checked} counties, {reasons} with a reason");
}

/// **`+0x21` is set only in a county below thirty happiness**, which is what
/// identifies it as `Unrest_UpdateAll`'s warning latch (`0x0044AA41`: set when
/// happiness is under `0x1E` and the byte is clear) — the flag
/// `l2_kingdom::county::County::unrest_warned` described without an offset.
#[test]
fn the_unrest_warning_latch_is_set_only_in_a_county_below_thirty_happiness() {
    let mut set = 0usize;
    for f in l2_testkit::saves!() {
        let s = &f.save;
        for id in 1..17usize {
            let base = COUNTY_BASE + (id * COUNTY_STRIDE) as u32;
            if s.u8_at(base + 0x21).unwrap() == 0 {
                continue;
            }
            let happiness = s.i8_at(base + 0x0C).unwrap();
            assert!(happiness < 30, "{}: county {id} is warned at happiness {happiness}", f.label());
            set += 1;
        }
    }
    assert!(set > 0, "no save carries the latch, so nothing identified it");
}

/// **The three fields the county panels draw and the importer dropped.**
///
/// `+0x0F` and `+0xC0` are the tax panel's *This county* and *People pay*;
/// `+0x10` is the health term the ration panel draws beside the band. All three
/// arrived as `County::new()`'s zero on every loaded game, so a player opening
/// the tax panel was told *"People pay 0 crowns"* whatever the rate and read
/// `( 0 ☺ )` where the original shows `( +5 ☺ )` at rate 0.
///
/// **Ablation, run:** delete `c.tax_shown = *tax_shown;` from
/// `Scenario::apply_counties` and the third clause fails on every save; delete
/// `c.d_hap_tax_local = *d_hap_tax_local;` and the first fails on England,
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
