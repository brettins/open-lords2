//! Unit_EnterOccupiedTile 0x004658C1, arm by arm.
#![allow(unused_imports)]
use super::*;
use crate::diplomacy::Letter;

fn march(k: &mut Kingdom, a: usize) -> UnitsTick {
    movement::order_move(&k.campaign.map, &mut k.campaign.units, a, (8, 10), movement::Routing::Direct)
        .unwrap();
    k.tick_units()
}

/// Ablation: 00460000.c:4284-4287, `return Arm::Over` on a refusal.
#[test]
fn an_ally_is_walked_over_and_the_ally_grudges_it() {
    let mut k = kingdom();
    k.realms[2].in_play = true;
    k.realms[1].ally = 2;
    let a = army(&mut k, 1, 5, 10);
    let b = army(&mut k, 2, 6, 10);
    let t = march(&mut k, a);
    assert_eq!(t.battle(), None);
    assert!(t.contacts.is_empty(), "{:?}", t.contacts);
    assert_eq!(k.campaign.units.get(a).unwrap().tile(), (6, 10));
    assert_eq!(k.campaign.units.get(b).unwrap().tile(), (6, 10));
    assert_eq!(k.realms[1].pair(2).grudge, 1, "Diplo_ActionAllowed's +1");
}

/// Ablation: :4280 `combine(units, id, occ)`, the mover as `into`, and its `Arm::Over`.
#[test]
fn an_ai_army_absorbs_its_own_and_walks_on() {
    let mut k = kingdom();
    k.realms[2].in_play = true;
    let a = army(&mut k, 2, 5, 10);
    let b = army(&mut k, 2, 6, 10);
    let t = march(&mut k, a);
    assert_eq!(t.contacts, vec![Contact::Merged { mover: a, into: a }]);
    assert!(k.campaign.units.get(b).is_none(), "the occupant is destroyed");
    let m = k.campaign.units.get(a).unwrap();
    assert_eq!((m.men, m.tile()), (200, (6, 10)));
}

/// Ablation: :4274-4279, `if errand { return Arm::Over; }`.
#[test]
fn an_army_on_mission_four_walks_over_without_merging() {
    let mut k = kingdom();
    k.realms[2].in_play = true;
    let a = army(&mut k, 2, 5, 10);
    let b = army(&mut k, 2, 6, 10);
    k.campaign.units.get_mut(b).unwrap().mission = crate::ai_army::Mission::JOIN_GARRISON;
    let t = march(&mut k, a);
    assert!(t.contacts.is_empty(), "{:?}", t.contacts);
    assert_eq!(k.campaign.units.get(a).unwrap().tile(), (6, 10));
    assert_eq!(k.campaign.units.get(b).unwrap().men, 100);
}

/// Ablation: :4301-4310, the transport arm and step_one's `Arm::Captured` return.
#[test]
fn an_enemy_transport_is_taken_and_the_army_stays() {
    let mut k = kingdom();
    k.realms[2].in_play = true;
    let a = army(&mut k, 1, 5, 10);
    let tr = k.campaign.units.spawn(Unit::new(UnitKind::Transport, 2, 6, 10)).unwrap();
    let t = march(&mut k, a);
    assert!(k.campaign.units.get(tr).is_none(), "FUN_0046F0A9 frees it");
    let m = k.campaign.units.get(a).unwrap();
    assert_eq!((m.tile(), m.moving), ((5, 10), true), "local_8 = 10: no step, moving kept");
    let letters: Vec<_> =
        t.posted.iter().filter_map(|p| if let Posted::Letter(l) = p { Some(*l) } else { None }).collect();
    assert!(letters.contains(&Letter::system(1, 162)));
    assert!(letters.contains(&Letter::system(2, 161)));
}

/// Ablation: :4301-4303 owner and Diplo_ActionAllowed tests; one's own transport is walked over.
#[test]
fn ones_own_transport_is_walked_over() {
    let mut k = kingdom();
    let a = army(&mut k, 1, 5, 10);
    let tr = k.campaign.units.spawn(Unit::new(UnitKind::Transport, 1, 6, 10)).unwrap();
    march(&mut k, a);
    assert!(k.campaign.units.get(tr).is_some());
    assert_eq!(k.campaign.units.get(a).unwrap().tile(), (6, 10));
}
