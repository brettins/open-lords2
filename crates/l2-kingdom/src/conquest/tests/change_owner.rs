#![allow(unused_imports)]
use super::*;
use super::capture_rules::*;
use super::defence::*;
use super::super::*;
use crate::explore::Explored;
use crate::map::{CampaignMap, MAP_DIM, MAP_TILES};
use crate::unit::Unit;

#[test]
fn a_captured_county_loses_happiness_on_the_events_line() {
    let (mut counties, mut realms) = world();
    let mut units = Units::new();
    counties[2].happiness = 77;
    let lost = change_owner(T, &mut counties, &mut realms, &mut units, 1, 2, 0, &CampaignMap::empty(), &mut Explored::new(), Restore::NEUTRAL).penalty;
    assert_eq!(lost, 10, "a human at difficulty 0");
    assert_eq!(counties[2].happiness, 67);
    assert_eq!(counties[2].shown_events, -10);
    assert_eq!(counties[2].owner, 1);
}

#[test]
fn the_conquest_penalty_is_flat_for_an_ai_and_scales_for_a_human() {
    for d in 0..=3u8 {
        assert_eq!(capture_happiness_penalty(false, d), 30, "difficulty {d}");
    }
    assert_eq!(capture_happiness_penalty(true, 0), 10);
    assert_eq!(capture_happiness_penalty(true, 1), 30, "equal at Normal");
    assert_eq!(capture_happiness_penalty(true, 2), 50);
}

#[test]
fn a_county_that_cannot_pay_the_penalty_goes_to_zero_and_the_panel_agrees() {
    let (mut counties, mut realms) = world();
    let mut units = Units::new();
    counties[2].happiness = 4;
    assert_eq!(change_owner(T, &mut counties, &mut realms, &mut units, 1, 2, 2, &CampaignMap::empty(), &mut Explored::new(), Restore::NEUTRAL).penalty, 50);
    assert_eq!(counties[2].happiness, 0);
    assert_eq!(counties[2].shown_events, -4, "what was taken, not the fifty");
}

#[test]
fn changing_hands_moves_the_county_between_the_two_realms_counts() {
    let (mut counties, mut realms) = world();
    let mut units = Units::new();
    counties[2].owner = 2;
    recount_realm_counties(&counties, &mut realms);
    assert_eq!((realms[1].county_count, realms[2].county_count), (1, 1));

    change_owner(T, &mut counties, &mut realms, &mut units, 1, 2, 0, &CampaignMap::empty(), &mut Explored::new(), Restore::NEUTRAL);
    assert_eq!((realms[1].county_count, realms[2].county_count), (2, 0));
}

/// This is the turn `docs/plan.md` item 4 asks for, minus the battle. It
/// exists because every other test here builds an army by hand, and a layer
/// whose pieces each work but do not compose is the failure
/// `docs/decisions.md` C21 is about.
#[test]
fn an_army_can_be_raised_marched_across_a_border_and_take_a_county() {
    use crate::levy::{self, LevyBasket, Muster};
    use crate::movement::Routing;

    let mut map = two_county_map();
    map.set_flags(50, 10, crate::map::flags::CASTLE);
    for x in 5..50u8 {
        map.set_flags(x, 10, crate::map::flags::ROAD);
    }
    let (mut counties, mut realms) = world();
    counties[2].population = 20; // too small to raise a defence
    realms[1].weapons = [100, 0, 0, 0, 0, 0];
    let mut units = Units::new();
    let mut names = ArmyNames::new();

    let levy = levy::set_percent(T, &counties[1], 20);
    assert_eq!(levy.men, 100);
    let mut basket = LevyBasket::seed(&realms[1], levy.men);
    basket.equip(crate::unit::TroopType::Crossbowman, 100);
    let army = levy::create_army(
        T, &map, &mut counties, &mut realms, &mut units, &mut names, &basket,
        Muster { realm: 1, county: 1, happiness_cost: levy.happiness_cost, year: 1268 },
        &mut Explored::new(),
    )
    .expect("county 1 has room");
    assert_eq!(counties[1].population, 400);
    assert_eq!(units.get(army).unwrap().men, 100);

    let mut campaign = crate::kingdom::Campaign::new();
    campaign.map = map;
    campaign.units = units;
    campaign.names = names;
    campaign.units.get_mut(army).unwrap().x = 5;
    campaign.units.get_mut(army).unwrap().y = 10;

    let mut seasons = 0;
    let outcome = loop {
        seasons += 1;
        assert!(seasons < 20, "it should not take twenty seasons to cross a map");
        crate::movement::order_move(&campaign.map, &mut campaign.units, army, (50, 10), Routing::Direct);
        let (_, outcome) =
            march_and_fight(T, &mut campaign, &mut counties, &mut realms, army, 0, 1268, Restore::NEUTRAL);
        if let Some(o) = outcome {
            break o;
        }
        campaign.units.reset_moves();
    };

    assert!(matches!(outcome, Attack::Captured(_)), "got {outcome:?}");
    assert_eq!(counties[2].owner, 1, "county 2 has changed hands");
    assert_eq!(realms[1].county_count, 2);
    assert_eq!(campaign.units.get(army).unwrap().county, 2, "and the army is standing in it");
}

pub(crate) fn bordering(counties: &mut [County; MAX_COUNTIES]) {
    counties[1].neighbour_count = 1;
    counties[1].neighbours[0] = 2;
    counties[2].neighbour_count = 1;
    counties[2].neighbours[0] = 1;
}

#[test]
fn a_capture_reports_the_takers_holding_and_peak_from_before_the_write() {
    let (mut counties, mut realms) = world();
    bordering(&mut counties);
    let mut units = Units::new();
    counties[2].owner = 2;
    realms[1].peak_counties = 1;

    let c = change_owner(T, &mut counties, &mut realms, &mut units, 1, 2, 0, &CampaignMap::empty(), &mut Explored::new(), Restore::NEUTRAL);
    assert_eq!((c.new_owner, c.old_owner, c.county), (1, 2, 2));
    assert_eq!(c.held_before, 1, "county 1, and not the county being taken");
    assert_eq!(c.held_after(), 2);
    assert_eq!(c.peak_before, 1);
    assert!(c.governable, "county 2 borders county 1");
    assert_eq!(realms[1].peak_counties, 2, "the peak rises to the new holding");
}

#[test]
fn the_peak_remembers_ground_lost_and_retaking_it_is_not_a_new_high() {
    let (mut counties, mut realms) = world();
    bordering(&mut counties);
    let mut units = Units::new();
    counties[2].owner = 1;
    realms[1].peak_counties = 2;

    let lost = change_owner(T, &mut counties, &mut realms, &mut units, 2, 2, 0, &CampaignMap::empty(), &mut Explored::new(), Restore::NEUTRAL);
    assert_eq!(lost.held_before, 0, "realm 2 held nothing, so it may take anything");
    assert!(lost.governable);
    assert_eq!(realms[1].peak_counties, 2, "the loser's peak stands");

    let back = change_owner(T, &mut counties, &mut realms, &mut units, 1, 2, 0, &CampaignMap::empty(), &mut Explored::new(), Restore::NEUTRAL);
    assert_eq!((back.held_after(), back.peak_before), (2, 2), "back to the peak, not past it");
    assert_eq!(realms[1].peak_counties, 2);
}

#[test]
fn a_county_far_from_the_takers_lands_declares_independence_instead() {
    let (mut counties, mut realms) = world();
    bordering(&mut counties);
    let mut units = Units::new();
    realms[1].peak_counties = 1;
    counties[3].population = 500;
    counties[3].happiness = 77;
    counties[3].owner = 2;
    counties[3].industry[0].enabled = true;
    counties[3].castle_switch = true;

    let far = change_owner(T, &mut counties, &mut realms, &mut units, 1, 3, 0, &CampaignMap::empty(), &mut Explored::new(), Restore::NEUTRAL);

    assert!(!far.governable, "county 3 has no neighbours at all");
    assert_eq!(far.old_owner, 2, "the letter still knows who lost it");
    assert_eq!(counties[3].owner, 0, "nobody's, not the taker's");
    assert_eq!(realms[1].peak_counties, 1, "the peak is not raised");
    assert_eq!(realms[1].county_count, 1, "and neither is the count");
    assert_eq!(counties[3].happiness, 77, "the conquest penalty is in the other branch");
    assert!(
        counties[3].industry.iter().all(|i| !i.enabled),
        "`County_MakeIndependent` switches all four industries off",
    );
    assert!(!counties[3].castle_switch, "and `+0x1B0` with them");
}

#[test]
fn a_realm_that_holds_nothing_keeps_a_county_it_cannot_reach() {
    let (mut counties, mut realms) = world();
    bordering(&mut counties);
    let mut units = Units::new();
    counties[3].population = 500;
    counties[3].happiness = 77;

    let first = change_owner(T, &mut counties, &mut realms, &mut units, 2, 3, 0, &CampaignMap::empty(), &mut Explored::new(), Restore::NEUTRAL);
    assert!(first.governable, "realm 2 held nothing");
    assert_eq!(counties[3].owner, 2);
}

#[test]
fn a_second_change_owner_on_a_county_already_held_counts_it_twice() {
    let (mut counties, mut realms) = world();
    bordering(&mut counties);
    let mut units = Units::new();
    realms[1].peak_counties = 1;
    change_owner(T, &mut counties, &mut realms, &mut units, 1, 2, 0, &CampaignMap::empty(), &mut Explored::new(), Restore::NEUTRAL);
    let again = change_owner(T, &mut counties, &mut realms, &mut units, 1, 2, 0, &CampaignMap::empty(), &mut Explored::new(), Restore::NEUTRAL);
    assert_eq!(again.old_owner, 1, "nothing changes hands the second time");
    assert_eq!(again.held_before, 2, "the county is counted as already held");
    assert_eq!(realms[1].peak_counties, 3, "and the peak is one past the truth");
    assert_eq!(realms[1].county_count, 2, "while the recount is not");
}

#[test]
fn taking_a_county_lifts_the_losers_garrison() {
    let (mut counties, mut realms) = world();
    let mut units = Units::new();
    let g = units.spawn(Unit::new(UnitKind::Army, 2, 0, 0)).unwrap();
    counties[2].owner = 2;
    counties[2].garrison_unit = g;
    // Both halves of the link, which is the only state `Army_GarrisonApply`
    // can leave: `FUN_00437535` clears the pair and tests neither.
    units.get_mut(g).unwrap().garrison_county = 2;
    let taken = change_owner(T, &mut counties, &mut realms, &mut units, 1, 2, 0, &CampaignMap::empty(), &mut Explored::new(), Restore::NEUTRAL);
    assert_eq!(counties[2].garrison_unit, 0);
    assert!(matches!(taken.garrison, Some(LeftCastle::Marched { .. })));
    // The loser keeps the men: `FUN_00437535` writes no owner byte.
    assert_eq!(units.get(g).map(|u| (u.owner, u.garrison_county)), Some((2, 0)));
}

