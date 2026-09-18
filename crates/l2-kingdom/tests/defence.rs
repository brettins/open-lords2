//! `County_FindDefendingArmy` (`0x0046D42C`) was modelled for months as *"the
//! lowest-numbered army of the county's owner standing in the county"*, with the
//! function itself unread. When it was read, **both halves were wrong**: the
//! scope is a 4×4 tile block around the county *town*, and the tie-break is the
//! largest army.
//!
//! That is `docs/decisions.md` C12 (a test that cannot fail) turned around: the
//! old reading and the new one are both run here, on the same bytes, and they
//! are asserted to return *different* answers.

use l2_formats::save::Save;
use l2_kingdom::conquest;
use l2_kingdom::county::{County, MAX_COUNTIES};
use l2_kingdom::unit::{Unit, UnitKind, Units};

/// `g_units`, `0x0052F0B0`, stride `0x1A4`, slots 1 … 150 — `docs/armies.md` §1.
const UNIT_BASE: u32 = 0x0052_F0B0;
const UNIT_STRIDE: u32 = 0x1A4;
const UNIT_SLOTS: u32 = 150;

struct SaveUnit {
    slot: u32,
    owner: u8,
    kind: u8,
    x: u8,
    y: u8,
    county: u8,
    men: i32,
}

fn units_of(save: &Save) -> Vec<SaveUnit> {
    (1..=UNIT_SLOTS)
        .filter_map(|slot| {
            let base = UNIT_BASE + slot * UNIT_STRIDE;
            let owner = save.u8_at(base).ok()?;
            if owner == 0 {
                return None; // a free slot
            }
            Some(SaveUnit {
                slot,
                owner,
                kind: save.u8_at(base + 0x08).ok()?,
                x: save.u8_at(base + 0x0A).ok()?,
                y: save.u8_at(base + 0x0B).ok()?,
                county: save.u8_at(base + 0x10).ok()?,
                men: save.i32_at(base + 0x168).ok()?,
            })
        })
        .collect()
}

#[test]
fn battle_before_holds_a_county_whose_army_is_too_far_from_its_town_to_defend_it() {
    let save: Save = l2_testkit::fixture!("battle-before.sav");

    let saved = save.county(2).expect("county 2");
    assert!(saved.is_county(), "county 2 is a real county");
    assert_ne!(saved.owner, 0, "county 2 is owned");
    assert_eq!(
        (saved.anchor_x, saved.anchor_y),
        (31, 50),
        "county 2's town anchor — the fixture fingerprint this test rests on"
    );

    let owner = saved.owner;
    let army: Vec<_> =
        units_of(&save).into_iter().filter(|u| u.kind == 1 && u.owner == owner).collect();
    assert_eq!(army.len(), 1, "one army belongs to county 2's owner");
    let army = &army[0];
    assert_eq!((army.x, army.y), (30, 46), "and it stands four rows north of the town");
    assert_eq!(army.county, 2, "inside county 2 all the same — which is why the old reading hit");

    // **What that army is.** `+0x198` is the garrison link, and it
    // holds 2: this is county 2's castle garrison, sitting on the castle tile
    // (`docs/armies.md` §8b.3) four rows from the town. So the county's own
    // defenders are outside the window that decides who defends it — which is
    // the point, and is why the old reading looked plausible.
    let base = UNIT_BASE + army.slot * UNIT_STRIDE;
    assert_eq!(save.u8_at(base + 0x198).unwrap(), 2, "county 2's castle garrison");
    // County `+0x1BC` is the link back. Stride 0x300 from `COUNTY_BASE`.
    let county_base = l2_formats::save::COUNTY_BASE + 2 * l2_formats::save::COUNTY_STRIDE as u32;
    assert_eq!(save.i32_at(county_base + 0x1BC).unwrap(), army.slot as i32, "and the link back");

    let mut counties: [County; MAX_COUNTIES] = core::array::from_fn(|_| County::new());
    counties[2].owner = owner;
    counties[2].anchor_x = saved.anchor_x;
    counties[2].anchor_y = saved.anchor_y;

    let mut units = Units::new();
    let mut u = Unit::new(UnitKind::Army, army.owner, army.x, army.y);
    u.men = army.men;
    u.county = army.county;
    let slot = units.spawn(u).expect("a unit array with room");

    let old = units
        .iter()
        .find(|(_, u)| u.kind == UnitKind::Army && u.owner == owner && u.county == 2)
        .map(|(i, _)| i);
    assert_eq!(old, Some(slot), "the old reading finds the army (g_units slot {})", army.slot);

    assert_eq!(
        conquest::find_defender(&units, &counties, 2),
        None,
        "County_FindDefendingArmy scans (29..=32, 48..=51) and (30, 46) is not in it"
    );
}

#[test]
fn the_same_army_standing_on_the_town_does_defend_it() {
    let save: Save = l2_testkit::fixture!("battle-before.sav");
    let saved = save.county(2).expect("county 2");

    let mut counties: [County; MAX_COUNTIES] = core::array::from_fn(|_| County::new());
    counties[2].owner = saved.owner;
    counties[2].anchor_x = saved.anchor_x;
    counties[2].anchor_y = saved.anchor_y;

    let mut units = Units::new();
    let mut u = Unit::new(UnitKind::Army, saved.owner, saved.anchor_x, saved.anchor_y);
    u.men = 300;
    u.county = 2;
    let slot = units.spawn(u).expect("a unit array with room");

    assert_eq!(conquest::find_defender(&units, &counties, 2), Some(slot));
}
