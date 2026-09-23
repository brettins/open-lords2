#![allow(unused_imports)]
use super::*;



/// **Corrected, and the assertion reversed.** This used to expect
/// `Contact::Blocked`, on the reading that a merchant cannot be
/// *fought*. The rung says `return local_8`, and `local_8` is the ordinary
/// Road or Open code — so the army walks *through* the merchant and no
/// contact is reported at all. [`crate::movement::pass_through`] is where
/// that now happens, which makes rungs 1 and 2 of
/// [`UnitsTick::classify_occupied`] unreachable by construction
/// by comment. `docs/decisions.md` C40.
#[test]
fn a_merchant_is_walked_through_rather_than_attacked() {
    let mut k = kingdom();
    k.realms[2].in_play = true;
    let a = army(&mut k, 1, 5, 10);
    let mut m = Unit::new(UnitKind::Merchant, OWNERLESS, 6, 10);
    m.county = 1;
    k.campaign.units.spawn(m).unwrap();
    movement::order_move(&k.campaign.map, &mut k.campaign.units, a, (8, 10), movement::Routing::Direct)
        .unwrap();
    let t = k.tick_units();
    assert_eq!(t.battle(), None);
    assert_eq!(t.contacts, vec![], "no fight, and no obstacle either");
    assert_eq!(k.campaign.units.get(a).unwrap().tile(), (6, 10), "the army is on the tile");
}

#[test]
fn phase_two_originates_no_movement() {
    let mut k = kingdom();
    let id = army(&mut k, 1, 5, 10);
    assert_eq!(k.begin_unit_phase(Phase::ArmyMovement), 0);
    assert!(!k.campaign.units.get(id).unwrap().moving);
}

#[test]
fn phase_three_sends_every_transport_to_its_cargo_county_anchor() {
    let mut k = kingdom();
    let mut t = Unit::new(UnitKind::Transport, 1, 5, 10);
    t.county = 1;
    t.cargo_county = 2;
    t.needs_destination = false;
    t.dest = Some((9, 9));
    let id = k.campaign.units.spawn(t).unwrap();

    assert_eq!(k.begin_unit_phase(Phase::SupplyTransports), 1);
    let u = k.campaign.units.get(id).unwrap();
    assert_eq!(u.dest, Some((50, 10)), "county 2's anchor, not a tile near it");
    assert_eq!(u.dest_county, 2);
    assert!(u.moving);
}

/// **The other end of a shipment.** `Transport_Deliver` (`0x004296B5`):
#[test]
fn a_transport_reaching_its_cargo_countys_town_unloads_and_is_gone() {
    let mut k = kingdom();
    k.campaign.map.set_flags(40, 10, flags::CASTLE);
    let (grain, herd) = (k.counties[2].grain, k.counties[2].herd);
    let mut t = Unit::new(UnitKind::Transport, 1, 30, 10);
    t.county = 1;
    t.morale = 1;
    t.cargo_county = 2;
    t.needs_destination = false;
    t.troops[0] = 300;
    t.troops[2] = 40;
    t.men = 340;
    let id = k.campaign.units.spawn(t).unwrap();
    movement::order_move(&k.campaign.map, &mut k.campaign.units, id, (40, 10), movement::Routing::Direct)
        .unwrap();

    for _ in 0..400 {
        k.tick_units();
    }
    assert!(k.campaign.units.get(id).is_none(), "the cart is unloaded and gone");
    assert_eq!(k.counties[2].grain, grain + 300);
    assert_eq!(k.counties[2].herd, herd + 40);
    assert_eq!(k.counties[2].owner, 0, "and a transport takes no county");
}

#[test]
fn a_transport_passing_a_town_that_is_not_its_destination_unloads_nothing() {
    let mut k = kingdom();
    k.campaign.map.set_flags(40, 10, flags::CASTLE);
    let grain = k.counties[2].grain;
    let mut t = Unit::new(UnitKind::Transport, 1, 30, 10);
    t.county = 1;
    t.cargo_county = 1;
    t.needs_destination = false;
    t.troops[0] = 300;
    t.men = 300;
    let id = k.campaign.units.spawn(t).unwrap();
    movement::order_move(&k.campaign.map, &mut k.campaign.units, id, (40, 10), movement::Routing::Direct)
        .unwrap();

    for _ in 0..400 {
        k.tick_units();
    }
    assert_eq!(k.campaign.units.get(id).map(|u| u.troops[0]), Some(300));
    assert_eq!(k.counties[2].grain, grain);
}

#[test]
fn phase_five_walks_one_cursor_for_every_mob() {
    let mut k = kingdom();
    for (x, county) in [(5u8, 1u8), (50, 2)] {
        let mut m = Unit::new(UnitKind::PeasantMob, OWNERLESS, x, 10);
        m.county = county;
        m.men = 200;
        k.campaign.units.spawn(m).unwrap();
    }
    k.season = MOB_RETARGET_SEASON;
    k.begin_unit_phase(Phase::PeasantMobs);

    let dests: Vec<u8> =
        ids_of_kind(&k.campaign.units, UnitKind::PeasantMob)
            .iter()
            .map(|&id| k.campaign.units.get(id).unwrap().dest_county)
            .collect();
    assert_eq!(dests, vec![2, 1], "the cursor advanced between them");
}

/// Only realm 6's mobs hold phase 5 open — `FUN_004A4E3D(2, 6)`.
#[test]
fn a_mob_belonging_to_a_realm_does_not_hold_phase_five_open() {
    let mut k = kingdom();
    let mut m = Unit::new(UnitKind::PeasantMob, 1, 5, 10);
    m.county = 1;
    m.moving = true;
    k.campaign.units.spawn(m).unwrap();
    assert!(!k.units_moving(UnitKind::PeasantMob));

    let mut m = Unit::new(UnitKind::PeasantMob, OWNERLESS, 6, 10);
    m.county = 1;
    m.moving = true;
    k.campaign.units.spawn(m).unwrap();
    assert!(k.units_moving(UnitKind::PeasantMob));
}

#[test]
fn a_unit_with_no_path_stops_being_moving() {
    let mut k = kingdom();
    let id = army(&mut k, 1, 5, 10);
    k.campaign.units.get_mut(id).unwrap().moving = true;
    assert!(k.units_moving(UnitKind::Army));
    k.tick_units();
    assert!(!k.units_moving(UnitKind::Army), "no path is not a reason to wait for ever");
    assert!(k.campaign.units.get(id).unwrap().needs_destination);
}

#[test]
fn the_allowance_is_rebuilt_from_the_type() {
    let mut k = kingdom();
    let id = army(&mut k, 1, 5, 10);
    k.campaign.units.get_mut(id).unwrap().move_allowance = 0;
    refresh_allowances(&mut k.campaign.units);
    assert_eq!(k.campaign.units.get(id).unwrap().move_allowance, 15);
}

