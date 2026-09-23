#![allow(unused_imports)]
use super::*;
use super::paths::*;
use super::burning::*;
use super::super::*;
use crate::map::flags;
use crate::unit::Unit;

#[test]
fn an_army_stops_when_its_fifteen_moves_are_spent() {
    let mut m = open_map();
    let (mut counties, realms) = blank();
    let mut units = Units::new();
    let id = army_at(&mut units, 1, 10, 10);
    order_move(&m, &mut units, id, (30, 10), Routing::Direct);
    let ordered = order_move(&m, &mut units, id, (30, 10), Routing::Direct).unwrap();
    let steps = march(&mut m, &mut counties, &realms, &mut units, id);
    assert_eq!(steps.len(), 5, "fifteen moves at three a tile");
    assert_eq!(units.get(id).unwrap().moves_left(), 0);
    let left = units.get(id).unwrap().path.len();
    assert_eq!(left, ordered - 5, "the rest of the path is kept");

    units.reset_moves();
    march(&mut m, &mut counties, &realms, &mut units, id);
    assert_eq!(units.get(id).unwrap().path.len(), left - 5);
    assert_eq!(units.get(id).unwrap().moves_used, 15);
}

#[test]
fn crossing_a_standing_field_costs_six_and_matches_the_cost_map() {
    let mut m = open_map();
    m.set_flags(11, 10, flags::FARMLAND);
    m.set_terrain(11, 10, 10);
    assert_eq!(m.cost_map().at(11, 10), 6, "the cost map's number");

    let (mut counties, realms) = blank();
    counties[1].owner = 1;
    let mut units = Units::new();
    let id = army_at(&mut units, 1, 10, 10);
    order_move(&m, &mut units, id, (11, 10), Routing::Direct);
    let steps = march(&mut m, &mut counties, &realms, &mut units, id);
    assert_eq!(steps[0].charged, 6, "and the stepper's, assembled from two threes");
}

#[test]
fn an_army_tramples_a_foreign_field_and_not_its_own() {
    let mut m = open_map();
    m.set_flags(11, 10, flags::FARMLAND);
    m.set_terrain(11, 10, 10);
    let (mut counties, realms) = blank();
    counties[1].owner = 1;
    counties[1].fields_grain = 4;
    // A standing crop was sown, and sowing writes `+0x206` beside the count;
    // `County_DestroyField` charges the crop against it.
    counties[1].fields_grain_standing = 4;
    counties[1].crop[1] = 400;

    let mut units = Units::new();
    let id = army_at(&mut units, 1, 10, 10);
    order_move(&m, &mut units, id, (11, 10), Routing::Direct);
    let steps = march(&mut m, &mut counties, &realms, &mut units, id);
    assert_eq!(steps[0].charged, 6);
    assert_eq!(steps[0].field_destroyed, None);
    assert_eq!(counties[1].fields_grain, 4);

    let mut m = open_map();
    m.set_flags(11, 10, flags::FARMLAND);
    m.set_terrain(11, 10, 10);
    let mut units = Units::new();
    let id = army_at(&mut units, 2, 10, 10);
    units.get_mut(id).unwrap().owner_is_human = true;
    order_move(&m, &mut units, id, (11, 10), Routing::Direct);
    let steps = march(&mut m, &mut counties, &realms, &mut units, id);
    assert_eq!(steps[0].field_destroyed, Some(1));
    assert_eq!(counties[1].fields_grain, 3);
    assert_eq!(counties[1].crop[1], 300, "one field of four, so a quarter of the crop");
    assert_eq!(
        steps[0].offence,
        Some(Offence { against: 1, by: 2, amount: FIELD_TRAMPLE_OFFENCE })
    );
}

#[test]
fn an_ai_army_pays_no_diplomatic_price_for_trampling() {
    let mut m = open_map();
    m.set_flags(11, 10, flags::FARMLAND);
    m.set_terrain(11, 10, 10);
    let (mut counties, realms) = blank();
    counties[1].owner = 1;
    counties[1].fields_grain = 2;
    counties[1].crop[1] = 100;

    let mut units = Units::new();
    let id = army_at(&mut units, 2, 10, 10);
    units.get_mut(id).unwrap().owner_is_human = false;
    order_move(&m, &mut units, id, (11, 10), Routing::Direct);
    let steps = march(&mut m, &mut counties, &realms, &mut units, id);
    assert_eq!(steps[0].field_destroyed, Some(1), "the field still goes");
    assert_eq!(steps[0].offence, None, "and it costs nothing");
}

#[test]
fn a_trampled_field_becomes_bare_ground_and_costs_three_afterwards() {
    let mut m = open_map();
    m.set_flags(11, 10, flags::FARMLAND);
    m.set_terrain(11, 10, 10);
    let (mut counties, realms) = blank();
    counties[1].owner = 1;
    counties[1].fields_grain = 1;
    counties[1].crop[1] = 60;
    let mut units = Units::new();
    let id = army_at(&mut units, 2, 10, 10);
    order_move(&m, &mut units, id, (11, 10), Routing::Direct);
    march(&mut m, &mut counties, &realms, &mut units, id);
    assert_eq!(m.terrain_at(11, 10), 0);
    assert_eq!(m.cost_map().at(11, 10), 3, "bare farmland is ordinary ground");
}

#[test]
fn trampling_a_pasture_takes_a_share_of_the_herd() {
    let mut m = open_map();
    m.set_flags(11, 10, flags::FARMLAND);
    m.set_terrain(11, 10, 0x16);
    let (mut counties, realms) = blank();
    counties[1].owner = 1;
    counties[1].fields_cattle = 5;
    counties[1].herd = 100;
    let mut units = Units::new();
    let id = army_at(&mut units, 2, 10, 10);
    order_move(&m, &mut units, id, (11, 10), Routing::Direct);
    march(&mut m, &mut counties, &realms, &mut units, id);
    assert_eq!(counties[1].fields_cattle, 4);
    assert_eq!(counties[1].herd, 80, "one pasture of five is a fifth of the herd");
}


#[test]
fn marching_over_an_enemy_mine_shuts_it_down_for_three_seasons() {
    let mut m = open_map();
    m.set_flags(11, 10, flags::SETTLEMENT);
    m.set_terrain(11, 10, 1); // an iron site
    let (mut counties, realms) = blank();
    counties[1].owner = 1;
    counties[1].industry[1].efficiency = 80;

    let mut units = Units::new();
    let id = army_at(&mut units, 2, 10, 10);
    units.get_mut(id).unwrap().path = vec![(11, 10)];
    units.get_mut(id).unwrap().moving = true;

    let s = step(&mut m, &mut counties, &realms, &mut units, id).unwrap();
    assert_eq!(s.entry, Entry::Settlement);
    assert_eq!(s.charged, 7);
    assert!(!s.moved, "the move ends at the site, it does not enter");
    assert_eq!(s.site_ruined, Some((1, 1)));
    assert_eq!(counties[1].industry[1].disabled_seasons, 3);
    assert_eq!(counties[1].industry[1].efficiency, 0);
    assert_eq!(m.terrain_at(11, 10), 3, "the ruined state");
    assert_eq!(m.cost_map().at(11, 10), 0, "and a hole in the map afterwards");
}

#[test]
fn each_terrain_group_ruins_its_own_industry_record() {
    for (terrain_byte, ruined, record) in
        [(0u8, 3u8, 1usize), (2, 3, 1), (4, 6, 3), (5, 6, 3), (7, 9, 2), (8, 9, 2), (10, 12, 0), (11, 12, 0)]
    {
        let mut m = open_map();
        m.set_flags(11, 10, flags::SETTLEMENT);
        m.set_terrain(11, 10, terrain_byte);
        let (mut counties, realms) = blank();
        counties[1].owner = 1;
        let mut units = Units::new();
        let id = army_at(&mut units, 2, 10, 10);
        units.get_mut(id).unwrap().path = vec![(11, 10)];
        let s = step(&mut m, &mut counties, &realms, &mut units, id).unwrap();
        assert_eq!(s.site_ruined, Some((1, record)), "terrain {terrain_byte}");
        assert_eq!(m.terrain_at(11, 10), ruined);
    }
}

#[test]
fn trampling_is_free_on_your_own_site_and_on_one_already_ruined() {
    let mut m = open_map();
    m.set_flags(11, 10, flags::SETTLEMENT);
    m.set_terrain(11, 10, 1);
    let (mut counties, realms) = blank();
    counties[1].owner = 2;

    let mut units = Units::new();
    let id = army_at(&mut units, 2, 10, 10);
    units.get_mut(id).unwrap().path = vec![(11, 10)];
    let s = step(&mut m, &mut counties, &realms, &mut units, id).unwrap();
    assert_eq!(s.charged, 0, "your own mine");
    assert_eq!(s.site_ruined, None);
    assert_eq!(counties[1].industry[1].disabled_seasons, 0);

    counties[1].owner = 1;
    m.set_terrain(11, 10, 3);
    units.get_mut(id).unwrap().path = vec![(11, 10)];
    let s = step(&mut m, &mut counties, &realms, &mut units, id).unwrap();
    assert_eq!(s.charged, 0, "there is nothing left to wreck");
}

