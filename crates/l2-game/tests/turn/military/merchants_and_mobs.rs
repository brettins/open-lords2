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

/// **A merchant walks its route, turn after turn.**
///
/// Four counties on one route, a merchant in slot 1, and six turns. Phase 6
/// gives it the next leg each turn — `Merchant_AdvanceAll` — and the unit sweep
/// walks it. What is asserted is the *sequence*: the merchant reaches the
/// counties its route names, in the order it names them, which is the thing a
/// merchant that merely wandered would fail.
#[test]
fn a_merchant_walks_its_route_across_several_turns() {
    let mut g = with_a_map();
    let mut routes = MerchantRoutes::none();
    // Route row 0 is unit slot 1's, and the cursor starts at 1, so the first
    // destination is the *second* entry: county 4.
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
    // The counties it was standing in at the end of each turn, with repeats
    // collapsed: the order it visited them in.
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

/// The move allowance is **rebuilt every tick from the unit's type**, so a unit
/// that arrived from a save with a zero in the field still walks.
/// `england-turn1.sav` holds exactly that: six merchants, all with
/// `moveAllowance = 0`, because the field is written by the tick handler and
/// never persisted.
///
/// > **The AI realms are taken out of play for this one test**, and the reason
/// > is worth a sentence. `with_a_map` gives every county a border with every
/// > other, so once `l2_kingdom::ai_army` landed the very first turn had realm
/// > 3 raise an army, march it across the map and **destroy this one at (11,
/// > 10) with 83 men to spare**. That is the feature working, and it is not
/// > what this test is about: the subject is the tick handler rewriting
/// > `move_allowance`, and a subject that has been killed by an unrelated rule
/// > cannot be observed. Every other test in this file that ends a turn with a
/// > unit on the board was already immune.
#[test]
fn a_unit_loaded_with_no_allowance_still_walks() {
    let mut g = with_a_map();
    for realm in 2..=5 {
        g.kingdom.realms[realm].in_play = false;
    }
    let id = army(&mut g, 1, 5, 10);
    g.order_unit_move(id, (12, 10)).unwrap();
    g.kingdom.campaign.units.get_mut(id).unwrap().move_allowance = 0;

    // Seven road tiles is 57 ticks and no phase waits on it — see [`march`].
    // The allowance is rebuilt inside the unit sweep, so `march` exercises the
    // subject just as `end_turn` did; what it no longer does is race it.
    //
    // **This was 49**, which is the tick the seventh tile was *entered*.
    // `Unit_Step` (`0x00465D28`) keeps the unit walking through that tile and
    // stops it at the tile's edge, eight road ticks later.
    let ticks = march(&mut g);
    assert_eq!(ticks, 57, "seven road tiles: the first free, eight for each of the six, eight to cross the last");

    turn::end_turn(&mut g).unwrap();

    assert_eq!(g.kingdom.campaign.units.get(id).unwrap().tile(), (12, 10));
    assert_eq!(g.kingdom.campaign.units.get(id).unwrap().move_allowance, 15);
}

/// Phase 5 re-targets the peasant mobs and phase 3 the transports, off their own
/// state
/// touched. These two phases have no other way to be exercised.
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


