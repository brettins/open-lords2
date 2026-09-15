#![allow(unused_imports)]
use super::*;
use super::pathfinding::*;
use super::stepper::*;
use crate::county::{County, MAX_COUNTIES};
use crate::map::{coords, index, terrain, CampaignMap, CostMap, MAP_DIM, MAP_TILES};
use crate::realm::{Realm, MAX_REALMS};
use crate::unit::{UnitKind, Units, MAX_PATH};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::flags;
    use crate::unit::Unit;

    fn open_map() -> CampaignMap {
        let mut m = CampaignMap::empty();
        for i in 0..MAP_TILES {
            m.county[i] = 1;
        }
        m
    }

    fn blank() -> ([County; MAX_COUNTIES], [Realm; MAX_REALMS]) {
        (core::array::from_fn(|_| County::new()), core::array::from_fn(|_| Realm::new()))
    }

    /// **`County_DestroyField` (`0x00469E5B`) charges against `+0x206` and
    /// steps it down**
    #[test]
    fn a_wrecked_grain_field_charges_and_steps_down_the_fields_still_standing() {
        let mut map = open_map();
        let mut c = County::new();
        c.fields_grain = 4;
        c.fields_grain_standing = 4;
        c.crop[1] = 400;
        map.set_flags(3, 3, flags::FARMLAND);
        map.set_terrain(3, 3, 7);
        assert_eq!(destroy_field(&mut c, &mut map, 3, 3), 100);
        assert_eq!((c.crop[1], c.fields_grain, c.fields_grain_standing), (300, 3, 3));
        assert_eq!(map.terrain_at(3, 3), 0);

        // More grain painted than was sown: `fieldsGrain > +0x206`, so the share
        // is 0 and `+0x206` stays — the original's `else 0`.
        let mut c = County::new();
        c.fields_grain = 6;
        c.fields_grain_standing = 4;
        c.crop[1] = 400;
        map.set_terrain(3, 3, 7);
        assert_eq!(destroy_field(&mut c, &mut map, 3, 3), 0);
        assert_eq!((c.crop[1], c.fields_grain, c.fields_grain_standing), (400, 5, 4));

        let mut c = County::new();
        c.fields_grain = 2;
        map.set_terrain(3, 3, 7);
        assert_eq!(destroy_field(&mut c, &mut map, 3, 3), 0);
        assert_eq!(c.fields_grain, 2, "the grain arm is guarded on +0x206");
        assert_eq!(map.terrain_at(3, 3), 0, "and the repaint is not guarded at all");
    }

    fn army_at(units: &mut Units, owner: u8, x: u8, y: u8) -> usize {
        let mut u = Unit::new(UnitKind::Army, owner, x, y);
        u.men = 200;
        u.county = 1;
        units.spawn(u).unwrap()
    }


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

    #[test]
    fn burning_a_dwelling_costs_seven_moves_which_the_document_records_as_nothing() {
        let mut m = open_map();
        m.set_flags(11, 10, flags::PLOT);
        m.set_terrain(11, 10, terrain::DWELLING);
        let (mut counties, realms) = blank();
        counties[1].owner = 1;
        let mut units = Units::new();
        let id = army_at(&mut units, 2, 10, 10);
        units.get_mut(id).unwrap().path = vec![(11, 10)];
        let s = step(&mut m, &mut counties, &realms, &mut units, id).unwrap();
        assert_eq!(s.entry, Entry::Plot);
        assert_eq!(s.charged, 7);
        assert!(!s.moved);
    }

    /// **The other half of `Unit_BurnDwelling` (`0x00468AE2`)** — the tile and
    /// the dead. 400 → 300 is the literal quarter; the plot goes to `0x13`,
    /// which `docs/draws-map.md` §3.2 draws the sixteen damage frames over.
    #[test]
    fn burning_a_dwelling_burns_the_plot_and_takes_a_quarter_of_the_people() {
        let mut m = open_map();
        m.set_flags(11, 10, flags::PLOT);
        m.set_terrain(11, 10, terrain::DWELLING);
        let (mut counties, realms) = blank();
        counties[1].owner = 1;
        counties[1].population = 400;
        let mut units = Units::new();
        let id = army_at(&mut units, 2, 10, 10);
        units.get_mut(id).unwrap().path = vec![(11, 10)];
        let s = step(&mut m, &mut counties, &realms, &mut units, id).unwrap();
        assert_eq!(s.dwelling_burnt, Some(1));
        assert_eq!(m.terrain_at(11, 10), terrain::DWELLING_BURNT);
        assert_eq!(counties[1].population, 300);
        assert_eq!(s.offence.map(|o| o.amount), Some(DWELLING_BURN_OFFENCE));
    }

    #[test]
    fn nothing_is_burnt_by_a_merchant_by_its_owner_or_twice() {
        for (kind, unit_owner, t) in [
            (UnitKind::Merchant, 2u8, terrain::DWELLING),
            (UnitKind::Army, 1, terrain::DWELLING),
            (UnitKind::Army, 2, terrain::DWELLING_BURNT),
        ] {
            let mut m = open_map();
            m.set_flags(11, 10, flags::PLOT);
            m.set_terrain(11, 10, t);
            let (mut counties, realms) = blank();
            counties[1].owner = 1;
            counties[1].population = 400;
            let mut units = Units::new();
            let id = army_at(&mut units, unit_owner, 10, 10);
            units.get_mut(id).unwrap().kind = kind;
            units.get_mut(id).unwrap().path = vec![(11, 10)];
            let s = step(&mut m, &mut counties, &realms, &mut units, id).unwrap();
            assert_eq!(s.dwelling_burnt, None, "{kind:?} owner {unit_owner} terrain {t:#04x}");
            assert_eq!(m.terrain_at(11, 10), t);
            assert_eq!(counties[1].population, 400);
        }
    }

    #[test]
    fn the_quarter_truncates_towards_zero_like_the_originals_shift() {
        for (before, after) in [(0i32, 0i32), (3, 3), (4, 3), (7, 6)] {
            let mut m = open_map();
            m.set_flags(11, 10, flags::PLOT);
            m.set_terrain(11, 10, terrain::DWELLING);
            let (mut counties, realms) = blank();
            counties[1].owner = 1;
            counties[1].population = before;
            let mut units = Units::new();
            let id = army_at(&mut units, 2, 10, 10);
            units.get_mut(id).unwrap().path = vec![(11, 10)];
            step(&mut m, &mut counties, &realms, &mut units, id).unwrap();
            assert_eq!(counties[1].population, after, "from {before}");
        }
    }


    #[test]
    fn the_entry_classification_tests_the_bits_in_the_originals_order() {
        let mut m = open_map();
        let units = Units::new();
        let cases: [(u8, u8, Entry); 6] = [
            (flags::ROAD | flags::FARMLAND, 0, Entry::Road),
            (flags::CASTLE, 0, Entry::Castle),
            (flags::SETTLEMENT, 0, Entry::Settlement),
            (flags::PLOT, 0, Entry::Plot),
            (flags::FARMLAND, 10, Entry::Field),
            (0, 0, Entry::Open),
        ];
        for (f, t, want) in cases {
            m.set_flags(5, 5, f);
            m.set_terrain(5, 5, t);
            assert_eq!(try_enter(&m, &units, 5, 5), want, "flags {f:#04x}");
        }
    }

    #[test]
    fn the_county_town_is_walked_into_rather_than_trampled() {
        let mut m = open_map();
        m.set_flags(5, 5, flags::SETTLEMENT);
        m.set_terrain(5, 5, terrain::TOWN);
        assert_eq!(try_enter(&m, &Units::new(), 5, 5), Entry::Open);
        assert_eq!(m.cost_map().at(5, 5), 3, "and the cost map agrees");
    }

    #[test]
    fn a_unit_in_the_way_stops_the_move_without_charging_anything() {
        let mut m = open_map();
        let (mut counties, realms) = blank();
        let mut units = Units::new();
        let mover = army_at(&mut units, 1, 10, 10);
        let blocker = army_at(&mut units, 2, 11, 10);
        units.get_mut(mover).unwrap().path = vec![(11, 10), (12, 10)];
        let s = step(&mut m, &mut counties, &realms, &mut units, mover).unwrap();
        assert_eq!(s.entry, Entry::Occupied(blocker));
        assert_eq!(s.charged, 0);
        assert!(!s.moved);
        assert_eq!(units.get(mover).unwrap().tile(), (10, 10));
    }

    #[test]
    fn a_merchant_walks_through_whatever_is_standing_in_its_way() {
        for (kind, blocker_kind, through) in [
            (UnitKind::Merchant, UnitKind::Army, true),
            (UnitKind::Transport, UnitKind::Army, true),
            (UnitKind::Army, UnitKind::Merchant, true),
            (UnitKind::PeasantMob, UnitKind::Merchant, true),
            (UnitKind::Army, UnitKind::Army, false),
            (UnitKind::Army, UnitKind::PeasantMob, false),
        ] {
            let mut m = open_map();
            let (mut counties, realms) = blank();
            let mut units = Units::new();
            let mover = units.spawn(Unit::new(kind, 1, 10, 10)).unwrap();
            units.spawn(Unit::new(blocker_kind, 2, 11, 10)).unwrap();
            units.get_mut(mover).unwrap().path = vec![(11, 10), (12, 10)];
            let s = step(&mut m, &mut counties, &realms, &mut units, mover).unwrap();
            assert_eq!(
                s.moved, through,
                "{kind:?} meeting {blocker_kind:?}: entry was {:?}",
                s.entry
            );
            if through {
                assert_eq!(s.entry, Entry::Open);
                assert_eq!(s.charged, crate::tables::STEP_COST_OPEN);
                assert_eq!(units.get(mover).unwrap().tile(), (11, 10));
            }
        }
    }

    #[test]
    fn passing_through_on_a_road_costs_a_road_step() {
        let mut m = open_map();
        m.set_flags(11, 10, flags::ROAD);
        let (mut counties, realms) = blank();
        let mut units = Units::new();
        let mover = units.spawn(Unit::new(UnitKind::Merchant, 1, 10, 10)).unwrap();
        units.spawn(Unit::new(UnitKind::Merchant, 6, 11, 10)).unwrap();
        units.get_mut(mover).unwrap().path = vec![(11, 10)];
        let s = step(&mut m, &mut counties, &realms, &mut units, mover).unwrap();
        assert_eq!(s.entry, Entry::Road);
        assert_eq!(s.charged, crate::tables::STEP_COST_ROAD);
        assert!(units.get(mover).unwrap().on_road);
    }

    #[test]
    fn a_unit_on_a_castle_tile_hides_it_from_a_merchant() {
        let mut m = open_map();
        m.set_flags(11, 10, flags::CASTLE);
        let (mut counties, realms) = blank();
        let mut units = Units::new();
        let mover = units.spawn(Unit::new(UnitKind::Merchant, 1, 10, 10)).unwrap();
        units.spawn(Unit::new(UnitKind::Army, 2, 11, 10)).unwrap();
        units.get_mut(mover).unwrap().path = vec![(11, 10)];
        let s = step(&mut m, &mut counties, &realms, &mut units, mover).unwrap();
        assert_eq!(s.entry, Entry::Open);
        assert_eq!(s.reached_castle, None);
    }

    #[test]
    fn crossing_a_border_is_reported_once_and_updates_the_unit() {
        let mut m = open_map();
        for y in 0..MAP_DIM as u8 {
            for x in 12..MAP_DIM as u8 {
                m.set_county(x, y, 2);
            }
        }
        let (mut counties, realms) = blank();
        let mut units = Units::new();
        let id = army_at(&mut units, 1, 10, 10);
        order_move(&m, &mut units, id, (14, 10), Routing::Direct);
        let steps = march(&mut m, &mut counties, &realms, &mut units, id);
        let crossings: Vec<_> = steps.iter().filter_map(|s| s.entered_county).collect();
        assert_eq!(crossings, vec![2], "one crossing, at x = 12");
        assert_eq!(units.get(id).unwrap().county, 2);
    }

    #[test]
    fn a_merchant_pays_no_field_surcharge_and_wrecks_nothing() {
        let mut m = open_map();
        m.set_flags(11, 10, flags::FARMLAND);
        m.set_terrain(11, 10, 10);
        let (mut counties, realms) = blank();
        counties[1].owner = 1;
        counties[1].fields_grain = 4;
        counties[1].crop[1] = 400;

        let mut units = Units::new();
        let mut trader = Unit::new(UnitKind::Merchant, 2, 10, 10);
        trader.county = 1;
        let id = units.spawn(trader).unwrap();
        units.get_mut(id).unwrap().path = vec![(11, 10)];

        let s = step(&mut m, &mut counties, &realms, &mut units, id).unwrap();
        assert_eq!(s.charged, 3, "the general step only");
        assert_eq!(counties[1].fields_grain, 4);
        assert_eq!(counties[1].crop[1], 400);
    }
}


