#![allow(unused_imports)]
use super::*;
use super::trample::*;
use super::burning::*;
use super::super::*;
use crate::map::flags;
use crate::unit::Unit;

#[test]
fn the_start_costs_nothing_and_a_neighbour_costs_its_own_tile() {
    let f = flood_fill(&open_map().cost_map(), (10, 10), Routing::Direct);
    assert_eq!(f.raw(10, 10), START_DISTANCE);
    assert_eq!(f.cost_to(10, 10), Some(0));
    assert_eq!(f.cost_to(11, 10), Some(3), "one step of open ground");
    assert_eq!(f.cost_to(11, 11), Some(3), "a diagonal costs the same");
    assert_eq!(f.cost_to(13, 10), Some(9));
}

#[test]
fn an_impassable_tile_is_never_reached_and_reads_as_unreached() {
    let mut m = open_map();
    for (dx, dy) in STEP_DIRECTIONS {
        m.set_flags((20 + dx) as u8, (20 + dy) as u8, flags::NO_COUNTY);
    }
    let f = flood_fill(&m.cost_map(), (5, 5), Routing::Direct);
    assert_eq!(f.cost_to(20, 20), None, "walled in");
    assert_eq!(f.cost_to(20, 19), None, "and the wall itself is unreached");
    assert_eq!(f.cost_to(18, 18), Some(3 * 13));
}

#[test]
fn a_road_tile_expands_orthogonally_only() {
    let mut m = open_map();
    m.set_flags(10, 10, flags::ROAD);
    let f = flood_fill(&m.cost_map(), (10, 10), Routing::Direct);
    for (dx, dy) in [(0i32, -1i32), (1, 0), (0, 1), (-1, 0)] {
        let (x, y) = ((10 + dx) as u8, (10 + dy) as u8);
        assert_eq!(f.cost_to(x, y), Some(3), "{x},{y} is one open tile away");
    }
    assert_eq!(f.cost_to(11, 11), Some(6), "the diagonal had to go round");

    let plain = flood_fill(&open_map().cost_map(), (10, 10), Routing::Direct);
    assert_eq!(plain.cost_to(11, 11), Some(3));
}

#[test]
fn a_cheaper_route_arriving_later_rewrites_the_cost() {
    let mut m = open_map();
    for y in 8..=12u8 {
        m.set_flags(11, y, flags::FARMLAND);
        m.set_terrain(11, y, 10);
    }
    let cost = m.cost_map();
    assert_eq!(cost.at(11, 10), 6);
    let f = flood_fill(&cost, (10, 10), Routing::Direct);
    assert_eq!(f.cost_to(11, 10), Some(6));

    let mut m = open_map();
    for y in 0..MAP_DIM as u8 {
        if y != 0 {
            m.set_flags(11, y, flags::NO_COUNTY);
        }
    }
    let f = flood_fill(&m.cost_map(), (10, 10), Routing::Direct);
    assert_eq!(f.cost_to(12, 10), Some(3 * 20));
}

#[test]
fn preferring_roads_prices_everything_else_at_a_hundred() {
    let mut m = open_map();
    for x in 0..20u8 {
        m.set_flags(x, 10, flags::ROAD);
    }
    let cost = m.cost_map();
    let direct = flood_fill(&cost, (0, 10), Routing::Direct);
    let hugging = flood_fill(&cost, (0, 10), Routing::PreferRoads);
    assert_eq!(direct.cost_to(10, 10), Some(10));
    assert_eq!(hugging.cost_to(10, 10), Some(10), "a road is 1 in both modes");
    assert_eq!(direct.cost_to(10, 11), Some(13), "one step off the road");
    assert_eq!(hugging.cost_to(10, 11), Some(110), "…and a hundred to the road-hugger");
}

#[test]
fn the_fill_never_walks_off_the_edge_of_the_map() {
    let f = flood_fill(&open_map().cost_map(), (0, 0), Routing::Direct);
    assert_eq!(f.cost_to(63, 0), Some(3 * 63));
    assert_eq!(f.cost_to(1, 0), Some(3));
}

#[test]
fn the_same_map_always_produces_the_same_field() {
    let m = open_map();
    let first = flood_fill(&m.cost_map(), (7, 9), Routing::Direct);
    for _ in 0..5 {
        assert_eq!(flood_fill(&m.cost_map(), (7, 9), Routing::Direct), first);
    }
}


#[test]
fn a_path_is_returned_in_travel_order_without_the_starting_tile() {
    let m = open_map();
    let cost = m.cost_map();
    let f = flood_fill(&cost, (10, 10), Routing::Direct);
    let path = extract_path(&cost, &f, (14, 10)).expect("a clear route");
    assert_eq!(path.len(), 4);
    assert_eq!(*path.last().unwrap(), (14, 10));
    assert!(!path.contains(&(10, 10)), "the starting tile is not in the path");

    assert_eq!(path, vec![(11, 11), (12, 12), (13, 11), (14, 10)]);
}

#[test]
fn every_step_of_an_extracted_path_is_to_a_neighbouring_tile() {
    let mut m = open_map();
    for y in 0..MAP_DIM as u8 {
        if y != 40 {
            m.set_flags(20, y, flags::NO_COUNTY);
        }
    }
    let cost = m.cost_map();
    let f = flood_fill(&cost, (5, 10), Routing::Direct);
    let path = extract_path(&cost, &f, (35, 10)).expect("through the gap");
    assert!(!path.is_empty());
    let mut prev = (5u8, 10u8);
    for &(x, y) in &path {
        let d = ((x as i32 - prev.0 as i32).abs()).max((y as i32 - prev.1 as i32).abs());
        assert_eq!(d, 1, "{prev:?} -> {:?} is not one step", (x, y));
        assert_ne!(cost.at(x, y), 0, "walked onto impassable ground");
        prev = (x, y);
    }
    assert_eq!(*path.last().unwrap(), (35, 10));
    assert!(path.iter().any(|&(x, y)| x == 20 && y == 40), "through the one gap");
}

#[test]
fn an_unreachable_destination_gives_an_empty_path_rather_than_a_failure() {
    let mut m = open_map();
    for (dx, dy) in STEP_DIRECTIONS {
        m.set_flags((30 + dx) as u8, (30 + dy) as u8, flags::NO_COUNTY);
    }
    let cost = m.cost_map();
    let f = flood_fill(&cost, (5, 5), Routing::Direct);
    assert_eq!(extract_path(&cost, &f, (30, 30)), Some(Vec::new()), "reached the descent and stopped at once");

    let mut units = Units::new();
    let id = army_at(&mut units, 1, 5, 5);
    assert_eq!(order_move(&m, &mut units, id, (30, 30), Routing::Direct), Some(0));
    assert!(units.get(id).unwrap().moving, "the order is still accepted");
    assert_eq!(units.get(id).unwrap().dest, Some((30, 30)));
}

/// * the search bound — `Move_FloodFill` (`0x0046F700`) wraps its frontier
///   queue at `0x3FF`, which is [`QUEUE_CAP`]. A fill of open ground never
///   queues more than 250 of those 1,024 cells, so it never truncates;
/// * the step cost — 3 an open tile, `Unit_StepOnce` (`0x0046634D`);
/// * the length cap — 150 steps, `Path_CopyToUnit` (`0x004707BE`).
#[test]
fn a_march_of_ten_to_thirty_open_tiles_is_ordered_whole() {
    let m = open_map();
    let mut units = Units::new();
    let id = army_at(&mut units, 1, 16, 16);
    for d in 10..=30u8 {
        let dest = (16, 16 + d);
        assert_eq!(
            order_move(&m, &mut units, id, dest, Routing::Direct),
            Some(d as usize),
            "a march of {d} open tiles"
        );
        assert_eq!(units.get(id).unwrap().path.len(), d as usize);
    }
    let f = flood_fill(&m.cost_map(), (16, 16), Routing::Direct);
    assert_eq!(f.reached().count(), MAP_TILES, "the whole map, so nothing was cut short");
    assert_eq!(f.cost_to(16, 46), Some(90), "30 open tiles at 3 each");
}

#[test]
fn a_path_is_capped_at_a_hundred_and_fifty_steps() {
    let m = open_map();
    let cost = m.cost_map();
    let f = flood_fill(&cost, (0, 0), Routing::Direct);
    assert!(extract_path(&cost, &f, (63, 63)).expect("open ground").len() <= MAX_PATH);
}


#[test]
fn a_road_step_costs_one_and_an_open_step_costs_three() {
    let mut m = open_map();
    for x in 10..14u8 {
        m.set_flags(x, 10, flags::ROAD);
    }
    let (mut counties, realms) = blank();
    counties[1].owner = 1;
    let mut units = Units::new();
    let id = army_at(&mut units, 1, 10, 10);
    order_move(&m, &mut units, id, (15, 10), Routing::Direct);

    let steps = march(&mut m, &mut counties, &realms, &mut units, id);
    let charged: Vec<i32> = steps.iter().map(|s| s.charged).collect();
    assert_eq!(charged, vec![1, 1, 1, 3, 3]);
    assert_eq!(units.get(id).unwrap().tile(), (15, 10));
    assert_eq!(units.get(id).unwrap().moves_used, 9);
}

#[test]
fn the_road_flag_does_not_stick_after_leaving_the_road() {
    let mut m = open_map();
    m.set_flags(11, 10, flags::ROAD);
    let (mut counties, realms) = blank();
    let mut units = Units::new();
    let id = army_at(&mut units, 1, 10, 10);
    order_move(&m, &mut units, id, (13, 10), Routing::Direct);
    let steps = march(&mut m, &mut counties, &realms, &mut units, id);
    assert_eq!(steps.iter().map(|s| s.charged).collect::<Vec<_>>(), vec![1, 3, 3]);
    assert!(!units.get(id).unwrap().on_road);
}

