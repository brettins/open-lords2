#![allow(unused_imports)]
use super::*;
use super::moves::*;
use super::retreat::*;
use super::super::*;
use crate::unit::{TroopType, UnitKind};

#[test]
fn two_ai_armies_never_reach_a_screen() {
    let mut units = Units::new();
    let a = army(&mut units, 2, false, &[(TroopType::Peasant, 100)]);
    let b = army(&mut units, 3, false, &[(TroopType::Peasant, 100)]);
    for byte in [0, 1] {
        assert_eq!(settlement(&units, a, b, byte), Settlement::Silently);
    }
}

#[test]
fn the_option_decides_whether_a_human_is_asked_and_two_humans_always_are() {
    let mut units = Units::new();
    let human = army(&mut units, 1, true, &[(TroopType::Peasant, 100)]);
    let ai = army(&mut units, 2, false, &[(TroopType::Peasant, 100)]);
    let other = army(&mut units, 3, true, &[(TroopType::Peasant, 100)]);

    assert_eq!(settlement(&units, human, ai, 0), Settlement::Reported);
    assert_eq!(settlement(&units, human, ai, 1), Settlement::Prompt);
    assert_eq!(settlement(&units, human, other, 0), Settlement::Prompt);
}


#[test]
fn an_even_fight_leaves_the_winner_a_tenth_of_its_army() {
    assert_eq!(survival_percent(100), 10);
    assert_eq!(survival_percent(109), 10);
    assert_eq!(survival_percent(110), 20, "the threshold is exclusive");
    assert_eq!(survival_percent(899), 98);
    assert_eq!(survival_percent(900), SURVIVAL_DEFAULT);
}

#[test]
fn a_tie_goes_to_the_attacker() {
    let mut units = Units::new();
    let a = army(&mut units, 1, true, &[(TroopType::Peasant, 100)]);
    let b = army(&mut units, 2, false, &[(TroopType::Peasant, 100)]);
    let v = auto_resolve(&mut units, a, b, None).unwrap();
    assert!(v.attacker_won, "equal scores, and the test is `sA < sB`");
    assert_eq!(v.winner(), a);
    assert_eq!(v.loser(), b);
    assert_eq!(units.get(a).unwrap().men, 10);
    assert_eq!(units.get(b).unwrap().men, 0);
}

#[test]
fn the_winners_total_is_the_sum_of_its_surviving_counts() {
    let mut units = Units::new();
    let a = army(
        &mut units,
        1,
        true,
        &[(TroopType::Knight, 100), (TroopType::Archer, 33), (TroopType::Peasant, 7)],
    );
    let b = army(&mut units, 2, false, &[(TroopType::Peasant, 3)]);
    let v = auto_resolve(&mut units, a, b, None).unwrap();
    assert!(v.attacker_won);
    let w = units.get(a).unwrap();
    assert_eq!(w.men, w.troops.iter().sum::<i32>());
    assert_eq!(v.survival_percent, Some(SURVIVAL_DEFAULT), "an overwhelming advantage");
    assert_eq!(w.troops[TroopType::Archer.index()], 33, "and it costs nobody");
}

#[test]
fn the_castle_bonus_can_turn_a_losing_defence_into_a_winning_one() {
    let mut units = Units::new();
    let a = army(&mut units, 1, true, &[(TroopType::Swordsman, 100)]);
    let b = army(&mut units, 2, false, &[(TroopType::Swordsman, 90)]);
    let open = auto_resolve(&mut units.clone(), a, b, None).unwrap();
    assert!(open.attacker_won, "in the open the bigger army wins");
    let sieged = auto_resolve(&mut units, a, b, Some(0)).unwrap();
    assert!(!sieged.attacker_won, "160% of 90 beats 100");
}


#[test]
fn the_four_siege_banners_each_say_a_different_thing() {
    let a_won = Verdict::a_won(1, 2);
    let b_won = Verdict::b_won(1, 2);
    let (besieger, garrison) = (1, 2);

    assert_eq!(outcome(a_won, true, besieger, besieger, garrison), Outcome::SiegeWon);
    assert_eq!(outcome(b_won, true, besieger, garrison, besieger), Outcome::SiegeLost);
    assert_eq!(outcome(b_won, true, garrison, garrison, besieger), Outcome::SiegeLifted);
    assert_eq!(outcome(a_won, true, garrison, besieger, garrison), Outcome::CastleLost);
    assert_eq!(outcome(a_won, false, 1, 1, 2), Outcome::Won);
    assert_eq!(outcome(a_won, false, 2, 1, 2), Outcome::Lost);
    assert_eq!(outcome(a_won, false, 4, 1, 2), Outcome::Bystander);
}

#[test]
fn the_seven_pairs_are_numbered_nought_to_six() {
    let all = [
        Outcome::Won,
        Outcome::Lost,
        Outcome::SiegeWon,
        Outcome::SiegeLost,
        Outcome::SiegeLifted,
        Outcome::CastleLost,
        Outcome::Bystander,
    ];
    for (i, o) in all.iter().enumerate() {
        assert_eq!(o.pair(), i);
    }
}


#[test]
fn a_winning_attacker_takes_the_county_from_a_marked_defence() {
    let (mut counties, mut realms) = world();
    let mut names = ArmyNames::new();
    let mut units = Units::new();
    let a = army(&mut units, 1, true, &[(TroopType::Knight, 100)]);
    let d = army(&mut units, 6, false, &[(TroopType::Peasant, 50)]);
    units.get_mut(d).unwrap().defence_mark = 1;
    counties[2].owner = 0;
    units.get_mut(a).unwrap().moves_used = 8;

    let v = Verdict::a_won(a, d);
    let after =
        return_to_campaign(T, &mut counties, &mut realms, &mut units, &mut names, v, 2, false, false, 1, &crate::map::CampaignMap::empty(), &mut crate::explore::Explored::new(), crate::conquest::Restore::NEUTRAL);

    assert_eq!(after.county_taken_by, Some(1));
    assert_eq!(counties[2].owner, 1, "the county changed hands");
    assert!(after.loser_destroyed);
    assert!(units.get(d).is_none(), "the loser is gone");
    assert_eq!(
        units.get(a).unwrap().moves_used,
        16,
        "eight from the attack and eight more from winning"
    );
    assert_eq!(after.offence, None, "an ownerless militia has no realm to resent with");
}

#[test]
fn a_winning_defender_leaves_the_county_exactly_where_it_was() {
    let (mut counties, mut realms) = world();
    let mut names = ArmyNames::new();
    let mut units = Units::new();
    let a = army(&mut units, 1, true, &[(TroopType::Peasant, 50)]);
    let d = army(&mut units, 6, false, &[(TroopType::Knight, 100)]);
    units.get_mut(d).unwrap().defence_mark = 1;
    counties[2].owner = 0;

    let v = Verdict::b_won(a, d);
    let after =
        return_to_campaign(T, &mut counties, &mut realms, &mut units, &mut names, v, 2, false, false, 1, &crate::map::CampaignMap::empty(), &mut crate::explore::Explored::new(), crate::conquest::Restore::NEUTRAL);

    assert_eq!(after.county_taken_by, None);
    assert_eq!(counties[2].owner, 0, "still neutral");
    assert!(units.get(a).is_none(), "the attacker is destroyed");
    assert!(units.get(d).is_some(), "the defence is still standing");
    assert_eq!(after.offence, Some((1, 6)), "the attacker's realm resents the militia");
}

