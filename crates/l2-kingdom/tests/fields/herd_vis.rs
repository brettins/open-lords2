#![allow(unused_imports)]
use super::*;
use super::counts::*;
use super::painting::*;
use super::economy::*;
use l2_kingdom::field::FieldType;
use l2_kingdom::{Kingdom, MAX_FIELDS};
use l2_scenario::Scenario;
use l2_testkit::england;

// ------------------------------------------------------- the cattle on the map

/// **The pasture picture is the herd count**.
///
/// `Herd_UpdateCrowding` (`0x0044D913`) writes one of `0x13 … 0x16` onto every
/// pasture tile of a county, chosen by `herd / fieldsCattle` banded at 11 and
/// 21, and `FUN_004071A0` draws a different number of animals for each. So the
/// terrain byte on a pasture tile is the herd meter,
/// stored where the renderer can see it.
///
/// This is the same shape as the ladder test above and earns the same `[V]`:
/// **the England turn-one save carries both halves and neither is ours.** The
/// file holds `county.herd`, it holds the twenty field tiles' terrain bytes,
/// and `l2_kingdom::land::herd_graphic` has to reproduce the second from the
/// first. Nothing in this tree can make that come out right by agreeing with
/// itself.
///
/// The position exercises **three** of the four states — county 1 grazes 74
/// head on one field (`0x16`), the eight-field counties at 67 head sit at
/// density 8 (`0x14`) and the 93/101/110-head ones at 11 and 12 (`0x15`). The
/// empty-herd state `0x13` does not occur at turn one and is covered by
/// [`the_map_empties_when_the_herd_does`] instead.
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

/// **The graphic is not the crowding meter**.
/// difference matters.**
///
/// `herd_crowding` bands at 11, 21 and 31 into 10/20/30/40; `herd_graphic`
/// bands at 11 and 21 only. County 1 grazes 74 head on one field — density 74,
/// which is meter band **40** and graphic `0x16`, the *third* of three. A
/// renderer that indexed three pictures by `herd_crowding / 10` would run off
/// the end of its table on the very first county of the shipped position.
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
    // The two bands that share a picture, stated directly.
    assert_eq!(l2_kingdom::land::herd_crowding(&tables, 25, 1), 30);
    assert_eq!(l2_kingdom::land::herd_crowding(&tables, 250, 1), 40);
    assert_eq!(l2_kingdom::land::herd_graphic(&tables, 25, 1), 0x16);
    assert_eq!(l2_kingdom::land::herd_graphic(&tables, 250, 1), 0x16);
}

/// **A county that loses its herd loses its cattle**
/// what does it.
///
/// `docs/agents.md`: *"a field is only tested if something a test reads was
/// written by something the game runs."* So this does not set a terrain byte
/// and read it back — it kills the herd and runs [`Kingdom::advance_season`]
/// pasture on the map has to follow. `Herd_UpdateCrowding` is called from
/// `Herd_SeasonTick`'s last line and that is the road being travelled.
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
    // The other half of that claim — that `0x13` draws nothing — belongs to
    // `l2-view` and is asserted in `crates/l2-game/tests/screens_*.rs`, which is
    // the lowest crate that can see both sides. This one must not reach for
    // `l2-view`: nothing in the simulation may depend on the renderer.
}

