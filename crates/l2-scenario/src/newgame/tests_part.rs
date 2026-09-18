#![allow(unused_imports)]
use super::*;
use super::build_part::*;
use super::sites::*;
use super::fields::*;
use super::setup::*;
use super::scenario::*;
use l2_formats::maps::{MapSlot, Plane, PLANE_DIM};
use l2_kingdom::county::{MAX_COUNTIES, MAX_COUNTY_ID, MAX_FIELDS, MAX_NEIGHBOURS};
use l2_kingdom::map::{CampaignMap, MAP_TILES};
use l2_kingdom::merchant::{self, MerchantRoutes, ROUTES, ROUTE_SLOTS};
use l2_kingdom::mercenary::MercenaryBands;
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::{Weather, JOB_COUNT};
use l2_kingdom::unit::{Unit, UnitKind, Units};
use l2_kingdom::Options;
use crate::{Clock, CountyState, IndustryState, RealmState, Scenario};

#[cfg(test)]
mod tests {
    use super::*;
    use l2_formats::maps::{MapSet, PLANE_LEN, SLOT_LEN};
    use l2_kingdom::map::index;

    const BANK_BASE: u8 = 0x00;

    fn one_county_slot() -> Vec<u8> {
        let mut buf = vec![0u8; SLOT_LEN];
        let put = |buf: &mut Vec<u8>, plane: Plane, x: usize, y: usize, v: u8| {
            buf[plane as usize * PLANE_LEN + y * PLANE_DIM + x] = v;
        };
        for y in 0..PLANE_DIM {
            for x in 0..PLANE_DIM {
                put(&mut buf, Plane::County, x, y, 1);
            }
        }
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            put(&mut buf, Plane::Flags, 10 + dx, 10 + dy, bit::TOWN);
            put(&mut buf, Plane::GfxBank, 10 + dx, 10 + dy, BANK_TOWN);
        }
        put(&mut buf, Plane::Marker, 10, 10, 1);
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            put(&mut buf, Plane::Flags, 20 + dx, 20 + dy, bit::SITE);
            put(&mut buf, Plane::GfxBank, 20 + dx, 20 + dy, BANK_BASE);
            put(&mut buf, Plane::GfxIndex, 20 + dx, 20 + dy, 6);
        }
        put(&mut buf, Plane::Marker, 20, 20, 1);
        put(&mut buf, Plane::Flags, 30, 30, bit::SITE);
        put(&mut buf, Plane::GfxBank, 30, 30, BANK_TOWN);
        put(&mut buf, Plane::GfxIndex, 30, 30, FRAME_MINE);
        for i in 0..4 {
            put(&mut buf, Plane::Flags, 40 + i, 40, bit::PLOT);
            put(&mut buf, Plane::GfxIndex, 40 + i, 40, 7 + i as u8);
        }
        for i in 0..2 {
            put(&mut buf, Plane::Flags, 12 + i, 12, bit::FARM);
            put(&mut buf, Plane::GfxBank, 12 + i, 12, BANK_ROADS);
            put(&mut buf, Plane::GfxIndex, 12 + i, 12, 80);
        }
        buf
    }

    fn world() -> MapWorld {
        let buf = one_county_slot();
        let set = MapSet::parse(&buf).unwrap();
        let slot = set.slot(0).unwrap();
        build(&slot, &NewGame::default()).unwrap()
    }

    #[test]
    fn the_town_gives_the_anchor_and_the_castle_gives_the_plot() {
        let w = world();
        assert_eq!(w.county_count, 1);
        assert_eq!(w.anchor[1], (11, 11));
        assert_eq!(w.town_tile[1], index(10, 10));
        assert_eq!(w.castle[1], (20, 20));
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            assert_eq!(w.tiles.content[index(20 + dx, 20 + dy)], TERRAIN_CASTLE_PLOT);
        }
    }

    #[test]
    fn the_mine_is_read_off_the_map_and_the_blacksmith_is_derived() {
        let w = world();
        assert!(w.has_resource[1][IND_IRON], "frame 30 on the Town bank is iron");
        assert_eq!(w.tiles.content[index(30, 30)], TERRAIN_IRON);
        assert!(!w.has_resource[1][IND_STONE]);
        assert!(!w.has_resource[1][IND_WOOD]);
        assert!(w.has_resource[1][IND_WEAPONS]);
        let smith = w.industry_site[1][IND_WEAPONS];
        assert_eq!(w.tiles.content[smith], TERRAIN_WEAPONS);
        assert_ne!(w.tiles.flags[smith] & bit::SITE, 0);
    }

    #[test]
    fn the_dwelling_plots_keep_the_ground_they_covered() {
        let w = world();
        assert_eq!(
            w.dwelling_plots[1],
            [index(40, 40), index(41, 40), index(42, 40), index(43, 40)]
        );
        for i in 0..4usize {
            let t = index(40 + i as u8, 40);
            assert_eq!(w.tiles.saved_frame[t], 7 + i as u8, "the terrain frame is remembered");
            assert_eq!(w.tiles.content[t], 0);
        }
    }

    #[test]
    fn the_two_fields_become_pasture_and_land_in_the_field_table() {
        let w = world();
        for i in 0..2u8 {
            assert_eq!(w.tiles.content[index(12 + i, 12)], 0x14);
            assert_eq!(w.tiles.frame[index(12 + i, 12)] & !3, 104);
        }
        assert_eq!(w.field_tiles[1][0], index(12, 12) as u16);
        assert_eq!(w.field_tiles[1][1], index(13, 12) as u16);
        assert_eq!(w.field_tiles[1][2], 0);
        assert_eq!(w.farm_tile_count[1], 2);
    }

    #[test]
    fn a_twenty_first_field_is_razed_rather_than_dropped() {
        let mut buf = one_county_slot();
        for i in 0..25usize {
            let (x, y) = (i % 5 + 20, i / 5 + 40);
            buf[Plane::Flags as usize * PLANE_LEN + y * PLANE_DIM + x] = bit::FARM;
            buf[Plane::GfxBank as usize * PLANE_LEN + y * PLANE_DIM + x] = BANK_ROADS;
            buf[Plane::GfxIndex as usize * PLANE_LEN + y * PLANE_DIM + x] = 80;
        }
        let set = MapSet::parse(&buf).unwrap();
        let w = build(&set.slot(0).unwrap(), &NewGame::default()).unwrap();
        assert_eq!(w.farm_tile_count[1], 27, "two originals plus twenty-five");
        let used = w.field_tiles[1].iter().filter(|&&t| t != 0).count();
        assert_eq!(used, MAX_FIELDS, "the table fills");
        let razed = (0..MAP_TILES)
            .filter(|&t| w.tiles.frame[t] == FRAME_GRASS && w.tiles.flags[t] == 0)
            .filter(|&t| {
                let (x, y) = (t % PLANE_DIM, t / PLANE_DIM);
                (20..25).contains(&x) && (40..45).contains(&y)
            })
            .count();
        assert_eq!(razed, 7, "27 fields, 20 slots");
    }

    #[test]
    fn the_difficulty_ladder_is_the_whole_of_its_effect_on_the_land() {
        let mut buf = one_county_slot();
        for i in 0..16usize {
            let (x, y) = (i + 20, 50);
            buf[Plane::Flags as usize * PLANE_LEN + y * PLANE_DIM + x] = bit::FARM;
            buf[Plane::GfxBank as usize * PLANE_LEN + y * PLANE_DIM + x] = BANK_ROADS;
            buf[Plane::GfxIndex as usize * PLANE_LEN + y * PLANE_DIM + x] = 80;
        }
        let set = MapSet::parse(&buf).unwrap();
        let slot = set.slot(0).unwrap();
        let counts = |difficulty: u8| {
            let mut setup = NewGame::default();
            setup.options.difficulty = difficulty;
            let w = build(&slot, &setup).unwrap();
            let mut pasture = 0;
            let mut fallow = 0;
            let mut wild = 0;
            for t in w.field_tiles[1].iter().filter(|&&t| t != 0) {
                match w.tiles.content[*t as usize] {
                    0x14 => pasture += 1,
                    1 => fallow += 1,
                    0 => wild += 1,
                    other => panic!("terrain {other}"),
                }
            }
            (pasture, fallow, wild)
        };
        assert_eq!(counts(0), (8, 10, 0), "easiest: eight pasture, the rest fallow");
        assert_eq!(counts(1), (6, 2, 10));
        assert_eq!(counts(2), (4, 2, 12));
        assert_eq!(counts(3), (4, 0, 14), "hardest: four pasture and nothing else sown");
    }

    #[test]
    fn a_map_with_no_county_is_refused_rather_than_half_loaded() {
        let buf = vec![0u8; SLOT_LEN];
        let set = MapSet::parse(&buf).unwrap();
        assert_eq!(build(&set.slot(0).unwrap(), &NewGame::default()), Err(MapError::NoCounties));
    }

    #[test]
    fn more_lords_than_the_map_seats_is_refused() {
        let w = world();
        let setup = NewGame { lords: 3, ..NewGame::default() };
        assert_eq!(
            Scenario::from_map_world(&w, &setup),
            Err(MapError::TooManyLords { lords: 3, seats: 1 })
        );
    }

    #[test]
    fn the_start_county_is_owned_switched_on_and_building() {
        let w = world();
        let setup = NewGame { lords: 1, ..NewGame::default() };
        let s = Scenario::from_map_world(&w, &setup).unwrap();
        let c = s.counties[1].as_ref().unwrap();
        assert_eq!(c.owner, 1);
        assert!(c.castle_switch, "the castle switch opens on");
        assert!(c.industry[IND_IRON].enabled, "the first non-weapons resource is switched on");
        assert!(!c.industry[IND_WEAPONS].enabled, "record 2 is skipped");
        assert!(c.industry[IND_WEAPONS].has_resource, "…but the blacksmith is still there");
        assert_eq!(s.clock, Clock { season: 3, season_next: 4, year: 1267, turn_count: 0 });
    }

    #[test]
    fn the_lords_are_distinct_for_every_scenario_group_and_every_colour() {
        for slot in 0..8usize {
            for shield in 1..=5u8 {
                let a = assign_lords(&NewGame { slot, shield, ..NewGame::default() }, 5);
                let given: Vec<u8> = (1..=5).filter(|&r| r != 1).map(|r| a.lord[r]).collect();
                assert!(
                    given.iter().all(|&l| l != 0),
                    "slot {slot} shield {shield} left a realm lordless"
                );
                for (i, x) in given.iter().enumerate() {
                    for y in given.iter().skip(i + 1) {
                        assert_ne!(x, y, "slot {slot} shield {shield} gave one lord twice");
                    }
                }
                assert_eq!(a.lord[1], 0, "the person has no AI lord");
                assert_eq!(a.shield[1], shield, "the person did not get the colour they picked");
                let mut flown = (1..=5).map(|r| a.shield[r]).collect::<Vec<_>>();
                flown.sort_unstable();
                assert_eq!(flown, vec![1, 2, 3, 4, 5], "slot {slot} shield {shield}");
            }
        }
    }

    #[test]
    fn the_walk_hands_out_exactly_one_shield_per_lord() {
        let a = assign_lords(&NewGame { lords: 3, shield: 4, ..NewGame::default() }, 3);
        assert_eq!(a.shield[1], 4, "the person's own choice");
        assert_eq!(a.shield[2], 1, "the lowest colour nobody took");
        assert_eq!(a.shield[3], 2);
        assert_eq!(a.shield[4], 0, "out of play, and the walk says nothing about it");
        assert_eq!(a.shield[5], 0);
        assert_eq!(a.lord[4], 0);
    }

    #[test]
    fn a_shield_outside_the_five_is_refused() {
        let w = world();
        for bad in [0u8, 6, 255] {
            let setup = NewGame { lords: 1, shield: bad, ..NewGame::default() };
            assert_eq!(Scenario::from_map_world(&w, &setup), Err(MapError::Shield(bad)));
        }
        for good in 1..=5u8 {
            let setup = NewGame { lords: 1, shield: good, ..NewGame::default() };
            let s = Scenario::from_map_world(&w, &setup).expect("shield {good} builds");
            assert_eq!(s.realms[1].shield_index, good);
        }
    }

    #[test]
    fn the_labour_shares_sum_to_a_hundred_in_both_halves() {
        let c = county_reset(1);
        assert_eq!(c.labour_share[0..3].iter().sum::<i32>(), 100, "the farm half");
        assert_eq!(c.labour_share[3..].iter().sum::<i32>(), 100, "the industry half");
    }
}

