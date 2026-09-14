#![allow(unused_imports)]
use super::*;
use super::painting::*;
use super::herd_vis::*;
use super::economy::*;
use l2_kingdom::field::FieldType;
use l2_kingdom::{Kingdom, MAX_FIELDS};
use l2_scenario::Scenario;
use l2_testkit::england;

/// The ladder, against the save's own arithmetic, on every county.
///
/// The `[V]` this earns is precise: 168 field tiles across fourteen counties,
/// three counts each, every one of them the file's.
#[test]
fn the_terrain_ladder_reproduces_every_county_s_stored_field_counts() {
    let save = england!();
    let scenario = Scenario::from_save(&save).expect("import");
    let kingdom = scenario.kingdom(1);

    let mut tiles_seen = 0;
    for id in scenario.county_ids() {
        let stored = scenario.counties[id].as_ref().expect("a county");
        let ours = &kingdom.counties[id];
        tiles_seen += ours.field_slots_used();
        assert_eq!(ours.fields_fallow, stored.fields_fallow, "county {id} fallow");
        assert_eq!(ours.fields_cattle, stored.fields_cattle, "county {id} pasture");
        assert_eq!(ours.fields_grain, stored.fields_grain, "county {id} grain");
    }
    assert_eq!(tiles_seen, 168, "the England map's fourteen counties own 168 field tiles");
}

/// **The premise.** Nobody starts with a grain field, so the brush is the only
/// way the player's economy ever begins.
///
/// A scenario value, not an invariant (`docs/decisions.md` C23): it is a fact
/// about how this game opens, and it is asserted here so that a future position
/// which *does* ship sown fields fails this and gets read
/// changing what the rest of the file means.
#[test]
fn no_county_of_the_england_position_has_a_single_grain_field() {
    let save = england!();
    let kingdom = Scenario::from_save(&save).expect("import").kingdom(1);
    for id in 1..=kingdom.county_count {
        assert_eq!(kingdom.counties[id].fields_grain, 0, "county {id}");
        assert!(kingdom.counties[id].field_slots_used() >= 8, "county {id} has fields to paint");
    }
}

/// Every field tile a county claims is inside its own county on the map, and no
/// two counties claim the same tile.
///
/// This is the *map's* half of the same reading: if the byte-offset-to-index
/// conversion in `l2-scenario` were out by a factor of eight, or the county
/// plane were being read at the wrong byte of the eight-byte tile record, the
/// tiles would land in other counties and this would say so. Neither could be
/// caught by the counts alone, because a wrong tile still has *a* terrain byte.
#[test]
fn every_field_tile_lies_in_its_own_county_and_belongs_to_nobody_else() {
    let save = england!();
    let scenario = Scenario::from_save(&save).expect("import");
    let kingdom = scenario.kingdom(1);
    let map = &kingdom.campaign.map;

    let mut owner = vec![0usize; map.terrain.len()];
    for id in scenario.county_ids() {
        for slot in 0..MAX_FIELDS {
            let Some(tile) = kingdom.counties[id].field_tile(slot) else { continue };
            assert_eq!(
                map.county[tile] as usize, id,
                "county {id}'s field {slot} is on tile {tile}, which the map gives to county {}",
                map.county[tile]
            );
            assert_eq!(owner[tile], 0, "tile {tile} is claimed by counties {} and {id}", owner[tile]);
            owner[tile] = id;
        }
    }
}

