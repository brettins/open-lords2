#![allow(unused_imports)]
use super::*;
use super::styles::*;
use super::fields::*;
use super::super::*;

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


