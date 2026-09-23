#![allow(unused_imports)]
use super::*;



    /// `g_goodsPrice[1]` is 2 and `Merchant_SpawnAll` gives every merchant a
    /// morale of 100, so `Ai_BuyGood` quotes `2 + max(1, Pct(2, 100))`. The
/// number is pinned from the table
    /// expression the code under test uses — `docs/agents.md`, *compute the
    /// probe from the constant you are ablating*.
    #[test]
    fn a_sack_of_grain_costs_four_crowns_at_the_countys_own_stall() {
        let (c, _, units) = neutral_with_purse(0);
        let s = stall(T, &c, &units);
        assert_eq!(s.price(Good::Grain, 100), 4, "base 2 doubled by a morale of 100");
        assert_eq!(s.price(Good::Grain, 0), 3, "the one-crown markup floor");
        assert_eq!(s.price(Good::Grain, 200), 6);
    }

    #[test]
    fn the_lot_an_unowned_county_takes_is_the_largest_its_purse_covers() {
        for (purse, expect_lot, why) in [
            (195, 0, "one turn earlier, 195 crowns buys nothing at all"),
            (200, 50, "exactly the 50-sack bill"),
            (316, 50, "the fixture's own purse"),
            (399, 50, "still short of the 100-sack lot's 400"),
            (400, 100, "and the next lot up opens at exactly 400"),
            (1600, 400, "the top of the cascade"),
        ] {
            let (mut c, mut map, units) = neutral_with_purse(purse);
            let before = c.grain;
            let mut m = stall(T, &c, &units);
            run_buys(FarmStyle::NeutralGrazing, 1, &mut c, &mut map, &mut m);
            assert_eq!(c.grain - before, expect_lot, "purse {purse}: {why}");
            assert_eq!(c.purse, purse - expect_lot * 4, "purse {purse}: {why}");
        }
    }

    /// *Ablation*: delete the `if county.merchant_count == 0 { continue }` in
    /// [`CountyStall::new`] and this goes red — the county has a purse of 1,000
    /// and would buy the 200-sack lot.
    #[test]
    fn a_county_with_no_merchant_in_it_cannot_buy_at_any_price() {
        let (mut c, mut map, units) = neutral_with_purse(1_000);
        c.merchant_count = 0;
        let mut m = stall(T, &c, &units);
        run_buys(FarmStyle::NeutralGrazing, 1, &mut c, &mut map, &mut m);
        assert_eq!(c.grain, 57, "the stall gate refuses before the purse is even read");
        assert_eq!(c.purse, 1_000);
    }

    /// `Ration_Apply` (`0x0044DF5F`) has no store `-=` anywhere in it, and
    /// `crate::trade::settle_county` used to call the debiting `ration::apply`
    /// twice —
    ///
    /// *Ablation*: put either `ration::preview` in `settle_county` back to
    /// `ration::apply` and this goes red, because the store loses two meals it
    /// should not. **The herd is cut to 11 on purpose**: the first draft of
    /// this test left the fixture's 135 head in place, the dairy alone fed the
    /// county, no grain was ever eaten,
    /// test that could not have failed. `docs/agents.md`, *ablate something the
    /// check claims*.
    #[test]
    fn buying_grain_does_not_make_the_county_eat_it() {
        let (mut c, mut map, units) = neutral_with_purse(400);
        c.herd = 11;
        c.ration_wanted = 3;
        c.ration_split = 0;
        let mut m = stall(T, &c, &units);
        // Establish that this county DOES eat grain, or the assertion below is
        // about nothing. `ration::apply` prices the meal and shadows it into
        // `+0x18C`; `land::grain_season_tick` is what takes it out of the
        // granary.
        let mut control = c.clone();
        crate::ration::apply(T, &mut control, true, crate::ration::Sowing::NONE);
        assert!(
            control.grain_eaten_shadow > 0,
            "the fixture must actually eat grain for this to test anything"
        );

        run_buys(FarmStyle::NeutralGrazing, 1, &mut c, &mut map, &mut m);
        assert_eq!(
            c.grain, 157,
            "57 in store plus the 100-sack lot, with nothing eaten on the way through"
        );
    }

    #[test]
    fn the_arable_realm_style_tops_up_to_six_hundred_then_to_one_hundred() {
        let (mut c, mut cmap) = county_with(4);
        c.grain = 150;
        let mut m = ledger(false);
        run_buys(FarmStyle::RealmArable, 1, &mut c, &mut cmap, &mut m);
        assert_eq!(
            m.asked,
            vec![(400, Good::Grain), (200, Good::Grain), (100, Good::Grain)],
            "150 sacks is under 600 but not under 100"
        );

        let (mut c, mut cmap) = county_with(4);
        c.grain = 50;
        let mut m = ledger(false);
        run_buys(FarmStyle::RealmArable, 1, &mut c, &mut cmap, &mut m);
        assert_eq!(m.asked.len(), 5, "under 100, the two small lots are tried too");
    }

    #[test]
    fn only_the_grazing_realm_style_tops_its_herd_up_to_forty() {
        for (style, floor) in [
            (FarmStyle::NeutralGrazing, 11),
            (FarmStyle::RealmGrazing, 41),
            (FarmStyle::RealmMixed, 11),
        ] {
            let line = style.buys()[0];
            assert_eq!((line.good, line.floor, line.lot), (Good::Cattle, floor, 10), "{style:?}");
        }
        for style in [FarmStyle::NeutralArable, FarmStyle::RealmArable] {
            assert!(style.buys().iter().all(|l| l.good == Good::Grain), "{style:?}");
        }
    }

    #[test]
    fn the_unowned_county_is_given_a_hundred_crowns_to_shop_with() {
        let (mut c, _) = county_with(4);
        c.grain = 0;
        assert_eq!(neutral_purse_top_up(FarmStyle::NeutralArable, &c), 100);
        c.grain = 100;
        assert_eq!(neutral_purse_top_up(FarmStyle::NeutralArable, &c), 0);
        c.grain = 0;
        for style in [FarmStyle::NeutralGrazing, FarmStyle::RealmArable, FarmStyle::RealmMixed] {
            assert_eq!(neutral_purse_top_up(style, &c), 0, "{style:?}");
        }
    }

    #[test]
    fn a_style_runs_to_completion_against_a_market_that_refuses_everything() {
        let (mut c, mut map) = county_with(8);
        c.owner = 2;
        c.herd = 40;
        lay(FarmStyle::RealmArable, &mut c, &mut map, &env());
        assert_eq!(c.industry_share, 50);
        assert!(counts(&c, &map).0 > 0, "it still planted");
    }


    #[test]
    fn the_arable_style_plants_half_the_county_in_winter() {
        let (mut c, mut map) = county_with(8);
        c.herd = 50;
        c.fertility = 0;
        lay(FarmStyle::RealmArable, &mut c, &mut map, &env());
        let (g, p, _) = counts(&c, &map);
        assert_eq!(g, 4, "half of eight");
        assert_eq!(p, 1, "and exactly one pasture, because the herd is over ten");
    }

    #[test]
    fn an_arable_lord_gives_a_small_herd_no_pasture_whatever() {
        let (mut c, mut map) = county_with(8);
        c.herd = 10;
        map.terrain[c.field_tile(0).unwrap()] = terrain::PASTURE;
        crate::field::recount(&mut c, &map);
        assert_eq!(c.fields_cattle, 1);
        lay(FarmStyle::RealmArable, &mut c, &mut map, &env());
        assert_eq!(counts(&c, &map).1, 0, "the pasture was cleared and not replaced");
    }

    #[test]
    fn a_grazing_lord_clears_grain_in_every_season() {
        for season in Season::ALL {
            let (mut c, mut map) = county_with(6);
            for slot in 0..3 {
                map.terrain[c.field_tile(slot).unwrap()] = terrain::GRAIN;
            }
            crate::field::recount(&mut c, &map);
            assert_eq!(c.fields_grain, 3);
            c.herd = 200;
            c.herd_crowding = 50;
            let mut e = env();
            e.season = season;
            lay(FarmStyle::RealmGrazing, &mut c, &mut map, &e);
            assert_eq!(counts(&c, &map).0, 0, "no grain in {}", season.name());
        }
    }

    #[test]
    fn a_grazing_lord_adds_one_pasture_a_pass_and_stops_one_short() {
        let (mut c, mut map) = county_with(5);
        c.owner = 2;
        c.herd = 400;
        let mut seen = Vec::new();
        for _ in 0..8 {
            c.herd_crowding = crate::land::herd_crowding(T, c.herd, c.fields_cattle);
            lay(FarmStyle::RealmGrazing, &mut c, &mut map, &env());
            seen.push(counts(&c, &map).1);
        }
        assert_eq!(seen, vec![1, 2, 3, 4, 4, 4, 4, 4], "one a pass, capped at total - 1");
    }

    #[test]
    fn the_mixed_style_gives_a_third_to_each() {
        let (mut c, mut map) = county_with(9);
        c.owner = 2;
        c.herd = 400;
        c.fertility = 0;
        for _ in 0..5 {
            c.herd_crowding = crate::land::herd_crowding(T, c.herd, c.fields_cattle);
            lay(FarmStyle::RealmMixed, &mut c, &mut map, &env());
        }
        let (g, p, _) = counts(&c, &map);
        assert_eq!(g, 3, "a third of nine in grain");
        assert_eq!(p, 3, "and a third in pasture");
    }

    #[test]
    fn the_fertility_ladders_middle_rung_can_never_be_reached() {
        for fertility in -100..=100 {
            let q = winter_grain_quota(fertility, 10);
            assert_ne!(q, 8, "fertility {fertility} reached the unreachable rung");
            assert_eq!(q, if fertility < -20 { 9 } else { 10 });
        }
    }

    #[test]
    fn switching_advanced_farming_off_makes_the_ai_plant_far_more_grain() {
        let plant = |style: FarmStyle, advanced: bool| {
            let (mut c, mut map) = county_with(9);
            c.herd = 0;
            c.fertility = 0;
            let mut e = env();
            e.advanced_farming = advanced;
            lay(style, &mut c, &mut map, &e);
            counts(&c, &map).0
        };
        assert_eq!((plant(FarmStyle::RealmArable, true), plant(FarmStyle::RealmArable, false)), (4, 8));
        assert_eq!((plant(FarmStyle::RealmMixed, true), plant(FarmStyle::RealmMixed, false)), (3, 4));
        assert_eq!(
            (plant(FarmStyle::NeutralArable, true), plant(FarmStyle::NeutralArable, false)),
            (4, 6),
            "the neutral pass keeps three fields back where the realm one keeps one"
        );
    }

    #[test]
    fn an_arable_lord_leaves_a_standing_crop_alone_outside_winter() {
        for season in [Season::Spring, Season::Summer, Season::Autumn] {
            let (mut c, mut map) = county_with(8);
            for slot in 0..6 {
                map.terrain[c.field_tile(slot).unwrap()] = terrain::GRAIN + 3;
            }
            crate::field::recount(&mut c, &map);
            c.herd = 0;
            let mut e = env();
            e.season = season;
            lay(FarmStyle::RealmArable, &mut c, &mut map, &e);
            assert_eq!(counts(&c, &map).0, 6, "{} left the crop standing", season.name());
        }
    }


    #[test]
    fn a_herd_county_is_fed_worse_than_a_grain_county_with_the_same_food() {
        let mut c = County::new();
        c.population = 100;
        c.herd = 57;
        c.grain = 0;
        assert_eq!(food_in_store(T, &c), 855);
        assert_eq!(ration_wanted(T, &c, false), 4, "Double: the dairy rung got there first");

        c.herd = 0;
        c.grain = 143;
        assert_eq!(food_in_store(T, &c), 858);
        assert_eq!(ration_wanted(T, &c, false), 5, "Triple, on the same food as bread");
    }

    ///: `store` counts
    #[test]
    fn the_triple_dairy_rung_is_redundant_at_every_herd_and_population() {
        let with_rung = |c: &County| ration_wanted(T, c, false) == 5;
        for people in [1i32, 7, 100, 999, 5_000] {
            for herd in [1i32, 10, 61, 400, 3_000] {
                let mut c = County::new();
                c.population = people;
                c.herd = herd;
                let dairy = ration::food_from_dairy(T, herd);
                if dairy > people * 3 {
                    assert!(
                        with_rung(&c),
                        "the rung fired at people {people}, herd {herd}"
                    );
                    assert!(
                        food_in_store(T, &c) > people * 8,
                        "and the store rung below would have said 5 anyway"
                    );
                }
            }
        }
    }

    #[test]
    fn the_ration_ladder_descends_with_the_larder() {
        let want = |grain: i32| {
            let mut c = County::new();
            c.population = 600;
            c.grain = grain;
            ration_wanted(T, &c, false)
        };
        assert_eq!(want(0), 0, "nothing at all");
        assert_eq!(want(49), 0, "294 < 300");
        assert_eq!(want(50), 1, "300 is not under 300");
        assert_eq!(want(100), 2, "600 clears the population");
        assert_eq!(want(200), 3, "1200 clears twice it");
        assert_eq!(want(601), 4, "3606 > 3600");
        assert_eq!(want(801), 5, "4806 > 4800");
    }

    #[test]
    fn the_two_split_searches_take_the_opposite_end_of_a_tie() {
        let mut base = County::new();
        base.population = 200;
        base.herd = 300;
        base.grain = 300;

        let mut low = base.clone();
        set_rations(T, &mut low, SplitSearch::PreferGrain, false, crate::ration::Sowing::NONE);
        let mut high = base.clone();
        set_rations(T, &mut high, SplitSearch::PreferHerd, false, crate::ration::Sowing::NONE);

        assert_eq!(low.ration_wanted, high.ration_wanted);
        assert!(low.ration_split < high.ration_split, "{} < {}", low.ration_split, high.ration_split);
        assert_eq!(low.ration_achieved, high.ration_achieved, "and both feed the county the same");
    }

    /// **`Ai_SetRations` (`0x004A4782`) rations against *next* season.** Its
    /// three calls are `Ration_Apply(county,g_seasonNext)` (`004a0000.c:1794`,
    /// `1805`, `1816`), and `Ration_Apply`'s seed reservation is gated
    /// `if (season == 4)` — so an AI county holds its seed back the turn it
    /// *faces* Winter, not the turn it is in Winter.
    ///
    /// *Ablation*: pass `env.season` at `layout.rs`'s `set_rations` call and
    /// the two rows swap.
    #[test]
    fn the_ai_holds_its_seed_back_the_turn_it_faces_winter() {
        let available = |season, season_next| {
            let (mut c, mut map) = county_with(8);
            for i in 0..8u8 {
                map.terrain[crate::map::index(i, 8)] = terrain::GRAIN;
            }
            crate::field::recount(&mut c, &map);
            c.grain = 400;
            c.labour_useful[T.job.grain_farming] = 5_000; // labour never binds
            let mut e = FarmEnv::new(season, season_next);
            e.advanced_farming = false;
            lay(FarmStyle::RealmMixed, &mut c, &mut map, &e);
            c.grain_available
        };
        assert_eq!(available(Season::Winter, Season::Spring), 400, "in Winter, none held");
        assert_eq!(available(Season::Autumn, Season::Winter), 352, "facing Winter: 400 - 48");
    }

    #[test]
    fn a_fixed_split_is_used_as_it_stands() {
        let mut c = County::new();
        c.population = 100;
        c.grain = 500;
        set_rations(T, &mut c, SplitSearch::Fixed(37), false, crate::ration::Sowing::NONE);
        assert_eq!(c.ration_split, 37);
    }

    #[test]
    fn only_an_arable_lord_with_a_hundred_head_spare_eats_the_herd_first() {
        let mut c = County::new();
        c.herd = 100;
        assert_eq!(split_search(FarmStyle::RealmArable, &c), SplitSearch::PreferGrain);
        c.herd = 101;
        assert_eq!(split_search(FarmStyle::RealmArable, &c), SplitSearch::PreferHerd);
        assert_eq!(split_search(FarmStyle::NeutralArable, &c), SplitSearch::PreferHerd);
        for style in [FarmStyle::RealmGrazing, FarmStyle::NeutralGrazing, FarmStyle::RealmMixed] {
            assert_eq!(split_search(style, &c), SplitSearch::PreferGrain, "{style:?}");
        }
    }

    #[test]
    fn the_ai_food_total_counts_the_herd_twice_and_ignores_the_season_caps() {
        let mut c = County::new();
        c.herd = 10;
        c.grain = 10;
        c.herd_available = 0;
        c.grain_available = 0;
        assert_eq!(food_in_store(T, &c), 10 * 5 + 10 * 10 + 10 * 6);
        assert_eq!(
            crate::ration::food_available(T, &c),
            10 * 5,
            "the sibling reads the caps and sees only the dairy"
        );
    }


    #[test]
    fn every_style_takes_the_castle_favouring_default_shares() {
        use crate::tables::{JOB_BLACKSMITH, JOB_CASTLE_BUILDING, JOB_FIELD_RECLAMATION};
        let mut c = County::new();
        default_shares_built(&mut c);
        assert_eq!(&c.labour_share[0..3], &[33, 50, 17]);
        assert_eq!(&c.labour_share[3..8], &[40, 15, 15, 15, 15]);
        assert_eq!(c.labour_share[0..=JOB_FIELD_RECLAMATION].iter().sum::<i32>(), 100);
        assert_eq!(c.labour_share[JOB_CASTLE_BUILDING..=JOB_BLACKSMITH].iter().sum::<i32>(), 100);
    }

    #[test]
    fn the_arable_realm_style_puts_half_the_county_into_industry() {
        assert_eq!(FarmStyle::RealmArable.industry_share(), 50);
        assert_eq!(FarmStyle::RealmMixed.industry_share(), 40);
        assert_eq!(FarmStyle::RealmGrazing.industry_share(), 20);
        assert_eq!(FarmStyle::NeutralArable.industry_share(), 0);
        assert_eq!(FarmStyle::NeutralGrazing.industry_share(), 0);
    }


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

