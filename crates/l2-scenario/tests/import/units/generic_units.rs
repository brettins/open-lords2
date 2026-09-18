#![allow(unused_imports)]
use super::*;
use super::trade_units::*;
use super::mercenaries::*;
use super::*;
use super::county::*;
use super::economy::*;
use l2_formats::save::{Layout, Save, COUNTY_BASE, COUNTY_STRIDE};
use l2_scenario::{ImportError, Scenario, STARTING_HEALTH_METER};
use l2_testkit::{saves, SaveFile};

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
            assert_eq!(ours.move_allowance, f.move_allowance as i32, "{at}: allowance");
            for t in 0..7 {
                assert_eq!(ours.troops[t], f.troops[t] as i32, "{at}: troop {t}");
            }
            assert_eq!(kingdom.campaign.units.get(*slot), Some(ours), "{at}: in the kingdom");
            checked += 1;
        }
    }
    eprintln!("{checked} units imported across {} saves", saves.len());
    assert!(checked > 6, "only {checked} units reached; the battle saves add armies");
}

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

/// The relation has two halves in the original — county `+0x1BC` names the unit,
/// unit `+0x198` names the county — and this importer reads only the unit's,
/// then derives the county's. It had not derived it at all: `County::new` seeds
/// `garrison_unit: 0`, nothing overwrote it, and **every loaded game arrived
/// with no castle garrisoned**. `conquest`'s ownership test, `divide`, `siege`
/// and the campaign map's castle flag are all downstream of that field
/// flag is what exposed it. C59.
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

