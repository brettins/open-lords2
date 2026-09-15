#![allow(unused_imports)]
use super::*;
use super::structure::*;
use super::realms_and_counties::*;
use l2_formats::save::{Save, SaveError, COUNTY_RECORDS, NEIGHBOUR_SLOTS, REALM_RECORDS};
use l2_testkit::{executable, saves, skip, SaveFile};

/// `+0x0C` is `(y * 64 + x) * 8`, which is redundant with `+0x0A`/`+0x0B` — so
/// the only way it can agree on every occupied slot of every save is if the base
/// and the `0x1A4` stride are both right. A wrong stride would shift the pair
/// and the offset by different amounts and the equality would fail on the first
/// unit of the first file.
#[test]
fn every_units_tile_offset_agrees_with_its_coordinates_in_every_save() {
    let saves = saves!();
    let mut checked = 0;
    for s in &saves {
        for u in s.save.units().unwrap().iter().filter(|u| u.is_live()) {
            assert!(
                u.tile_offset_agrees(),
                "{}: unit {} is at ({}, {}) but +0x0C is {:#x}",
                s.label(),
                u.index,
                u.x,
                u.y,
                u.tile_offset
            );
            checked += 1;
        }
    }
    eprintln!("tile offsets agreed on {checked} units across {} saves", saves.len());
    assert!(checked > 0, "no save offered a single unit");
}

#[test]
fn every_live_unit_is_one_of_the_four_types_and_owned_by_somebody() {
    let saves = saves!();
    for s in &saves {
        let county_count = s.save.globals().unwrap().county_count;
        for u in s.save.units().unwrap().iter().filter(|u| u.is_live()) {
            let at = format!("{} unit {}", s.label(), u.index);
            assert!((1..=4).contains(&u.kind), "{at}: type byte {}", u.kind);
            assert!((1..=6).contains(&u.owner), "{at}: owner {}", u.owner);
            assert!(u.x < 64 && u.y < 64, "{at}: at ({}, {})", u.x, u.y);
            assert!(u.path_len as usize <= l2_formats::save::UNIT_PATH_STEPS, "{at}: path");
            assert!(u.county as i32 <= county_count, "{at}: county {}", u.county);
            assert!(u.dest_county as i32 <= county_count, "{at}: dest {}", u.dest_county);
            assert!(u.garrison_county as i32 <= county_count, "{at}: garrison");
            assert!(u.besieging_county as i32 <= county_count, "{at}: besieging");
            assert!((0..=5).contains(&u.starvation), "{at}: starvation {}", u.starvation);
        }
    }
}

#[test]
fn the_merchants_are_the_first_slots_and_there_are_as_many_as_the_counter_says() {
    let saves = saves!();
    for s in &saves {
        let counted = s.save.globals().unwrap().merchant_count;
        let units = s.save.units().unwrap();
        let merchants: Vec<usize> =
            units.iter().filter(|u| u.is_live() && u.kind == 3).map(|u| u.index).collect();
        assert_eq!(merchants.len() as i32, counted, "{}: g_merchantCount", s.label());
        let expected: Vec<usize> = (1..=merchants.len()).collect();
        assert_eq!(merchants, expected, "{}: merchants are not slots 1..n", s.label());
    }
}

#[test]
fn the_merchant_route_table_is_well_formed_in_every_save() {
    let saves = saves!();
    for s in &saves {
        let county_count = s.save.globals().unwrap().county_count as u8;
        let rows = s.save.merchant_routes().unwrap();
        let start = s.save.merchant_start_counties().unwrap();
        for (r, row) in rows.iter().enumerate() {
            let live: Vec<u8> = row.iter().copied().take_while(|&c| c != 0).collect();
            let at = format!("{} route {}", s.label(), r + 1);
            assert_eq!(
                row[live.len()..].iter().filter(|&&c| c != 0).count(),
                0,
                "{at}: a zero with counties after it"
            );
            for &c in &live {
                assert!(c <= county_count, "{at}: county {c}, and the map has {county_count}");
            }
            let mut sorted = live.clone();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(sorted.len(), live.len(), "{at}: a county appears twice");
        }
        let spawned = start.iter().take_while(|&&c| c != 0).count();
        for &c in &start[..spawned] {
            assert!(c <= county_count, "{}: start county {c}", s.label());
        }
        assert_eq!(
            spawned as i32,
            s.save.globals().unwrap().merchant_count,
            "{}: a merchant per start county before the first zero",
            s.label()
        );
    }
}

/// **`+0x167` on a merchant is where it was born, not where it is.**
///
/// `Merchant_SpawnAll` writes `g_merchantStartCounty[i]` into it once and
/// nothing updates it afterwards, so merchant *n* carries start county *n* for
/// the life of the game while its `+0x10` follows it around the map. Worth an
/// assertion because the same byte is an army's county-defence mark, and reading
/// a merchant's birthplace as a defence mark would make every merchant look like
/// a levy.
#[test]
fn a_merchants_0x167_is_its_start_county_however_far_it_has_walked() {
    let saves = saves!();
    let mut moved = 0;
    for s in &saves {
        let start = s.save.merchant_start_counties().unwrap();
        for u in s.save.units().unwrap().iter().filter(|u| u.is_live() && u.kind == 3) {
            assert_eq!(
                u.role,
                start[u.index - 1],
                "{}: merchant {} carries {} and started in {}",
                s.label(),
                u.index,
                u.role,
                start[u.index - 1]
            );
            if u.county != u.role {
                moved += 1;
            }
        }
    }
    assert!(moved > 0, "no merchant in any save had left its start county");
    eprintln!("{moved} merchants stood outside the county +0x167 names");
}

/// **The three minimap rating bytes, `+0x01`, `+0x02` and `+0x03`, named from
/// the saved games.**
///
/// `FUN_00451BBA` recomputes them on every minimap draw and `Minimap_DrawOverlay`
/// reads them. `docs/screens.md` §3.2 used to call them `+0x0B1`, `+0x0B2` and
/// `+0x0B3`, which is the literal `0x0053F9B3` in the disassembly mistaken for an
/// offset; they are three of the five bytes `Sync_CompareState` skips, so they
/// are interface state and a save holds whatever the last draw left there.
///
/// * **`+0x02` is the food rating and it is binary** — 0 when the county did not
///   achieve the ration it was asked for and 6, off the end of the six-entry
///   ramp, when it did. Never anything between.
///
/// * **`+0x03` is the labour rating and it has three values** — 0 short of farm
///   workers, 5 carrying slack, 6 neither.
///
/// * **`+0x01` is `happiness / 20`.** Checked only in saves where the bands have
/// been computed at all: a game whose minimap overlay has all
///   three bytes zero, which the England turn-one fixture is.
#[test]
fn the_minimap_rating_bytes_are_food_labour_and_happiness_over_twenty() {
    use l2_formats::save::{COUNTY_BASE, COUNTY_STRIDE};
    let saves = saves!();
    let mut checked = 0;
    let mut computed = 0;
    for s in &saves {
        let mut any = false;
        let mut happiness_bands = Vec::new();
        for i in 0..COUNTY_RECORDS {
            let county = s.save.county(i).unwrap();
            if !county.is_county() {
                continue;
            }
            let base = COUNTY_BASE + (i * COUNTY_STRIDE) as u32;
            let (b1, b2, b3) = (
                s.save.u8_at(base + 1).unwrap(),
                s.save.u8_at(base + 2).unwrap(),
                s.save.u8_at(base + 3).unwrap(),
            );
            assert!(
                matches!(b2, 0 | 6),
                "{}: county {i} food band {b2} is neither 0 nor 6",
                s.label()
            );
            assert!(
                matches!(b3, 0 | 5 | 6),
                "{}: county {i} labour band {b3} is not 0, 5 or 6",
                s.label()
            );
            any |= b1 != 0 || b2 != 0 || b3 != 0;
            happiness_bands.push((i, b1, county.happiness));
            checked += 1;
        }
        if !any {
            continue;
        }
        computed += 1;
        for (i, band, happiness) in happiness_bands {
            assert_eq!(
                band as i32,
                happiness as i32 / 20,
                "{}: county {i} band {band} against happiness {happiness}",
                s.label()
            );
        }
    }
    assert!(computed >= 2, "at least two saves must have the bands computed");
    eprintln!("minimap bands: {checked} counties, {computed} saves with the bands computed");
}

