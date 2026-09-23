#![allow(unused_imports)]
use super::*;
use super::orders::*;
use super::super::*;
use crate::unit::Unit;

#[test]
fn the_cursor_stops_on_the_army_whose_engines_came_in() {
    let (counties, realms, mut units) = besieged_county();
    let slow = besieger(&mut units, 100);
    let fast = besieger(&mut units, 400);
    begin_siege(T, &counties, &realms, &mut units, slow, 4, 1).unwrap();
    units.get_mut(slow).unwrap().besieging_county = 4;
    units.get_mut(fast).unwrap().besieging_county = 4;
    units.get_mut(slow).unwrap().engines[0].ordered = 4; // 800 over 100 men
    units.get_mut(fast).unwrap().engines[0].ordered = 1; // 200 over 400 men
    recompute_build_time(&mut units, slow);
    recompute_build_time(&mut units, fast);

    let mut cursor = SiegeCursor { at: 1, count: 2 };
    assert_eq!(tick_phase(&mut cursor, &mut units), Some(fast));
    assert_eq!(cursor.at, fast, "the cursor is left on the army that is ready");
    assert_eq!(units.get(slow).unwrap().siege_seasons_left, 7, "800-100 over 100");
}

#[test]
fn the_phase_opens_by_breaking_links_that_no_longer_agree() {
    let (counties, realms, mut units) = besieged_county();
    let army = besieger(&mut units, 400);
    begin_siege(T, &counties, &realms, &mut units, army, 4, 1).unwrap();

    let garrison = counties[4].garrison_unit;
    units.get_mut(garrison).unwrap().garrison_county = 0;
    start_phase(&counties, &mut units);
    assert_eq!(units.get(army).unwrap().besieging_county, 0);

    // `Siege_ValidateLink` clears `+0x199` and touches `+0x19A` not at all;
    // the county sweep that would clear it ran *before* the validation
    // pass in the same call. So the pair takes two turn-phase-2s to come
// fully apart, and this asserts the lag.
    assert_eq!(units.get(garrison).unwrap().besieged_by, army as u8, "stale for one turn");
    start_phase(&counties, &mut units);
    assert_eq!(units.get(garrison).unwrap().besieged_by, 0, "and gone on the next");
}

#[test]
fn only_the_garrison_gets_oil_and_the_count_is_the_castle_level() {
    for (level, expected) in OIL_BY_CASTLE_LEVEL.iter().enumerate() {
        assert_eq!(prepare_garrison(level as u8).oil, *expected);
    }
    assert_eq!(prepare_garrison(9).oil, 0, "no default arm");

    let mut u = Unit::new(UnitKind::Army, 1, 0, 0);
    u.engines[0].ordered = 2;
    u.engines[2].ordered = 1;
    let e = prepare_besieger(&u);
    assert_eq!((e.catapults, e.siege_towers, e.battering_rams, e.oil), (2, 0, 1, 0));
}

#[test]
fn a_besieged_garrison_cannot_be_reinforced_and_may_only_sortie() {
    let (counties, realms, mut units) = besieged_county();
    let army = besieger(&mut units, 400);
    assert!(!garrison_is_besieged(&counties, &units, 4));
    begin_siege(T, &counties, &realms, &mut units, army, 4, 1).unwrap();
    assert!(garrison_is_besieged(&counties, &units, 4));
    assert_eq!(sortie_target(&units, counties[4].garrison_unit), Some(army));
    break_siege(&counties, &mut units, army);
    assert_eq!(sortie_target(&units, counties[4].garrison_unit), None);
}

#[test]
fn a_besieger_with_no_men_never_reports_its_engines_ready() {
    let (counties, realms, mut units) = besieged_county();
    let army = besieger(&mut units, 400);
    begin_siege(T, &counties, &realms, &mut units, army, 4, 1).unwrap();
    order_engine(&mut units, army, Engine::Catapult, 1);
    units.get_mut(army).unwrap().men = 0;
    assert!(!build_tick(&mut units, army));
    assert_eq!(units.get(army).unwrap().siege_seasons_left, 1, "unchanged, not zeroed");
}

