#![allow(unused_imports)]
use super::*;
use super::starvation::*;
use super::mercenaries::*;
use super::mines::*;
use super::conquest_part::*;
use l2_kingdom::conquest;
use l2_kingdom::county::MAX_COUNTIES;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::movement::Routing;
use l2_kingdom::phase::Pass;
use l2_kingdom::report::Message;
use l2_kingdom::unit::{Unit, UnitKind, Units};
use l2_kingdom::{Kingdom, MercenaryBands, Options, TroopType};

#[test]
fn a_kingdom_with_armies_in_it_saves_and_resumes_identically() {
    let mut k = kingdom();
    k.options.armies_eat = true;
    let a = army(&mut k, 1, 1, 400, 5, 5);
    let b = army(&mut k, 1, 1, 250, 6, 6);
    k.campaign.units.get_mut(b).unwrap().mercenaries =
        Some(l2_kingdom::Mercenaries { band: 1, troop: TroopType::Pikeman, men: 100 });
    l2_kingdom::movement::order_move(&k.campaign.map, &mut k.campaign.units, a, (20, 20), Routing::Direct);
    k.advance_season();

    let bytes = l2_kingdom::save::encode(&k);
    let restored = l2_kingdom::save::decode(&bytes, k.tables.clone()).expect("our own bytes");
    assert_eq!(restored, k, "field for field, campaign included");

    let mut one = k.clone();
    let mut two = restored;
    for _ in 0..4 {
        one.advance_season();
        two.advance_season();
    }
    assert_eq!(one, two, "and they play on identically");
    assert_eq!(l2_kingdom::save::encode(&one), l2_kingdom::save::encode(&two));
}

#[test]
fn a_kingdom_with_no_units_at_all_round_trips() {
    let k = Kingdom::new(5);
    let bytes = l2_kingdom::save::encode(&k);
    assert_eq!(l2_kingdom::save::decode(&bytes, k.tables.clone()).unwrap(), k);
    assert_eq!(k.campaign.units.len(), 0);
    assert_eq!(k.campaign.mercenaries.in_play(), 0);
}

#[test]
fn a_save_keeps_units_in_the_slots_they_were_in() {
    let mut k = kingdom();
    let mut units = Units::new();
    let mut far = Unit::new(UnitKind::Army, 1, 3, 3);
    far.men = 100;
    units.put(90, far);
    k.campaign.units = units;
    k.counties[1].garrison_unit = 90;

    let bytes = l2_kingdom::save::encode(&k);
    let restored = l2_kingdom::save::decode(&bytes, k.tables.clone()).unwrap();
    assert!(restored.campaign.units.get(90).is_some(), "slot 90, not slot 1");
    assert!(restored.campaign.units.get(1).is_none());
    assert_eq!(restored.counties[1].garrison_unit, 90);
}

#[test]
fn two_identically_built_kingdoms_stay_byte_identical_through_a_year() {
    let build = || {
        let mut k = kingdom();
        k.options.armies_eat = true;
        for n in 0..6 {
            let id = army(&mut k, 1, 1, 200 + n * 30, 5 + n as u8, 5);
            l2_kingdom::movement::order_move(
                &k.campaign.map,
                &mut k.campaign.units,
                id,
                (20 + n as u8, 25),
                Routing::Direct,
            );
        }
        k
    };
    let (mut one, mut two) = (build(), build());
    assert_eq!(l2_kingdom::save::encode(&one), l2_kingdom::save::encode(&two));
    for _ in 0..4 {
        one.advance_season();
        two.advance_season();
        assert_eq!(l2_kingdom::save::encode(&one), l2_kingdom::save::encode(&two));
    }
}

#[test]
fn the_unit_array_holds_a_hundred_and_fifty_and_refuses_the_hundred_and_fifty_first() {
    let mut k = kingdom();
    for _ in 0..150 {
        let mut u = Unit::new(UnitKind::Army, 1, 0, 0);
        u.men = 50;
        assert!(k.campaign.units.spawn(u).is_some());
    }
    let mut one_too_many = Unit::new(UnitKind::Army, 1, 0, 0);
    one_too_many.men = 50;
    assert_eq!(k.campaign.units.spawn(one_too_many), None);
    assert_eq!(k.campaign.units.len(), 150);
    assert_eq!(MAX_COUNTIES, 17, "and the county array is still seventeen");
}

