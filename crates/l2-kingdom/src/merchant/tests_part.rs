#![allow(unused_imports)]
use super::*;
use super::traversal::*;
use super::placement::*;
use crate::county::{County, MAX_COUNTIES};
use crate::map::{flags, CampaignMap};
use crate::movement;
use crate::unit::{UnitKind, Units};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::county::County;
    use crate::map::MAP_TILES;
    use crate::unit::Unit;

    /// England's route 0, straight out of `england-turn1.sav`.
    const ENGLAND_ROUTE_0: [u8; 5] = [14, 4, 7, 8, 2];

    fn routes() -> MerchantRoutes {
        let mut r = MerchantRoutes::none();
        r.set_route(0, &ENGLAND_ROUTE_0);
        r
    }

    fn counties() -> Box<[County; MAX_COUNTIES]> {
        Box::new(core::array::from_fn(|_| County::new()))
    }

    fn merchant_in(county: u8) -> Unit {
        let mut u = Unit::new(UnitKind::Merchant, 6, 8, 8);
        u.morale = 100;
        u.county = county;
        u
    }

    /// **The recount is the stall**, and `docs/symbols.md`'s own evidence for
    /// it is the shape this reproduces: the counties with a non-zero count are
    /// exactly the counties holding a merchant, the count reaches 2 where two
    /// share a county, and `merchant_unit` holds a live slot.
    ///
    /// *Ablation*: delete the `county.merchant_count += 1` and the first three
    /// assertions go red; delete the `merchant_count = 0` in the clear loop and
    /// the re-run assertion below goes red instead.
    #[test]
    fn the_recount_counts_merchants_and_only_merchants() {
        let mut c = counties();
        let mut units = Units::new();
        units.put(1, merchant_in(3));
        units.put(2, merchant_in(3));
        units.put(5, merchant_in(1));
        // An army standing in county 2 is not a stall.
        let mut army = Unit::new(UnitKind::Army, 1, 9, 9);
        army.county = 2;
        units.put(7, army);

        recount_all(&mut c, 4, &units);
        assert_eq!((c[1].merchant_count, c[1].merchant_unit), (1, 5));
        assert_eq!(c[2].merchant_count, 0, "an army is not a merchant");
        assert_eq!(
            (c[3].merchant_count, c[3].merchant_unit),
            (2, 2),
            "two merchants, and the HIGHER slot survives because the store overwrites"
        );

        // **The count is a snapshot, not an accumulator** — the clear loop is
        // why. Running it twice leaves the same counts.
        recount_all(&mut c, 4, &units);
        assert_eq!(c[3].merchant_count, 2, "the clear loop makes the pass idempotent");
        // The visit counter is the one thing that is NOT cleared, so it rises
        // by that pass's count every time. Two passes, two merchants: four.
        assert_eq!(c[3].merchant_visits, 4, "the lifetime counter is deliberately not cleared");
    }

    /// A merchant that walks away takes the stall with it.
    #[test]
    fn a_county_the_merchant_has_left_has_no_stall() {
        let mut c = counties();
        let mut units = Units::new();
        units.put(1, merchant_in(3));
        recount_all(&mut c, 4, &units);
        assert_eq!(c[3].merchant_count, 1);

        units.get_mut(1).expect("slot 1").county = 4;
        recount_all(&mut c, 4, &units);
        assert_eq!(c[3].merchant_count, 0, "the stall is where the merchant is standing");
        assert_eq!(c[4].merchant_count, 1);
    }

    /// The cursor starts at 1, so the first destination is the *second* county
    /// on the list — and the merchant does not immediately re-visit the county
    /// it was spawned in. `plane4.md` §2.3.
    #[test]
    fn the_first_destination_is_the_second_entry() {
        let mut cursor = 1;
        assert_eq!(next_destination(&routes(), 0, &mut cursor, 14), Some(4));
        assert_eq!(cursor, 2);
    }

    /// The whole circuit, twice round, including the eleven empty cells the
    /// cursor walks through to get back to entry 0.
    #[test]
    fn the_route_cycles_and_entry_zero_is_only_reached_by_wrapping() {
        let r = routes();
        let mut cursor = 1;
        let walked: Vec<u8> = (0..10)
            .filter_map(|_| next_destination(&r, 0, &mut cursor, 14))
            .collect();
        assert_eq!(walked, vec![4, 7, 8, 2, 14, 4, 7, 8, 2, 14]);
    }

    /// A county id above the map's count is absent, not an error — and a row
    /// of nothing but such ids yields nothing.
    #[test]
    fn a_county_above_the_map_count_is_skipped() {
        let mut r = MerchantRoutes::none();
        r.set_route(0, &[9, 3]);
        let mut cursor = 1;
        assert_eq!(next_destination(&r, 0, &mut cursor, 4), Some(3));

        r.set_route(1, &[9, 11]);
        let mut cursor = 0;
        assert_eq!(next_destination(&r, 1, &mut cursor, 4), None);
    }

    #[test]
    fn an_empty_row_yields_nothing() {
        let mut cursor = 1;
        assert_eq!(next_destination(&MerchantRoutes::none(), 3, &mut cursor, 14), None);
    }

    /// The road search grows its box to radius 3, so a road three tiles away is
    /// found — and **grass next door is not a candidate at all**
    /// `Merchant_AdvanceAll` never calls the open-ground fallback.
    #[test]
    fn the_road_search_reaches_three_tiles_and_ignores_grass() {
        let mut map = CampaignMap::empty();
        // Everything is grass except one road at distance 3.
        map.set_flags(20, 17, flags::ROAD);
        let units = Units::new();
        assert_eq!(find_free_road_tile(&map, &units, (20, 20)), Some((20, 17)));

        // With no road at all the road search fails outright; only the spawn's
        // fallback would have taken the grass.
        let plain = CampaignMap::empty();
        assert_eq!(find_free_road_tile(&plain, &units, (20, 20)), None);
        assert!(find_free_open_tile(&plain, &units, (20, 20)).is_some());
    }

    /// A tile with a unit on it is not free, whatever it is made of.
    #[test]
    fn an_occupied_road_is_not_free() {
        let mut map = CampaignMap::empty();
        map.set_flags(20, 19, flags::ROAD);
        map.set_flags(21, 19, flags::ROAD);
        let mut units = Units::new();
        units.put(1, Unit::new(UnitKind::Merchant, 6, 20, 19));
        assert_eq!(find_free_road_tile(&map, &units, (20, 20)), Some((21, 19)));
    }

    /// Rough ground and sea are not open ground: the fallback's test is
    /// `flags & !BOUNDARY == 0`, so only the boundary bit is forgiven.
    #[test]
    fn only_the_boundary_bit_is_forgiven_by_the_open_ground_fallback() {
        let mut map = CampaignMap::empty();
        for i in 0..MAP_TILES {
            map.flags[i] = flags::ROUGH;
        }
        map.set_flags(21, 21, flags::BOUNDARY);
        let units = Units::new();
        assert_eq!(find_free_open_tile(&map, &units, (20, 20)), Some((21, 21)));
    }

    /// The row a merchant reads is its slot minus one, and slots above 6 have
    /// no row at all — a seventh merchant would walk nothing.
    #[test]
    fn the_row_is_the_slot_minus_one() {
        assert_eq!(route_row_for(1), Some(0));
        assert_eq!(route_row_for(6), Some(5));
        assert_eq!(route_row_for(7), None);
        assert_eq!(route_row_for(0), None);
    }

    /// Two counties, a road down the middle of each, and a merchant standing on
    /// the western one.
    fn two_county_map() -> (CampaignMap, [County; MAX_COUNTIES]) {
        let mut map = CampaignMap::empty();
        for i in 0..MAP_TILES {
            map.county[i] = if i % crate::map::MAP_DIM < 32 { 1 } else { 2 };
        }
        for x in 0..64u8 {
            map.set_flags(x, 10, flags::ROAD);
        }
        let mut counties: [County; MAX_COUNTIES] = core::array::from_fn(|_| County::new());
        counties[1].anchor_x = 10;
        counties[1].anchor_y = 10;
        counties[2].anchor_x = 50;
        counties[2].anchor_y = 10;
        (map, counties)
    }

    /// End to end: a merchant with no destination is given one, and it is a
    /// road tile inside the next county on its route.
    #[test]
    fn an_idle_merchant_is_sent_to_the_next_county_on_its_route() {
        let (map, counties) = two_county_map();
        let mut units = Units::new();
        let mut m = Unit::new(UnitKind::Merchant, 6, 10, 10);
        m.needs_destination = true;
        m.year_formed = 1;
        units.put(1, m);

        let mut r = MerchantRoutes::none();
        r.set_route(0, &[1, 2]);

        assert_eq!(advance_all(&r, &map, &counties, &mut units, 2), 1);
        let u = units.get(1).unwrap();
        assert_eq!(u.dest_county, 2, "the second entry on the route");
        assert!(u.moving, "and it has been set walking");
        assert!(!u.path.is_empty());
        let dest = u.dest.unwrap();
        assert_eq!(map.county_at(dest.0, dest.1), 2);
        assert!(map.has(dest.0, dest.1, flags::ROAD), "and it is a road tile");
    }

    /// **A merchant part-way through a leg is re-pathed, not left alone.** The
    /// `needs_destination` test guards only the route lookup; every merchant
    /// with somewhere to go is set walking again. Without this a merchant that
    /// stopped for any reason would never move again, and phase 6 would settle
    /// on the first tick for ever.
    #[test]
    fn a_merchant_mid_leg_keeps_its_destination_and_is_set_walking_again() {
        let (map, counties) = two_county_map();
        let mut units = Units::new();
        let mut m = Unit::new(UnitKind::Merchant, 6, 30, 10);
        m.needs_destination = false;
        m.dest = Some((50, 10));
        m.dest_county = 2;
        m.year_formed = 9;
        m.moving = false;
        units.put(1, m);

        let mut r = MerchantRoutes::none();
        r.set_route(0, &[1, 2]);

        assert_eq!(advance_all(&r, &map, &counties, &mut units, 2), 1);
        let u = units.get(1).unwrap();
        assert_eq!(u.dest_county, 2, "the same leg");
        assert_eq!(u.year_formed, 9, "and the route cursor did not move");
        assert!(u.moving);
    }

    /// A county with no free road tile near its anchor is skipped and the
    /// merchant keeps its flag, so it tries again next season
    /// losing its place on the route.
    #[test]
    fn a_county_with_no_free_road_tile_is_skipped_and_retried() {
        let mut map = CampaignMap::empty();
        for i in 0..MAP_TILES {
            map.county[i] = 1;
        }
        let mut counties: [County; MAX_COUNTIES] = core::array::from_fn(|_| County::new());
        counties[1].anchor_x = 10;
        counties[1].anchor_y = 10;
        counties[2].anchor_x = 50;
        counties[2].anchor_y = 10;

        let mut units = Units::new();
        let mut m = Unit::new(UnitKind::Merchant, 6, 10, 10);
        m.needs_destination = true;
        m.year_formed = 1;
        units.put(1, m);

        let mut r = MerchantRoutes::none();
        r.set_route(0, &[1, 2]);

        assert_eq!(advance_all(&r, &map, &counties, &mut units, 2), 0);
        let u = units.get(1).unwrap();
        assert!(u.needs_destination, "still asking");
        assert!(!u.moving);
    }
}

