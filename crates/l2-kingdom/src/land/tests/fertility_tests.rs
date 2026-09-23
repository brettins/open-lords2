#![allow(unused_imports)]
use super::*;
use super::grain_tests::*;
use super::herd_tests::*;
use super::crowding::*;
use super::super::*;

/// **One fallow field per two grain fields is exactly break-even**, and
/// **cattle fields do not enter the formula at all.** Both contradict the
/// printed manual (`docs/decisions.md` C10).
#[test]
fn one_fallow_field_per_two_grain_fields_is_break_even() {
    for pairs in 1..=8 {
        let mut c = County::new();
        c.fields_fallow = pairs;
        c.fields_grain = pairs * 2;
        c.fields_cattle = 99; // deliberately absurd
        update_fertility(&mut c, true);
        assert_eq!(c.fertility, 0, "{pairs} fallow, {} grain", pairs * 2);
    }
}

#[test]
fn cattle_fields_are_invisible_to_fertility() {
    let mut with = County::new();
    with.fields_fallow = 4;
    with.fields_grain = 4;
    with.fields_cattle = 8;
    update_fertility(&mut with, true);

    let mut without = County::new();
    without.fields_fallow = 4;
    without.fields_grain = 4;
    update_fertility(&mut without, true);

    assert_eq!(with.fertility, without.fertility);
    assert_eq!(with.fertility, 12, "6*4 - 3*4");
}

#[test]
fn fertility_is_clamped_to_its_hundred_point_scale() {
    let mut c = County::new();
    c.fields_fallow = 20;
    for _ in 0..20 {
        update_fertility(&mut c, true);
    }
    assert_eq!(c.fertility, FERTILITY_MAX);

    let mut c = County::new();
    c.fields_grain = 20;
    for _ in 0..20 {
        update_fertility(&mut c, true);
    }
    assert_eq!(c.fertility, FERTILITY_MIN);
}

#[test]
fn basic_farming_pins_fertility_at_zero() {
    let mut c = County::new();
    c.fields_fallow = 10;
    c.fields_grain = 1;
    update_fertility(&mut c, false);
    assert_eq!(c.fertility, 0);
}

fn reclaiming(slots: &[(usize, u16)], workers: i32) -> (County, crate::map::CampaignMap) {
    let mut map = crate::map::CampaignMap::empty();
    let mut c = County::new();
    c.labour[crate::tables::JOB_FIELD_RECLAMATION] = workers;
    for slot in 0..MAX_FIELDS {
        c.field_tiles[slot] = slot as u16 + 1;
        map.terrain[slot + 1] = crate::field::terrain::FALLOW;
    }
    for &(slot, progress) in slots {
        c.field_progress[slot] = progress;
        map.terrain[slot + 1] = reclaim_terrain(T, progress as i32);
    }
    (c, map)
}

#[test]
fn only_fields_already_started_are_reclaimed() {
    let (mut c, mut map) = reclaiming(&[(1, 100)], 10_000);
    c.field_progress[0] = 0; // fallow tile: never started
    c.field_progress[2] = 800; // fallow tile: finished long ago
    reclaim_fields(T, &mut c, &mut map);
    assert_eq!(c.field_progress[0], 0);
    assert_eq!(c.field_progress[1], 300, "a quarter, and no more");
    assert_eq!(c.field_progress[2], 800);
}

#[test]
fn a_field_under_way_finishes_in_four_seasons_at_most() {
    let (mut c, mut map) = reclaiming(&[(5, 1)], 10_000);
    for _ in 0..4 {
        reclaim_fields(T, &mut c, &mut map);
    }
    assert!(c.field_progress[5] >= T.field.progress_max as u16);
    assert_eq!(
        map.terrain[6],
        crate::field::terrain::FALLOW,
        "and the tile becomes fallow, which is the reward"
    );
}

#[test]
fn a_county_with_nobody_on_reclamation_reclaims_nothing() {
    let (mut c, mut map) = reclaiming(&[(0, 100), (3, 400)], 0);
    reclaim_fields(T, &mut c, &mut map);
    assert_eq!(c.field_progress[0], 100);
    assert_eq!(c.field_progress[3], 400);

    c.labour[crate::tables::JOB_FIELD_RECLAMATION] = 50;
    reclaim_fields(T, &mut c, &mut map);
    assert_eq!(c.field_progress[3], 450, "the most advanced field goes first");
    assert_eq!(c.field_progress[0], 100, "and the other gets nothing");
}

#[test]
fn the_budget_walks_the_rota_from_the_leading_field() {
    let (mut c, mut map) = reclaiming(&[(0, 0), (7, 700)], 500);
    reclaim_fields(T, &mut c, &mut map);
    assert_eq!(c.field_progress[7], 900);
    assert_eq!(map.terrain[8], crate::field::terrain::FALLOW);
    assert_eq!(c.field_progress[0], 200);
}

#[test]
fn a_well_supplied_county_sows_ten_sacks_a_field_not_five() {
    assert_eq!(sacks_per_field(T, 6, 10_000, 10_000, true), 10);
    assert_eq!(sacks_per_field(T, 9, 10_000, 10_000, true), 10);
    assert_eq!(6 * sacks_per_field(T, 6, 10_000, 10_000, true), 60);
    assert_eq!(9 * sacks_per_field(T, 9, 10_000, 10_000, true), 90);
}

#[test]
fn the_seed_store_is_the_binding_constraint_when_labour_is_plentiful() {
    assert_eq!(sacks_per_field(T, 6, 30, 100_000, true), 5);
    assert_eq!(sacks_per_field(T, 6, 29, 100_000, true), 4);
    assert_eq!(sacks_per_field(T, 6, 5, 100_000, true), 0, "not even one a field");
}

#[test]
fn advanced_farming_needs_less_labour_for_the_same_sowing() {
    for fields in 1..=16 {
        let advanced = sacks_per_field(T, fields, 10_000, 200, true);
        let basic = sacks_per_field(T, fields, 10_000, 200, false);
        assert!(advanced >= basic, "{fields} fields: {advanced} vs {basic}");
    }
    assert_eq!(sacks_per_field(T, 8, 10_000, 192, true), 10);
    assert_eq!(sacks_per_field(T, 8, 10_000, 191, true), 9);
    assert_eq!(sacks_per_field(T, 8, 10_000, 192, false), 4);
}

#[test]
fn a_county_with_no_grain_fields_sows_nothing() {
    assert_eq!(sacks_per_field(T, 0, 10_000, 10_000, true), 0);
    let mut c = County::new();
    c.grain = 500;
    sow(T, &mut c, true);
    assert_eq!(c.grain, 500);
    assert_eq!(c.crop, [0; 3]);
}

