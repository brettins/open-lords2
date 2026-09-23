#![allow(unused_imports)]
use super::*;
use super::styles::*;
use super::feeding::*;
use super::super::*;

#[test]
fn step_five_stamps_the_lords_style_on_every_county_he_holds() {
    let mut counties = vec![County::new(); 4];
    let mut map = CampaignMap::empty();
    for id in 1..=3 {
        for i in 0..6 {
            let tile = crate::map::index(i as u8, id as u8);
            counties[id].set_field_tile(i, Some(tile));
            map.terrain[tile] = terrain::FALLOW;
        }
        counties[id].population = 500;
        crate::field::recount(&mut counties[id], &map);
    }
    counties[1].owner = 2;
    counties[2].owner = 3;
    counties[3].owner = 2;

    manage_county_farms(T, &mut counties, 3, &mut map, &realms(), 2, 4, &mut NoMarket, &env());
    assert_eq!(counties[1].farm_style, 9);
    assert_eq!(counties[3].farm_style, 9);
    assert_eq!(counties[2].farm_style, 0, "another realm's county is untouched");
    assert_eq!(counties[1].industry_share, FarmStyle::RealmMixed.industry_share());
    assert_eq!(counties[2].industry_share, 25, "a fresh county's default");
}

#[test]
fn an_unowned_county_farms_by_whichever_lord_held_it_last() {
    let mut counties = vec![County::new(); 3];
    let mut map = CampaignMap::empty();
    for id in 1..=2 {
        for i in 0..6 {
            let tile = crate::map::index(i as u8, id as u8);
            counties[id].set_field_tile(i, Some(tile));
            map.terrain[tile] = terrain::FALLOW;
        }
        counties[id].population = 500;
        counties[id].herd = 200;
        crate::field::recount(&mut counties[id], &map);
        counties[id].herd_crowding =
            crate::land::herd_crowding(T, counties[id].herd, counties[id].fields_cattle);
    }
    counties[1].farm_style = 1; // an ex-grazier's county
    counties[2].farm_style = 9; // an ex-Knight's

    manage_neutral_fields(T, &mut counties, 2, &mut map, &realms(), &mut NoMarket, &env());
    assert_eq!(counties[1].farm_style, 1, "never rewritten");
    assert!(counts(&counties[1], &map).1 > 0, "it grazed");
    assert_eq!(
        counts(&counties[2], &map),
        (0, 0, 6),
        "style 9 dispatches nowhere in the neutral pass"
    );
}

#[test]
fn ordering_a_field_puts_a_wasteland_tile_under_reclamation() {
    let mut counties = vec![County::new(); 2];
    let mut map = CampaignMap::empty();
    counties[1].owner = 2;
    counties[1].population = 10;
    for i in 0..4 {
        let tile = crate::map::index(i as u8, 3);
        counties[1].set_field_tile(i, Some(tile));
        map.terrain[tile] = terrain::WASTE;
    }
    crate::field::recount(&mut counties[1], &map);
    assert_eq!(counties[1].fields_waste, 4);

    assert_eq!(
        manage_county_farms(T, &mut counties, 1, &mut map, &realms(), 2, 4, &mut NoMarket, &env()),
        1,
        "an empty county always gets its first field"
    );
    crate::field::recount(&mut counties[1], &map);
    assert_eq!(counties[1].fields_reclaiming, 1);
    assert_eq!(counties[1].fields_waste, 3);
}

#[test]
fn the_field_ladder_orders_a_field_only_when_both_conditions_hold() {
    let case = |fields: usize, population: i32| {
        let mut c = County::new();
        let mut map = CampaignMap::empty();
        for i in 0..20 {
            let tile = crate::map::index(i as u8, 8);
            c.set_field_tile(i, Some(tile));
            map.terrain[tile] = if i < fields { terrain::FALLOW } else { terrain::WASTE };
        }
        crate::field::recount(&mut c, &map);
        c.population = population;
        order_fields(&c, &mut map)
    };
    assert_eq!(case(0, 10), 1, "an empty county always gets its first field");
    assert_eq!(case(2, 150), 0, "small, but not populous enough for any row");
    assert_eq!(case(2, 201), 1);
    assert_eq!(case(4, 401), 1);
    assert_eq!(case(6, 601), 1);
    assert_eq!(case(8, 1001), 1);
    assert_eq!(case(8, 601), 0, "under 1000 the fields < 9 row does not fire");
    assert_eq!(case(12, 1201), 2, "a big county adds two");
    assert_eq!(case(12, 1200), 0, "and the test is strictly greater");
}

#[test]
fn a_full_county_cannot_gain_another_field() {
    let (mut c, mut map) = county_with(crate::county::MAX_FIELDS);
    c.population = 5_000;
    assert_eq!(order_fields(&c, &mut map), 0);
    crate::field::recount(&mut c, &map);
    assert_eq!(c.field_total(), crate::county::MAX_FIELDS as i32);
}

#[test]
fn a_county_with_nothing_left_to_reclaim_gains_no_field() {
    let (mut c, mut map) = county_with(2);
    c.population = 10;
    assert_eq!(order_fields(&c, &mut map), 0, "two fallow fields, no waste");
}

#[test]
fn a_reclamation_already_running_consumes_the_quota_without_doing_anything() {
    let build = || {
        let mut c = County::new();
        let mut map = CampaignMap::empty();
        for i in 0..4 {
            let tile = crate::map::index(i as u8, 5);
            c.set_field_tile(i, Some(tile));
            map.terrain[tile] = terrain::WASTE;
        }
        map.terrain[c.field_tile(0).unwrap()] = terrain::RECLAIM_FIRST + 1;
        crate::field::recount(&mut c, &map);
        (c, map)
    };
    let (c, mut map) = build();
    assert_eq!(crate::field::order_reclamation(&c, &mut map, 1), 0);
    let (c, mut map) = build();
    assert_eq!(crate::field::order_reclamation(&c, &mut map, 2), 1);
    let (mut c, mut map) = build();
    map.terrain[c.field_tile(0).unwrap()] = terrain::GRAIN;
    crate::field::recount(&mut c, &map);
    assert_eq!(crate::field::order_reclamation(&c, &mut map, 1), 1);
}

#[test]
fn a_lord_with_no_personality_record_farms_nothing() {
    let mut counties = vec![County::new(); 2];
    let mut map = CampaignMap::empty();
    counties[1].owner = 2;
    counties[1].population = 900;
    for i in 0..4 {
        let tile = crate::map::index(i as u8, 7);
        counties[1].set_field_tile(i, Some(tile));
        map.terrain[tile] = terrain::WASTE;
    }
    crate::field::recount(&mut counties[1], &map);
    assert_eq!(
        manage_county_farms(T, &mut counties, 1, &mut map, &realms(), 2, 5, &mut NoMarket, &env()),
        0
    );
    crate::field::recount(&mut counties[1], &map);
    assert_eq!(counties[1].fields_reclaiming, 0, "not even a field ordered");
}

// --- FUN_004A4694 -------------------------------------------------------

#[test]
fn the_cattle_nudge_moves_one_field_and_only_under_its_own_conditions() {
    let grow = |crowding: i32, herd: i32, cattle: usize, cap: i32| {
        let (mut c, mut map) = county_with(8);
        for slot in 0..cattle {
            map.terrain[c.field_tile(slot).unwrap()] = terrain::PASTURE;
        }
        crate::field::recount(&mut c, &map);
        c.herd = herd;
        c.herd_crowding = crowding;
        fit_cattle_fields(T, &mut c, &mut map, cap);
        c.fields_cattle
    };
    assert_eq!(grow(11, 100, 2, 5), 3, "crowded and under the cap: one more");
    assert_eq!(grow(11, 100, 5, 5), 5, "at the cap: nothing");
    assert_eq!(grow(10, 5, 3, 5), 2, "uncrowded, tiny herd, spare fields: one back");
    assert_eq!(grow(10, 5, 2, 5), 2, "two fields is the floor");
    assert_eq!(grow(10, 50, 3, 5), 3, "a herd of ten or more keeps its fields");
}


