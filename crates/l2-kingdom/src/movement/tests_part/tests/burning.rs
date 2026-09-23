#![allow(unused_imports)]
use super::*;
use super::paths::*;
use super::trample::*;
use super::super::*;
use crate::map::flags;
use crate::unit::Unit;

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

