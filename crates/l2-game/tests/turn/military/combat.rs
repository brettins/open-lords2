#![allow(unused_imports)]
use super::*;
use super::army_movement::*;
use super::tile_effects::*;
use super::merchants_and_mobs::*;
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
fn two_enemy_armies_meeting_during_a_turn_fight_a_battle() {
    let mut g = with_a_map();
    let mine = army(&mut g, 1, 5, 10);
    let theirs = army(&mut g, 2, 6, 10);
    g.order_unit_move(mine, (20, 10)).expect("a road runs the whole way");

    let outcome = turn::end_turn(&mut g).unwrap();

    assert!(
        outcome.pending_battles.is_empty(),
        "the encounter reached engagement::resolve: {:?}",
        outcome.pending_battles
    );
    assert!(
        outcome.contacts.iter().any(|c| matches!(c, Contact::Battle(_))),
        "and the sweep reported it: {:?}",
        outcome.contacts
    );

    let alive = [mine, theirs]
        .iter()
        .filter(|&&id| g.kingdom.campaign.units.get(id).is_some_and(|u| u.men > 0))
        .count();
    assert_eq!(alive, 1, "a battle has one survivor, not two and not none");
}

#[test]
fn the_attacker_does_not_enter_the_defenders_tile() {
    let mut g = with_a_map();
    let mine = army(&mut g, 1, 5, 10);
    let theirs = army(&mut g, 2, 6, 10);
    g.kingdom.campaign.units.get_mut(theirs).unwrap().men = 1200;
    g.kingdom.campaign.units.get_mut(theirs).unwrap().troops[0] = 1200;
    g.order_unit_move(mine, (20, 10)).unwrap();

    turn::end_turn(&mut g).unwrap();

    assert!(g.kingdom.campaign.units.get(mine).is_none(), "the attacker lost and is gone");
    assert_eq!(
        g.kingdom.campaign.units.get(theirs).unwrap().tile(),
        (6, 10),
        "and the defender is still on its own tile"
    );
}

