use super::*;
use crate::movement;

/// Army_Tick 00460000.c:4097 `else if (playerDriven == 0) moving = 2;`. Ablate that arm in tick_units.
#[test]
fn a_stacked_army_is_started_every_frame() {
    let mut k = kingdom();
    let a = army(&mut k, 2, 5, 10);
    k.campaign.units.get_mut(a).unwrap().player_driven = false;
    k.tick_units();
    let u = k.campaign.units.get(a).unwrap();
    assert!(u.moving && !u.ordered, "moving = 2");
}

/// Unit_Step 00460000.c:4368 `FUN_00467196(unit)` when a chained unit stops. Ablate the
/// step_aside call in step_one.
#[test]
fn a_stacked_army_that_stops_steps_aside() {
    let mut k = kingdom();
    k.campaign.rebuild_move_cost();
    let a = army(&mut k, 2, 5, 10);
    let u = k.campaign.units.get_mut(a).unwrap();
    (u.player_driven, u.moving, u.moves_used) = (false, true, 4);
    k.tick_units();
    let u = k.campaign.units.get(a).unwrap();
    assert!(u.moving && !u.ordered, "FUN_00467196 writes moving = 2");
    assert_eq!(u.moves_used, 3, "refunded the ring, 1");
    assert!(!u.path.is_empty() && u.dest != Some((5, 10)));
}

/// Unit_MoveInFacing 00460000.c:4805 Unit_LinkToTile: entering an occupied tile chains the unit.
/// Ablate the link_to_tile call in movement::step.
#[test]
fn a_merchant_passing_through_an_army_is_chained() {
    let mut k = kingdom();
    let _ = army(&mut k, 2, 6, 10);
    let mut m = Unit::new(UnitKind::Merchant, OWNERLESS, 5, 10);
    m.player_driven = true;
    m.path = vec![(6, 10)];
    m.moving = true;
    let id = k.campaign.units.spawn(m).unwrap();
    let c = &mut k.campaign;
    movement::step(&mut c.map, &mut k.counties, &k.realms, &mut c.units, id).unwrap();
    assert_eq!(k.campaign.units.get(id).unwrap().tile(), (6, 10));
    assert!(!k.campaign.units.get(id).unwrap().player_driven);
}

/// 004a0000.c:1978 `if (kind == 1) Siege_Break(unit);`. Ablate the break_siege call.
#[test]
fn starting_a_realms_army_lifts_its_siege() {
    let mut k = kingdom();
    let a = army(&mut k, 2, 5, 10);
    let u = k.campaign.units.get_mut(a).unwrap();
    (u.moving, u.ordered, u.besieging_county) = (true, true, 1);
    assert!(k.realm_units_moving(UnitKind::Army, 2));
    assert_eq!(k.campaign.units.get(a).unwrap().besieging_county, 0);
}

/// Merchant_AdvanceAll 00420000.c:2954 `moving = 1`; Units_AnyMovingOfKind 004a0000.c:2004
/// `moving = 2`. Ablate `u.ordered = true` in order_move_by_road, or `u.ordered = false` in units_moving.
#[test]
fn a_merchant_is_ordered_then_started_by_the_phase_wait() {
    let mut k = kingdom();
    k.campaign.rebuild_move_cost();
    let id = k.campaign.units.spawn(Unit::new(UnitKind::Merchant, OWNERLESS, 5, 10)).unwrap();
    let c = &mut k.campaign;
    movement::order_move_by_road(&c.move_cost, &c.map, &mut c.units, id, (8, 10)).unwrap();
    assert!(k.campaign.units.get(id).unwrap().ordered, "moving = 1");
    assert!(k.units_moving(UnitKind::Merchant));
    assert!(!k.campaign.units.get(id).unwrap().ordered, "moving = 2");
}

/// Unit_Step 00460000.c:4368 reads path[0] = (0, 0) after an empty Path_CopyToUnit: a load's
/// all-zero g_moveCost gives Merchant_AdvanceAll one, and the merchant steps north-west (the
/// original's turn 1 after england-turn1.sav: 2 merchant.wav at phase 6). Ablate zeroed_head's call.
#[test]
fn a_merchant_with_an_empty_copied_path_walks_toward_the_origin() {
    let mut k = kingdom();
    let id = k.campaign.units.spawn(Unit::new(UnitKind::Merchant, OWNERLESS, 5, 10)).unwrap();
    let c = &mut k.campaign;
    assert_eq!(movement::order_move_by_road(&c.move_cost, &c.map, &mut c.units, id, (8, 10)), Some(0));
    k.campaign.units.get_mut(id).unwrap().player_driven = false;
    assert!(k.units_moving(UnitKind::Merchant));
    k.tick_units();
    assert_eq!(k.campaign.units.get(id).unwrap().tile(), (4, 9));
}
