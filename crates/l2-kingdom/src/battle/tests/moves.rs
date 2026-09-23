#![allow(unused_imports)]
use super::*;
use super::resolve::*;
use super::retreat::*;
use super::super::*;
use crate::unit::{TroopType, UnitKind};

#[test]
fn the_move_charge_is_asymmetric_and_the_ai_is_the_one_that_pays() {
    let (mut counties, mut realms) = world();
    let mut names = ArmyNames::new();

    let mut units = Units::new();
    let a = army(&mut units, 2, false, &[(TroopType::Knight, 100)]);
    let d = army(&mut units, 1, true, &[(TroopType::Peasant, 10)]);
    units.get_mut(a).unwrap().moves_used = 8;
    return_to_campaign(
        T, &mut counties, &mut realms, &mut units, &mut names,
        Verdict::a_won(a, d), 2, false, false, 1,
        &crate::map::CampaignMap::empty(),
        &mut crate::explore::Explored::new(),
    crate::conquest::Restore::NEUTRAL,
    );
    let w = units.get(a).unwrap();
    assert_eq!(w.moves_left(), 1, "a winning AI is finished for the season");

    let mut units = Units::new();
    let a = army(&mut units, 2, false, &[(TroopType::Peasant, 10)]);
    let d = army(&mut units, 1, true, &[(TroopType::Knight, 100)]);
    return_to_campaign(
        T, &mut counties, &mut realms, &mut units, &mut names,
        Verdict::b_won(a, d), 2, false, false, 1,
        &crate::map::CampaignMap::empty(),
        &mut crate::explore::Explored::new(),
    crate::conquest::Restore::NEUTRAL,
    );
    assert_eq!(units.get(d).unwrap().moves_used, 0);

    let mut units = Units::new();
    let a = army(&mut units, 1, true, &[(TroopType::Peasant, 10)]);
    let d = army(&mut units, 2, false, &[(TroopType::Knight, 100)]);
    return_to_campaign(
        T, &mut counties, &mut realms, &mut units, &mut names,
        Verdict::b_won(a, d), 2, false, false, 1,
        &crate::map::CampaignMap::empty(),
        &mut crate::explore::Explored::new(),
    crate::conquest::Restore::NEUTRAL,
    );
    assert_eq!(units.get(d).unwrap().moves_used, AI_DEFENDER_MOVE_COST);
}

#[test]
fn an_unmarked_defender_that_loses_does_not_hand_over_its_county() {
    let (mut counties, mut realms) = world();
    let mut names = ArmyNames::new();
    let mut units = Units::new();
    let a = army(&mut units, 1, true, &[(TroopType::Knight, 100)]);
    let d = army(&mut units, 2, false, &[(TroopType::Peasant, 10)]);
    counties[2].owner = 2;

    let after = return_to_campaign(
        T, &mut counties, &mut realms, &mut units, &mut names,
        Verdict::a_won(a, d), 2, false, false, 1,
        &crate::map::CampaignMap::empty(),
        &mut crate::explore::Explored::new(),
    crate::conquest::Restore::NEUTRAL,
    );
    assert_eq!(after.county_taken_by, None);
    assert_eq!(counties[2].owner, 2, "beating an army in the field takes no land");
}

/// The rule C31 stopped one branch short of: a loser that is still linked
/// as a besieger and still has men keeps its men and only loses the siege.
#[test]
fn a_besieger_that_loses_with_men_left_keeps_them_and_only_loses_the_siege() {
    let (mut counties, mut realms) = world();
    let mut names = ArmyNames::new();
    let mut units = Units::new();
    let a = army(&mut units, 1, true, &[(TroopType::Peasant, 200)]);
    let d = army(&mut units, 2, false, &[(TroopType::Archer, 100)]);
    units.get_mut(a).unwrap().besieging_county = 2;
    units.get_mut(d).unwrap().garrison_county = 2;
    units.get_mut(d).unwrap().besieged_by = a as u8;
    counties[2].owner = 2;
    counties[2].garrison_unit = d;

    let after = return_to_campaign(
        T, &mut counties, &mut realms, &mut units, &mut names,
        Verdict::b_won(a, d), 2, true, false, 1,
        &crate::map::CampaignMap::empty(),
        &mut crate::explore::Explored::new(),
    crate::conquest::Restore::NEUTRAL,
    );
    assert!(!after.loser_destroyed, "the assault failed; the army did not");
    assert!(after.loser_siege_lifted);
    assert_eq!(units.get(a).unwrap().men, 200, "it keeps every man it walked off with");
    assert_eq!(units.get(a).unwrap().besieging_county, 0, "and loses only the siege");
    assert_eq!(units.get(d).unwrap().besieged_by, 0, "both halves of the link");
    assert_eq!(counties[2].owner, 2, "the castle held");

    let mut units = Units::new();
    let a = army(&mut units, 1, true, &[(TroopType::Peasant, 200)]);
    let d = army(&mut units, 2, false, &[(TroopType::Archer, 100)]);
    units.get_mut(a).unwrap().besieging_county = 2;
    units.get_mut(a).unwrap().men = 0;
    units.get_mut(a).unwrap().troops = [0; TROOP_TYPES];
    let after = return_to_campaign(
        T, &mut counties, &mut realms, &mut units, &mut names,
        Verdict::b_won(a, d), 2, true, false, 1,
        &crate::map::CampaignMap::empty(),
        &mut crate::explore::Explored::new(),
    crate::conquest::Restore::NEUTRAL,
    );
    assert!(after.loser_destroyed);
    assert!(units.get(a).is_none());
}

/// > `Army_WithdrawCasualties` runs *above* the whole loser branch and
/// > halves every troop line first, so the fifty the original tests is the
/// > fifty *left*, and an army needs a **hundred** to make it. That is the
/// > Readme's own wording — *"any army that would have less than 50 men
/// > **after** retreating is eliminated instead"* — and this file had the
/// > constant right and the order wrong. `docs/decisions.md`
/// > C71.
#[test]
fn a_withdrawing_army_needs_a_hundred_men_to_walk_off_with_fifty() {
    let (mut counties, mut realms) = world();
    let mut names = ArmyNames::new();
    for (men, survives, left) in [(100, true, 50), (99, false, 49), (50, false, 25)] {
        let mut units = Units::new();
        let a = army(&mut units, 1, true, &[(TroopType::Peasant, men)]);
        let d = army(&mut units, 2, false, &[(TroopType::Archer, 100)]);
        let after = return_to_campaign(
            T, &mut counties, &mut realms, &mut units, &mut names,
            Verdict::b_won(a, d), 2, false, true, 1,
            &crate::map::CampaignMap::empty(),
            &mut crate::explore::Explored::new(),
        crate::conquest::Restore::NEUTRAL,
        );
        assert_eq!(units.get(a).is_some(), survives, "{men} men");
        assert_eq!(after.loser_siege_lifted, survives);
        assert_eq!(after.withdrawal_casualties, Some(men - left), "{men} men");
        if survives {
            assert_eq!(units.get(a).unwrap().men, left);
        }
    }
}

