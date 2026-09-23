#![allow(unused_imports)]
use super::*;
use super::feeding::*;
use super::fields::*;
use super::super::*;

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

