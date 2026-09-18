#![allow(unused_imports)]
use super::*;
use super::counts::*;
use super::painting::*;
use super::economy::*;
use l2_kingdom::field::FieldType;
use l2_kingdom::{Kingdom, MAX_FIELDS};
use l2_scenario::Scenario;
use l2_testkit::england;


/// `Herd_UpdateCrowding` (`0x0044D913`) writes one of `0x13 … 0x16` onto every
/// pasture tile of a county, chosen by `herd / fieldsCattle` banded at 11 and
/// 21, and `FUN_004071A0` draws a different number of animals for each. So the
/// terrain byte on a pasture tile is the herd meter,
/// stored where the renderer can see it.
///
/// This is the same shape as the ladder test above and earns the same `[V]`:
#[test]
fn every_county_s_pasture_carries_the_picture_its_herd_calls_for() {
    let save = england!();
    let kingdom = Scenario::from_save(&save).expect("import").kingdom(1);
    let tables = l2_kingdom::tables::Tables::DEFAULT;

    let mut seen = std::collections::BTreeSet::new();
    let mut tiles = 0;
    for id in 1..=kingdom.county_count {
        let c = &kingdom.counties[id];
        let want = l2_kingdom::land::herd_graphic(&tables, c.herd, c.fields_cattle);
        for slot in 0..MAX_FIELDS {
            let Some(tile) = c.field_tile(slot) else { continue };
            let got = kingdom.campaign.map.terrain[tile];
            if l2_kingdom::field::classify(got) != FieldType::Pasture {
                continue;
            }
            assert_eq!(
                got, want,
                "county {id}: {} head on {} fields is density {}, so the game drew {want:#04x} \
                 and we would have drawn {got:#04x}",
                c.herd,
                c.fields_cattle,
                c.herd / c.fields_cattle.max(1),
            );
            seen.insert(got);
            tiles += 1;
        }
    }
    assert_eq!(tiles, 107, "the England position has 107 pasture tiles");
    assert_eq!(
        seen,
        [0x14u8, 0x15, 0x16].into_iter().collect(),
        "three of the four stocking states occur at turn one",
    );
}

#[test]
fn the_map_merges_the_top_two_crowding_bands_and_the_meter_does_not() {
    let save = england!();
    let kingdom = Scenario::from_save(&save).expect("import").kingdom(1);
    let tables = l2_kingdom::tables::Tables::DEFAULT;
    let c = &kingdom.counties[1];
    assert_eq!(c.fields_cattle, 1, "county 1 grazes everything on one field");
    assert_eq!(c.herd_crowding, 40, "which is the top meter band");
    assert_eq!(
        l2_kingdom::land::herd_graphic(&tables, c.herd, c.fields_cattle),
        0x16,
        "and the top *picture*, which is also band 30's",
    );
    assert_eq!(l2_kingdom::land::herd_crowding(&tables, 25, 1), 30);
    assert_eq!(l2_kingdom::land::herd_crowding(&tables, 250, 1), 40);
    assert_eq!(l2_kingdom::land::herd_graphic(&tables, 25, 1), 0x16);
    assert_eq!(l2_kingdom::land::herd_graphic(&tables, 250, 1), 0x16);
}

#[test]
fn the_map_empties_when_the_herd_does() {
    let save = england!();
    let mut kingdom = Scenario::from_save(&save).expect("import").kingdom(1);
    let id = 2; // eight pasture fields, 67 head, drawn 0x14
    let pasture: Vec<usize> = (0..MAX_FIELDS)
        .filter_map(|s| kingdom.counties[id].field_tile(s))
        .filter(|&t| {
            l2_kingdom::field::classify(kingdom.campaign.map.terrain[t]) == FieldType::Pasture
        })
        .collect();
    assert_eq!(pasture.len(), 8, "county 2's eight pastures");
    assert!(pasture.iter().all(|&t| kingdom.campaign.map.terrain[t] == 0x14));

    kingdom.counties[id].herd = 0;
    kingdom.advance_season();

    for &t in &pasture {
        assert_eq!(
            kingdom.campaign.map.terrain[t], 0x13,
            "an empty herd leaves bare pasture, and l2_view draws no animals on it",
        );
    }
}

