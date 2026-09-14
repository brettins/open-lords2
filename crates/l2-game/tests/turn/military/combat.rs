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

/// **Two units meet during a played turn, and the battle is fought.**
///
/// This was written as the seam's marker while battle resolution was on
/// another branch: it asserted one *unfought* pair in `pending_battles`, so
/// that filling the seam in would change a test
/// It did exactly that. `turn::resolve_battle` now calls
/// `engagement::resolve`, so what a played turn produces is a result, not a
/// note — one of the two armies is destroyed and `pending_battles` is empty.
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

    // Exactly one of the two is left standing. The autocalc destroys the
    // loser, and `Units::get` returns `None` for a slot that has been freed.
    let alive = [mine, theirs]
        .iter()
        .filter(|&&id| g.kingdom.campaign.units.get(id).is_some_and(|u| u.men > 0))
        .count();
    assert_eq!(alive, 1, "a battle has one survivor, not two and not none");
}

/// The attacker never walks onto the tile it is attacking — the step returns
/// before the move is committed, which is what lets the army resume from where
/// it stood if the battle leaves it alive.
#[test]
fn the_attacker_does_not_enter_the_defenders_tile() {
    let mut g = with_a_map();
    let mine = army(&mut g, 1, 5, 10);
    // A defender big enough that the attacker loses and is destroyed, so the
    // survivor is the one that never moved.
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

