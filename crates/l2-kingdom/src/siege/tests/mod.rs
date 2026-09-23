use super::*;
use crate::unit::Unit;

const T: &Tables = &Tables::DEFAULT;

fn besieged_county() -> ([County; MAX_COUNTIES], [Realm; MAX_REALMS], Units) {
    let mut counties: [County; MAX_COUNTIES] = core::array::from_fn(|_| County::new());
    let mut realms: [Realm; MAX_REALMS] = core::array::from_fn(|_| Realm::new());
    let mut units = Units::new();
    realms[1].in_play = true;
    realms[1].is_human = true;
    realms[2].in_play = true;
    realms[2].lord = 1;
    counties[4].owner = 2;
    counties[4].castle_type = 4;
    let mut garrison = Unit::new(UnitKind::Army, 2, 10, 10);
    garrison.men = 200;
    garrison.troops[0] = 200;
    garrison.garrison_county = 4;
    let g = units.spawn(garrison).unwrap();
    counties[4].garrison_unit = g;
    (counties, realms, units)
}

fn besieger(units: &mut Units, men: i32) -> usize {
    let mut a = Unit::new(UnitKind::Army, 1, 11, 10);
    a.owner_is_human = true;
    a.men = men;
    a.troops[0] = men;
    units.spawn(a).unwrap()
}

#[test]
fn the_four_refusals_are_each_reachable() {
    let (mut counties, realms, mut units) = besieged_county();
    let army = besieger(&mut units, 400);

    let saved = counties[4].garrison_unit;
    counties[4].garrison_unit = 0;
    assert_eq!(
        begin_siege(T, &counties, &realms, &mut units, army, 4, 1),
        Err(SiegeRefusal::NoGarrison)
    );
    counties[4].garrison_unit = saved;

    counties[4].castle_ruined = true;
    assert_eq!(
        begin_siege(T, &counties, &realms, &mut units, army, 4, 1),
        Err(SiegeRefusal::CastleRuined)
    );
    counties[4].castle_ruined = false;

    counties[4].castle_degraded = CASTLE_DEGRADED_BUILDING;
    counties[4].castle_building = 0;
    assert_eq!(
        begin_siege(T, &counties, &realms, &mut units, army, 4, 1),
        Err(SiegeRefusal::CastleUnderConstruction)
    );
    counties[4].castle_degraded = 0;

    units.get_mut(counties[4].garrison_unit).unwrap().besieged_by = 99;
    assert_eq!(
        begin_siege(T, &counties, &realms, &mut units, army, 4, 1),
        Err(SiegeRefusal::AlreadyBesieged)
    );
    units.get_mut(counties[4].garrison_unit).unwrap().besieged_by = 0;

    assert_eq!(begin_siege(T, &counties, &realms, &mut units, army, 4, 1), Ok(()));
}

#[test]
fn the_link_is_a_pair_and_breaking_it_clears_both_halves() {
    let (counties, realms, mut units) = besieged_county();
    let army = besieger(&mut units, 400);
    begin_siege(T, &counties, &realms, &mut units, army, 4, 1).unwrap();
    assert_eq!(units.get(army).unwrap().besieging_county, 4);
    assert_eq!(units.get(counties[4].garrison_unit).unwrap().besieged_by, army as u8);

    break_siege(&counties, &mut units, army);
    assert_eq!(units.get(army).unwrap().besieging_county, 0);
    assert_eq!(units.get(counties[4].garrison_unit).unwrap().besieged_by, 0);
}

#[test]
fn four_hundred_men_build_two_towers_in_one_season() {
    let (counties, realms, mut units) = besieged_county();
    let army = besieger(&mut units, 400);
    begin_siege(T, &counties, &realms, &mut units, army, 4, 1).unwrap();
    order_engine(&mut units, army, Engine::SiegeTower, 1);
    order_engine(&mut units, army, Engine::SiegeTower, 1);
    assert_eq!(units.get(army).unwrap().siege_seasons_left, 1, "400 man-seasons over 400 men");
    assert!(build_tick(&mut units, army));
    assert_eq!(units.get(army).unwrap().engines[1].percent, 100);
}

#[test]
fn the_same_army_ordering_rams_waits_three_seasons() {
    let (counties, realms, mut units) = besieged_county();
    let army = besieger(&mut units, 400);
    begin_siege(T, &counties, &realms, &mut units, army, 4, 1).unwrap();
    units.get_mut(army).unwrap().engines[2].ordered = 3;
    recompute_build_time(&mut units, army);
    assert_eq!(units.get(army).unwrap().siege_seasons_left, 3);
    assert!(!build_tick(&mut units, army));
    assert_eq!(units.get(army).unwrap().siege_seasons_left, 2);
    assert!(!build_tick(&mut units, army));
    assert!(build_tick(&mut units, army));
}

#[test]
fn work_spills_from_a_finished_engine_onto_an_unfinished_one() {
    let (counties, realms, mut units) = besieged_county();
    let army = besieger(&mut units, 400);
    begin_siege(T, &counties, &realms, &mut units, army, 4, 1).unwrap();
    units.get_mut(army).unwrap().engines[1].ordered = 1;
    units.get_mut(army).unwrap().engines[2].ordered = 1;
    recompute_build_time(&mut units, army);
    assert_eq!(units.get(army).unwrap().siege_seasons_left, 2);

    build_tick(&mut units, army);
    let u = units.get(army).unwrap();
    assert_eq!(u.engines[1].percent, 100);
    assert_eq!(u.engines[2].work_done, 200);
    assert!(build_tick(&mut units, army));
}

#[test]
fn the_screen_caps_orders_at_four_four_and_two() {
    let (counties, realms, mut units) = besieged_county();
    let army = besieger(&mut units, 400);
    begin_siege(T, &counties, &realms, &mut units, army, 4, 1).unwrap();
    for (engine, cap) in ENGINES.iter().zip(ENGINE_ORDER_CAP.iter()) {
        for _ in 0..10 {
            order_engine(&mut units, army, *engine, 1);
        }
        assert_eq!(units.get(army).unwrap().engines[engine.index()].ordered, *cap);
        for _ in 0..10 {
            order_engine(&mut units, army, *engine, -1);
        }
        assert_eq!(units.get(army).unwrap().engines[engine.index()].ordered, 0);
    }
}

#[test]
fn a_big_castle_cannot_be_stormed_bare_handed_and_a_small_one_can() {
    assert!(can_assault(0, 0), "a palisade needs nothing");
    assert!(can_assault(2, 0), "a Norman keep needs nothing");
    assert!(!can_assault(3, 0), "a stone castle does");
    assert!(can_assault(4, 1), "one engine is enough for a royal castle");
}

#[test]
fn a_besieger_with_no_engines_against_a_stone_castle_lifts_its_own_siege() {
    let (mut counties, realms, mut units) = besieged_county();
    counties[4].castle_type = 4; // level 3
    let army = besieger(&mut units, 400);
    begin_siege(T, &counties, &realms, &mut units, army, 4, 1).unwrap();
    assert_eq!(assault(&counties, &mut units, army), Assault::NoEngines);
    assert_eq!(units.get(army).unwrap().besieging_county, 0, "the siege was lifted");
    assert_eq!(units.get(counties[4].garrison_unit).unwrap().besieged_by, 0);
}

#[test]
fn the_castle_that_is_fought_is_not_always_the_castle_that_is_owned() {
    let mut c = County::new();
    c.castle_type = 5;
    assert_eq!(assault_castle_level(&c), 4, "a royal castle is level 4");

    c.castle_degraded = CASTLE_DEGRADED_BUILDING;
    c.castle_building = 3;
    assert_eq!(assault_castle_level(&c), 2, "the one being built, not the one owned");

    c.castle_degraded = CASTLE_DEGRADED_DAMAGED;
    c.castle_level_left = 1;
    assert_eq!(assault_castle_level(&c), 1, "what a previous siege left standing");
}

#[test]
fn every_shipped_lord_takes_a_named_branch_and_none_takes_the_default() {
    let doctrines: Vec<i32> = (1..=4).filter_map(|l| siege_doctrine(T, l)).collect();
    assert_eq!(doctrines, vec![8, 9, 7, 7], "Knight, Baron, Countess, Bishop");
    assert_eq!(siege_doctrine(T, 0), None, "the human has no doctrine");
}

#[test]
fn the_ai_orders_are_cumulative_and_not_alternative() {
    let (counties, mut realms, mut units) = besieged_county();
    realms[3].in_play = true;
    realms[3].lord = 3; // the Countess

    let mut knight = Unit::new(UnitKind::Army, 2, 11, 10);
    knight.men = 400;
    let k = units.spawn(knight).unwrap();
    units.get_mut(k).unwrap().besieging_county = 4;
    prepare(T, &counties, &realms, &mut units, k, 1);
    let e = units.get(k).unwrap().engines;
    assert_eq!((e[0].ordered, e[1].ordered, e[2].ordered), (0, 4, 0), "the Knight: 4 towers");

    let mut countess = Unit::new(UnitKind::Army, 3, 12, 10);
    countess.men = 400;
    let c = units.spawn(countess).unwrap();
    units.get_mut(c).unwrap().besieging_county = 4;
    prepare(T, &counties, &realms, &mut units, c, 1);
    let e = units.get(c).unwrap().engines;
    assert_eq!(
        (e[0].ordered, e[1].ordered, e[2].ordered),
        (3, 2, 0),
        "the Countess: 3 catapults AND the default 2 towers"
    );
    assert_eq!(units.get(c).unwrap().siege_seasons_left, 3, "1000 over 400 men");
}

#[test]
fn the_late_ram_needs_a_big_castle_and_a_late_season() {
    let (mut counties, mut realms, mut units) = besieged_county();
    realms[3].in_play = true;
    realms[3].lord = 3;
    let mut a = Unit::new(UnitKind::Army, 3, 12, 10);
    a.men = 400;
    let c = units.spawn(a).unwrap();
    units.get_mut(c).unwrap().besieging_county = 4;

    counties[4].castle_type = 3; // not > 3
    prepare(T, &counties, &realms, &mut units, c, 4);
    assert_eq!(units.get(c).unwrap().engines[2].ordered, 0, "a keep gets no ram");

    counties[4].castle_type = 5;
    prepare(T, &counties, &realms, &mut units, c, 2); // not > 2
    assert_eq!(units.get(c).unwrap().engines[2].ordered, 0, "not before season 3");

    prepare(T, &counties, &realms, &mut units, c, 3);
    assert_eq!(units.get(c).unwrap().engines[2].ordered, 1);
}

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
