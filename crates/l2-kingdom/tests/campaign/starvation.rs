#![allow(unused_imports)]
use super::*;
use super::mercenaries::*;
use super::mines::*;
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
fn a_starving_army_warns_then_deserts_three_times_then_dies() {
    let mut k = kingdom();
    k.options = Options { difficulty: 0, advanced_farming: false, armies_eat: true, ..Options::default() };
    k.counties[1].herd = 0;
    k.counties[1].grain = 0;
    k.counties[1].herd_available = 0;
    k.counties[1].grain_available = 0;
    let id = army(&mut k, 1, 1, 700, 5, 5);
    for t in 0..7 {
        k.campaign.units.get_mut(id).unwrap().troops[t] = 100;
    }

    let mut seen = Vec::new();
    let mut men = Vec::new();
    for _ in 0..5 {
        let mut report = l2_kingdom::report::SeasonReport::new();
        k.run_pass(Pass::WagesPay, &mut report);
        k.counties[1].herd = 0;
        k.counties[1].grain = 0;
        seen.push(
            report
                .messages
                .iter()
                .filter_map(|m| match m {
                    Message::ArmyStarving { stage, .. } => Some(*stage),
                    _ => None,
                })
                .collect::<Vec<_>>(),
        );
        men.push(k.campaign.units.get(id).map(|u| u.men));
    }

    assert_eq!(seen, vec![vec![1], vec![2], vec![3], vec![4], vec![5]]);
    assert_eq!(men[0], Some(700), "the first season only warns");
    assert_eq!(men[1], Some(630));
    assert_eq!(men[2], Some(567));
    assert_eq!(men[3], Some(511));
    assert_eq!(men[4], None, "the fifth season destroys it");
    assert_eq!(k.realms[1].wages, 0, "and the bill goes with it");
}

#[test]
fn a_garrisoned_army_is_fed_by_the_castle() {
    let mut k = kingdom();
    k.options.armies_eat = true;
    k.counties[1].herd = 0;
    k.counties[1].grain = 0;
    let id = army(&mut k, 1, 1, 400, 5, 5);
    k.campaign.units.get_mut(id).unwrap().garrison_county = 1;

    for _ in 0..8 {
        let mut report = l2_kingdom::report::SeasonReport::new();
        k.run_pass(Pass::WagesPay, &mut report);
        assert!(report.messages.iter().all(|m| !matches!(m, Message::ArmyStarving { .. })));
    }
    assert_eq!(k.campaign.units.get(id).unwrap().men, 400);
    assert_eq!(k.campaign.units.get(id).unwrap().starvation, 0);
}

#[test]
fn with_foraging_off_nothing_starves_at_all() {
    let mut k = kingdom();
    k.options.armies_eat = false;
    k.counties[1].herd = 0;
    k.counties[1].grain = 0;
    let id = army(&mut k, 1, 1, 900, 5, 5);
    k.campaign.units.get_mut(id).unwrap().starvation = 3;

    let mut report = l2_kingdom::report::SeasonReport::new();
    k.run_pass(Pass::WagesPay, &mut report);
    assert_eq!(k.campaign.units.get(id).unwrap().starvation, 0);
    assert_eq!(k.campaign.units.get(id).unwrap().men, 900);
}

#[test]
fn an_armys_food_cost_scales_with_the_countys_ration_level() {
    let eaten = |level: i32, troops: i32| {
        let mut k = kingdom();
        k.options.armies_eat = true;
        k.counties[1].ration_wanted = level;
        k.counties[1].friendly_troops = troops;
        k.counties[1].herd = 0;
        k.counties[1].grain = 10_000;
        k.counties[1].ration_split = 0; // all grain, so the answer is in sacks
        let mut report = l2_kingdom::report::SeasonReport::new();
        k.run_pass(Pass::RationApply, &mut report);
        k.counties[1].grain_eaten
    };

    let normal_alone = eaten(3, 0);
    let normal_with = eaten(3, 300);
    let triple_alone = eaten(5, 0);
    let triple_with = eaten(5, 300);

    let normal_cost = normal_with - normal_alone;
    let triple_cost = triple_with - triple_alone;
    assert!(normal_cost > 0, "the troops eat something");
    assert_eq!(
        triple_cost,
        normal_cost * 3,
        "three hundred men cost three times as much at Triple rations \
         ({normal_cost} -> {triple_cost}), which a flat subtraction could not do"
    );
}

#[test]
fn an_occupying_army_eats_the_countys_food() {
    let mut k = kingdom();
    k.options.armies_eat = true;
    k.counties[1].ration_split = 0;
    k.counties[1].herd = 0;
    k.counties[1].grain = 10_000;

    let mut report = l2_kingdom::report::SeasonReport::new();
    k.run_pass(Pass::RationApply, &mut report);
    let alone = k.counties[1].grain_eaten;

    let mut k = kingdom();
    k.options.armies_eat = true;
    k.counties[1].ration_split = 0;
    k.counties[1].herd = 0;
    k.counties[1].grain = 10_000;
    k.counties[1].enemy_troops = 400;
    let mut report = l2_kingdom::report::SeasonReport::new();
    k.run_pass(Pass::RationApply, &mut report);
    assert!(k.counties[1].grain_eaten > alone, "the invader is fed by the invaded");
}

#[test]
fn marching_an_army_across_a_border_moves_which_county_feeds_it() {
    let mut k = kingdom();
    let id = army(&mut k, 1, 1, 300, 30, 10);
    let realms = k.realms.clone();
    k.campaign.units.recount_county_troops(&mut k.counties, &realms);
    assert_eq!(k.counties[1].friendly_troops, 300);
    assert_eq!(k.counties[2].enemy_troops, 0);

    l2_kingdom::movement::order_move(&k.campaign.map, &mut k.campaign.units, id, (33, 10), Routing::Direct);
    let Kingdom { campaign, counties, realms, .. } = &mut k;
    let realms_snapshot = realms.clone();
    l2_kingdom::movement::march(&mut campaign.map, counties, &realms_snapshot, &mut campaign.units, id);
    assert_eq!(k.campaign.units.get(id).unwrap().county, 2);

    let realms = k.realms.clone();
    k.campaign.units.recount_county_troops(&mut k.counties, &realms);
    assert_eq!(k.counties[1].friendly_troops, 0);
    assert_eq!(k.counties[2].enemy_troops, 300, "an unowned county sees it as an enemy");
}

