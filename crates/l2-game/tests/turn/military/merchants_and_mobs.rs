#![allow(unused_imports)]
use super::*;
use super::army_movement::*;
use super::combat::*;
use super::tile_effects::*;
use super::purity::*;
use super::*;
use super::spine::*;
use l2_kingdom::phase::Phase;
use l2_kingdom::realm::AI_STEP_DONE;
use l2_kingdom::tables::Tables;
use l2_game::turn;
use l2_game::Game;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::units_tick::Contact;
use l2_kingdom::merchant::MerchantRoutes;
use l2_kingdom::unit::{Unit, UnitKind};

#[test]
fn a_merchant_walks_its_route_across_several_turns() {
    let mut g = with_a_map();
    let mut routes = MerchantRoutes::none();
    routes.set_route(0, &[1, 4, 7, 10]);
    g.kingdom.campaign.routes = routes;

    let mut m = Unit::new(UnitKind::Merchant, 6, 2, 10);
    m.county = 1;
    m.needs_destination = true;
    m.year_formed = 1; // the route cursor
    let id = g.kingdom.campaign.units.spawn(m).expect("slot 1");
    assert_eq!(id, 1, "the route row is the slot minus one");

    let mut counties = Vec::new();
    for _ in 0..6 {
        turn::end_turn(&mut g).unwrap();
        counties.push(g.kingdom.campaign.units.get(id).unwrap().county);
    }

    assert!(
        counties.iter().any(|&c| c != 1),
        "the merchant never left its county: {counties:?}"
    );
    let mut visited: Vec<u8> = Vec::new();
    for &c in &counties {
        if visited.last() != Some(&c) {
            visited.push(c);
        }
    }
    assert!(
        visited.windows(2).all(|w| w[0] < w[1]),
        "it walked east along its route without doubling back: {visited:?}"
    );
    assert!(visited.contains(&4), "it reached the first leg's county: {visited:?}");
}

#[test]
fn a_unit_loaded_with_no_allowance_still_walks() {
    let mut g = with_a_map();
    for realm in 2..=5 {
        g.kingdom.realms[realm].in_play = false;
    }
    let id = army(&mut g, 1, 5, 10);
    g.order_unit_move(id, (12, 10)).unwrap();
    g.kingdom.campaign.units.get_mut(id).unwrap().move_allowance = 0;

    // `Unit_Step` (`0x00465D28`) keeps the unit walking through that tile and
    // stops it at the tile's edge, eight road ticks later.
    let ticks = march(&mut g);
    assert_eq!(ticks, 57, "seven road tiles: the first free, eight for each of the six, eight to cross the last");

    turn::end_turn(&mut g).unwrap();

    assert_eq!(g.kingdom.campaign.units.get(id).unwrap().tile(), (12, 10));
    assert_eq!(g.kingdom.campaign.units.get(id).unwrap().move_allowance, 15);
}

#[test]
fn the_turn_moves_mobs_and_transports_nobody_ordered() {
    let mut g = with_a_map();
    g.kingdom.season = 2; // spring: every mob is re-targeted

    let mut mob = Unit::new(UnitKind::PeasantMob, 6, 2, 10);
    mob.county = 1;
    mob.men = 200;
    let mob_id = g.kingdom.campaign.units.spawn(mob).unwrap();

    let mut t = Unit::new(UnitKind::Transport, 1, 3, 10);
    t.county = 1;
    t.cargo_county = 5;
    let transport = g.kingdom.campaign.units.spawn(t).unwrap();

    turn::end_turn(&mut g).unwrap();

    assert_ne!(
        g.kingdom.campaign.units.get(mob_id).unwrap().tile(),
        (2, 10),
        "the mob was given a destination by phase 5 and walked"
    );
    assert_ne!(
        g.kingdom.campaign.units.get(transport).unwrap().tile(),
        (3, 10),
        "the transport was re-targeted by phase 3 and walked"
    );
    assert_eq!(
        g.kingdom.campaign.units.get(transport).unwrap().dest_county,
        5,
        "at its cargo county"
    );
}


