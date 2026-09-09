//! **The field counts, against the tiles they were counted from.**
//!
//! ```text
//! LORDS2_FIXTURES="E:\dev\lords2-fixtures" cargo test -p l2-kingdom --test fields
//! ```
//!
//! # What this is evidence about
//!
//! `l2_kingdom::field::classify` is a ladder of six terrain boundaries taken
//! off `County_RecountFields` (`FUN_00469B8D`, `0x00469B8D`) in the
//! decompiler. `docs/decisions.md` C3 is what happens when a plausible ladder
//! is believed because it is tidy, and C24 is what happens when a
//! hand-transcribed table is never read back.
//!
//! The England turn-one save closes that loop **without any of our rules being
//! involved**, because it stores both halves of the sum. `g_countyFieldTiles`
//! names the twenty tiles, `g_tiles` holds each one's terrain byte, and county
//! `+0x1FF`, `+0x200` and `+0x201` hold the three counts the game itself made
//! of them. Applying our ladder to the first and comparing against the third is
//! a check nothing in this tree can make come out right by agreeing with
//! itself: the file was written by the original.
//!
//! # And it is the reason the brush exists
//!
//! Every one of the fourteen counties has `fieldsGrain = 0`. That is not a
//! quirk of this position — it is what the start of a game *is*: you paint your
//! fields. Until [`l2_kingdom::field`] there was no code path in this tree that
//! could set that number for the human player at all, so the entire grain half
//! of the economy was finished, tested and unreachable in play.

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
/// which *does* ship sown fields fails this and gets read rather than silently
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

/// **The end-to-end one.** A human player paints a field to grain, ends four
/// turns, and the grain pipeline runs: sown, grown, harvested.
///
/// This is the first time this engine has driven its own economy from the
/// player's side. It is deliberately built on the real position rather than on
/// a fabricated county, because `docs/decisions.md` C26 is what happens when
/// the only fixture exercises one value of a rule's input.
#[test]
fn a_player_can_paint_a_field_to_grain_and_harvest_it_four_seasons_later() {
    let save = england!();
    let scenario = Scenario::from_save(&save).expect("import");
    let mut k = scenario.kingdom(1);

    // The human's county. `g_localPlayer` is realm 1 and which counties it
    // holds is rolled per game, so it is found rather than named (C23).
    let human = scenario.local_player;
    let mine = (1..=k.county_count)
        .find(|&id| k.counties[id].owner == human)
        .expect("the local player holds a county");

    // Seed grain, **before** painting. The five owned counties of this
    // position store none at all (C20 — the file records no *opening* store),
    // and the order matters for a reason worth stating: the grain ceiling the
    // brush computes is a search over `Grain_Sow`, which asks whether the
    // store can afford the seed. A county with an empty granary is told it has
    // no use for a farmer, and none is assigned.
    k.counties[mine].grain = 10_000;
    let before = k.counties[mine].grain;

    // Paint every fallow field this county has to grain.
    let painted = paint_all_fallow_to_grain(&mut k, mine);
    assert!(painted > 0, "county {mine} had fields to paint");
    assert_eq!(k.counties[mine].fields_grain, painted, "and they are grain now");
    assert!(
        k.counties[mine].labour[0] > 0,
        "and painting put farmers on them: {:?}",
        k.counties[mine].labour
    );

    let sown = k.advance_season();
    assert!(
        k.counties[mine].grain < before,
        "sowing debits the seed: {} -> {}",
        before,
        k.counties[mine].grain
    );
    let crop_after_sowing: i32 = k.counties[mine].crop.iter().sum();
    assert!(crop_after_sowing > 0, "and it puts a crop in the ground: {sown:?}");

    // Spring and Summer grow it; Autumn harvests it into the store.
    let mut low = k.counties[mine].grain;
    for _ in 0..3 {
        k.advance_season();
        low = low.min(k.counties[mine].grain);
    }
    assert_eq!(k.season, 4, "back to Winter");
    assert!(
        k.counties[mine].grain > low,
        "the harvest put grain back in the store: low {low}, now {}",
        k.counties[mine].grain
    );
    // **The crop words are not cleared by the harvest**, and this test used to
    // say they were. `crop` is *seed, standing crop, harvest* rather than three
    // growth stages: `Grain_SeasonTick` clears `crop[2]` at the top of every
    // season and fills it at the harvest, and `crop[0]` and `crop[1]` keep the
    // year's record until the next sowing overwrites them. See
    // [`l2_kingdom::land`].
    assert!(
        k.counties[mine].crop[2] > 0,
        "the harvest is the third word: {:?}",
        k.counties[mine].crop
    );
    assert_eq!(
        k.counties[mine].crop[0] * l2_kingdom::tables::GRAIN_YIELD_PER_SACK,
        k.counties[mine].crop[1],
        "and the first two are still the seed and the crop it became"
    );
}

/// The same painting, done twice, is the same kingdom — the brush is a rule and
/// rules are deterministic (`docs/netcode.md`).
#[test]
fn painting_is_deterministic() {
    let save = england!();
    let scenario = Scenario::from_save(&save).expect("import");
    let run = || {
        let mut k = scenario.kingdom(7);
        let mine = (1..=k.county_count)
            .find(|&id| k.counties[id].owner == scenario.local_player)
            .expect("a county");
        paint_all_fallow_to_grain(&mut k, mine);
        k.counties[mine].grain = 10_000;
        for _ in 0..4 {
            k.advance_season();
        }
        k
    };
    assert!(run() == run(), "two identical playthroughs diverged");
}

/// Paint every fallow field of one county to grain, the way the player does it:
/// one tile, one brush stroke. Returns how many strokes landed.
fn paint_all_fallow_to_grain(k: &mut Kingdom, county: usize) -> i32 {
    let tiles: Vec<usize> = k
        .field_tiles(county)
        .into_iter()
        .filter(|&(_, kind)| kind == FieldType::Fallow)
        .map(|(tile, _)| tile)
        .collect();
    let mut painted = 0;
    for tile in tiles {
        k.paint_field(county, tile, FieldType::Grain)
            .expect("a fallow field takes the grain brush");
        painted += 1;
    }
    painted
}
