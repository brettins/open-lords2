#![allow(unused_imports)]
use super::*;
use super::starvation::*;
use super::mercenaries::*;
use super::conquest_part::*;
use super::persistence::*;
use l2_kingdom::conquest;
use l2_kingdom::county::MAX_COUNTIES;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::movement::Routing;
use l2_kingdom::phase::Pass;
use l2_kingdom::report::Message;
use l2_kingdom::unit::{Unit, UnitKind, Units};
use l2_kingdom::{Kingdom, MercenaryBands, Options, TroopType};


#[test]
fn a_trampled_mine_is_dead_for_three_seasons_and_then_reopens() {
    let mut k = kingdom();
    k.counties[2].owner = 2;
    k.realms[2].in_play = true;
    k.campaign.map.set_flags(40, 10, flags::SETTLEMENT);
    k.campaign.map.set_terrain(40, 10, 1); // an iron site
    k.counties[2].industry[1].efficiency = 80;

    let id = army(&mut k, 1, 2, 300, 39, 10);
    k.campaign.units.get_mut(id).unwrap().path = vec![(40, 10)];

    let Kingdom { campaign, counties, realms, .. } = &mut k;
    let realms_snapshot = realms.clone();
    let step = l2_kingdom::movement::step(&mut campaign.map, counties, &realms_snapshot, &mut campaign.units, id)
        .expect("a step to take");
    assert_eq!(step.site_ruined, Some((2, 1)));
    assert_eq!(k.counties[2].industry[1].disabled_seasons, 3);
    assert_eq!(k.counties[2].industry[1].efficiency, 0);

    assert_eq!(k.campaign.map.cost_map().at(40, 10), 0);

    let mut left = Vec::new();
    for _ in 0..4 {
        k.advance_season();
        left.push(k.counties[2].industry[1].disabled_seasons);
    }
    assert_eq!(left, vec![2, 1, 0, 0], "three seasons, then it is back");
    assert!(k.counties[2].industry[1].enabled);
}

#[test]
fn a_neutral_countys_trampled_mine_stays_shut_until_somebody_owns_it() {
    let mut k = kingdom();
    assert_eq!(k.counties[2].owner, 0);
    k.counties[2].industry[1].disabled_seasons = 3;

    for _ in 0..6 {
        k.advance_season();
    }
    assert_eq!(k.counties[2].industry[1].disabled_seasons, 3, "nothing ticked it");

    k.counties[2].owner = 1;
    for _ in 0..3 {
        k.advance_season();
    }
    assert_eq!(k.counties[2].industry[1].disabled_seasons, 0);
}

#[test]
fn an_army_cannot_shut_down_its_own_realms_mine() {
    let mut k = kingdom();
    k.counties[2].owner = 1;
    k.campaign.map.set_flags(40, 10, flags::SETTLEMENT);
    k.campaign.map.set_terrain(40, 10, 1);

    let id = army(&mut k, 1, 2, 300, 39, 10);
    k.campaign.units.get_mut(id).unwrap().path = vec![(40, 10)];
    let Kingdom { campaign, counties, realms, .. } = &mut k;
    let realms_snapshot = realms.clone();
    let step = l2_kingdom::movement::step(&mut campaign.map, counties, &realms_snapshot, &mut campaign.units, id)
        .expect("a step to take");
    assert_eq!(step.site_ruined, None);
    assert_eq!(step.charged, 0);
    assert_eq!(k.counties[2].industry[1].disabled_seasons, 0);
}

