//! `Weather_UpdateAll` (`0x00449889`) ends each county's band with three calls:
//!
//! `FUN_0046942C` clears last season's ruin back to waste, then a *Drought*
//! county has one field parched (`FUN_00469A9C(county, 0x18)`) and a *Flooding*
//! one has one flooded (`0x17`), each behind `Msg_Enqueue` to the owner.
//!
//! `FUN_00469A9C` picks the field off the county's own round-robin cursor
//! `+0x15B` (`County::blight_cursor`), pre-incremented and wrapped at `+0x205`.
#![allow(unused_imports)]
use super::*;
use l2_kingdom::field::terrain;
use l2_kingdom::{Kingdom, Message, Pass, Season, SeasonReport, Weather};

fn one_county(dryness: i32, human: bool) -> Kingdom {
    let mut k = Kingdom::new(7);
    assert!(k.set_county_count(1));
    k.options.advanced_farming = true;
    k.realms[1].in_play = true;
    k.realms[1].is_human = human;
    k.counties[1].owner = 1;
    k.counties[1].dryness = dryness;
    for slot in 0..4u8 {
        let tile = l2_kingdom::map::index(slot, 8);
        assert!(k.counties[1].set_field_tile(slot as usize, Some(tile)));
        k.campaign.map.terrain[tile] = terrain::FALLOW;
    }
    k.season = Season::Summer as u8;
    assert_eq!(k.season(), Some(Season::Summer));
    k
}

fn tiles(k: &Kingdom) -> Vec<usize> {
    (0..4).map(|s| k.counties[1].field_tile(s).expect("slot filled")).collect()
}

fn count_of(k: &Kingdom, want: u8) -> usize {
    tiles(k).into_iter().filter(|&t| k.campaign.map.terrain[t] == want).count()
}

fn weather_pass(k: &mut Kingdom) -> SeasonReport {
    let mut report = SeasonReport::new();
    k.run_pass(Pass::Weather, &mut report);
    report
}

/// A drought parches **one** field and posts `0x8F` — `L2.eng` group 143.
#[test]
fn a_drought_parches_one_field_and_writes_terrain_0x18() {
    let mut k = one_county(l2_kingdom::weather::DRYNESS_MAX, true);
    let report = weather_pass(&mut k);

    assert_eq!(k.counties[1].weather, Weather::Drought);
    assert_eq!(count_of(&k, terrain::PARCHED), 1, "one field, not all four");
    assert_eq!(count_of(&k, terrain::FALLOW), 3);
    assert_eq!(report.messages, vec![Message::Drought { county: 1 }]);
    assert_eq!(report.messages[0].original_id(), Some(0x8F));
}

/// A flood floods one and posts `0x90` — group 144.
#[test]
fn a_flood_floods_one_field_and_writes_terrain_0x17() {
    let mut k = one_county(l2_kingdom::weather::DRYNESS_MIN, true);
    let report = weather_pass(&mut k);

    assert_eq!(k.counties[1].weather, Weather::Flooding);
    assert_eq!(count_of(&k, terrain::FLOODED), 1);
    assert_eq!(report.messages, vec![Message::Flooding { county: 1 }]);
    assert_eq!(report.messages[0].original_id(), Some(0x90));
}

/// **`FUN_0046942C`: wild for exactly one season, then wasteland.**
#[test]
fn last_seasons_parched_field_goes_back_to_waste() {
    let mut k = one_county(l2_kingdom::weather::DRYNESS_MAX, true);
    weather_pass(&mut k);
    let ruined = tiles(&k)
        .into_iter()
        .find(|&t| k.campaign.map.terrain[t] == terrain::PARCHED)
        .expect("a field was parched");
    assert_eq!(k.counties[1].dryness, 70, "the drought clamp");

    k.season = Season::Autumn as u8;
    weather_pass(&mut k);
    assert_ne!(k.counties[1].weather, Weather::Drought, "not a second drought");
    assert_eq!(
        k.campaign.map.terrain[ruined],
        terrain::WASTE,
        "`Terrain_Set(tile, 0)` — the player has to reclaim it"
    );
    assert_eq!(count_of(&k, terrain::PARCHED), 0);
    assert_eq!(count_of(&k, terrain::FALLOW), 3, "the other three are untouched");
}

/// `+0x15B` is advanced *before* the tile is read, so a fresh county's first
/// ruin lands on slot 1, not slot 0, and a cursor loaded at 2 sends it to
/// slot 3. Ablate the cursor — ruin the first slot that holds a tile — and
/// both halves fail.
#[test]
fn the_blight_cursor_picks_the_field_and_is_left_where_it_stopped() {
    let mut k = one_county(l2_kingdom::weather::DRYNESS_MAX, true);
    let slots = tiles(&k);
    weather_pass(&mut k);
    assert_eq!(k.campaign.map.terrain[slots[1]], terrain::PARCHED, "slot 1");
    assert_eq!(k.counties[1].blight_cursor, 1);

    let mut k = one_county(l2_kingdom::weather::DRYNESS_MAX, true);
    k.counties[1].blight_cursor = 2;
    let slots = tiles(&k);
    weather_pass(&mut k);
    assert_eq!(k.campaign.map.terrain[slots[3]], terrain::PARCHED, "slot 3");
    assert_eq!(k.counties[1].blight_cursor, 3);
}

/// The cursor wraps at the county's used-slot count (`+0x205`), so a county
/// with four fields ruins slot 0 next after slot 3.
#[test]
fn the_cursor_wraps_at_the_used_slot_count() {
    let mut k = one_county(l2_kingdom::weather::DRYNESS_MAX, true);
    k.counties[1].blight_cursor = 3;
    let slots = tiles(&k);
    weather_pass(&mut k);
    assert_eq!(k.campaign.map.terrain[slots[0]], terrain::PARCHED, "wrapped to 0");
    assert_eq!(k.counties[1].blight_cursor, 0);
}

#[test]
fn an_ai_lords_field_is_ruined_without_a_letter() {
    let mut k = one_county(l2_kingdom::weather::DRYNESS_MIN, false);
    let report = weather_pass(&mut k);

    assert_eq!(count_of(&k, terrain::FLOODED), 1, "the field is still ruined");
    assert!(report.messages.is_empty(), "no letter to an AI");
}

#[test]
fn basic_farming_flattens_the_byte_and_still_ruins_the_field() {
    let mut k = one_county(l2_kingdom::weather::DRYNESS_MIN, true);
    k.options.advanced_farming = false;
    let report = weather_pass(&mut k);

    assert_eq!(k.counties[1].weather, Weather::Cloudy);
    assert_eq!(count_of(&k, terrain::FLOODED), 1);
    assert_eq!(report.messages, vec![Message::Flooding { county: 1 }]);
}

/// A county with no field slots is left alone: `FUN_00469A9C` gives up after
/// twenty tries and writes no tile.
#[test]
fn a_county_with_no_fields_survives_a_drought() {
    let mut k = one_county(l2_kingdom::weather::DRYNESS_MAX, true);
    let slots = tiles(&k);
    for slot in 0..4 {
        k.counties[1].set_field_tile(slot, None);
    }
    weather_pass(&mut k);
    assert_eq!(k.counties[1].weather, Weather::Drought);
    for tile in slots {
        assert_eq!(k.campaign.map.terrain[tile], terrain::FALLOW);
    }
}
