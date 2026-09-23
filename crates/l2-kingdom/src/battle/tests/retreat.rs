#![allow(unused_imports)]
use super::*;
use super::resolve::*;
use super::moves::*;
use super::super::*;
use crate::unit::{TroopType, UnitKind};

#[test]
fn a_retreat_halves_every_line_wipes_the_small_ones_and_spares_the_band() {
    let (_counties, mut realms) = world();
    let mut units = Units::new();
    let a = army(
        &mut units,
        1,
        true,
        &[
            (TroopType::Peasant, 101),
            (TroopType::Knight, 11),
            (TroopType::Archer, 10),
            (TroopType::Pikeman, 1),
        ],
    );
    units.get_mut(a).unwrap().mercenaries = Some(crate::unit::Mercenaries {
        troop: TroopType::Swordsman,
        men: 40,
        band: 1,
    });
    let before = units.get(a).unwrap().men;
    assert_eq!(before, 123, "the band is not part of `troops`");

    let left = withdraw_casualties(T, &mut units, &mut realms, a, 1);
    let u = units.get(a).unwrap();
    assert_eq!(u.troops[TroopType::Peasant.index()], 50, "101 / 2, truncating");
    assert_eq!(u.troops[TroopType::Knight.index()], 5, "eleven is the first line that halves");
    assert_eq!(u.troops[TroopType::Archer.index()], 0, "ten is wiped outright");
    assert_eq!(u.troops[TroopType::Pikeman.index()], 0);
    assert_eq!(u.mercenary_men(), 40, "the band walks off whole");
    assert_eq!(left, 55 + 40);
    assert_eq!(u.men, u.troops.iter().sum::<i32>() + u.mercenary_men(), "summed, not scaled");
    assert!(!u.moving, "and it stops where it stands rather than resuming its order");
    assert!(u.path.is_empty());
}

#[test]
fn an_army_of_small_lines_does_not_survive_a_retreat_at_all() {
    let (mut counties, mut realms) = world();
    let mut names = ArmyNames::new();
    let mut units = Units::new();
    let a = army(
        &mut units,
        1,
        true,
        &[(TroopType::Knight, 10), (TroopType::Swordsman, 10), (TroopType::Maceman, 10)],
    );
    let d = army(&mut units, 2, false, &[(TroopType::Archer, 100)]);
    units.get_mut(a).unwrap().besieging_county = 2;
    let after = return_to_campaign(
        T, &mut counties, &mut realms, &mut units, &mut names,
        Verdict::b_won(a, d), 2, true, true, 1,
        &crate::map::CampaignMap::empty(),
        &mut crate::explore::Explored::new(),
    crate::conquest::Restore::NEUTRAL,
    );
    assert_eq!(after.withdrawal_casualties, Some(30));
    assert!(after.loser_destroyed, "thirty men, none of them in a line of eleven");
    assert!(units.get(a).is_none());
}

/// And without the flag, fifty men buys nothing — which is the half of
/// C31 that stands.
#[test]
fn without_a_withdrawal_a_loser_with_men_is_still_destroyed() {
    let (mut counties, mut realms) = world();
    let mut names = ArmyNames::new();
    let mut units = Units::new();
    let a = army(&mut units, 1, true, &[(TroopType::Peasant, 500)]);
    let d = army(&mut units, 2, false, &[(TroopType::Archer, 100)]);
    let after = return_to_campaign(
        T, &mut counties, &mut realms, &mut units, &mut names,
        Verdict::b_won(a, d), 2, false, false, 1,
        &crate::map::CampaignMap::empty(),
        &mut crate::explore::Explored::new(),
    crate::conquest::Restore::NEUTRAL,
    );
    assert!(after.loser_destroyed);
    assert!(units.get(a).is_none());
}


#[test]
fn a_levied_defence_walks_its_survivors_back_into_the_county() {
    let (mut counties, mut realms) = world();
    let mut names = ArmyNames::new();
    let mut units = Units::new();
    let d = army(&mut units, 6, false, &[(TroopType::Peasant, 24), (TroopType::Archer, 12)]);
    {
        let u = units.get_mut(d).unwrap();
        u.defence_mark = 1;
        u.home_county = 3;
    }
    counties[3].population = 546;

    let back = disband_defence(T, &mut counties, &mut realms, &mut units, &mut names, d, 1);
    assert_eq!(back, 36);
    assert_eq!(counties[3].population, 582);
    assert!(units.get(d).is_none(), "a levied defence does not survive the battle it won");
}

#[test]
fn an_existing_army_pressed_into_defending_only_loses_the_mark() {
    let (mut counties, mut realms) = world();
    let mut names = ArmyNames::new();
    let mut units = Units::new();
    let d = army(&mut units, 2, false, &[(TroopType::Knight, 40)]);
    {
        let u = units.get_mut(d).unwrap();
        u.defence_mark = 2;
        u.home_county = 3;
    }
    counties[3].population = 546;

    let back = disband_defence(T, &mut counties, &mut realms, &mut units, &mut names, d, 1);
    assert_eq!(back, 0);
    assert_eq!(counties[3].population, 546, "nobody goes home; it was never a levy");
    assert_eq!(units.get(d).unwrap().defence_mark, 0);
    assert_eq!(units.get(d).unwrap().men, 40, "and the army is untouched");
}

#[test]
fn disbanding_an_army_that_never_defended_anything_does_nothing() {
    let (mut counties, mut realms) = world();
    let mut names = ArmyNames::new();
    let mut units = Units::new();
    let u = army(&mut units, 1, true, &[(TroopType::Peasant, 100)]);
    counties[3].population = 546;
    assert_eq!(disband_defence(T, &mut counties, &mut realms, &mut units, &mut names, u, 1), 0);
    assert_eq!(counties[3].population, 546);
    assert!(units.get(u).is_some());
}

#[test]
fn the_fixture_battle_resolves_to_the_thirty_six_men_the_save_holds() {
    let mut units = Units::new();
    let a = army(
        &mut units,
        1,
        true,
        &[(TroopType::Peasant, 128), (TroopType::Swordsman, 25), (TroopType::Archer, 25)],
    );
    let d = army(&mut units, 6, false, &[(TroopType::Peasant, 122), (TroopType::Archer, 60)]);

    assert_eq!(units.get(a).unwrap().strength_score(), 926);
    assert_eq!(units.get(d).unwrap().strength_score(), 1044);

    let v = auto_resolve(&mut units, a, d, None).unwrap();
    assert!(!v.attacker_won, "the player lost that battle, and the save says so");
    assert_eq!(v.survival_percent, Some(20));

    let survivor = units.get(d).unwrap();
    assert_eq!(survivor.troops[TroopType::Peasant.index()], 24);
    assert_eq!(survivor.troops[TroopType::Archer.index()], 12);
    assert_eq!(survivor.men, 36, "county 3 goes 546 -> 582 across the save pair");
    assert_eq!(units.get(a).unwrap().men, 0);
}

