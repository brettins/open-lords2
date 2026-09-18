#![allow(unused_imports)]
use super::*;
use super::combat::*;
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
fn an_army_ordered_through_the_game_actually_moves_when_the_turn_is_ended() {
    let mut g = with_a_map();
    let id = army(&mut g, 1, 5, 10);
    let from = g.kingdom.campaign.units.get(id).unwrap().tile();

    assert_eq!(g.order_unit_move(id, (20, 10)), Some(15), "fifteen road tiles");

    let outcome = turn::end_turn(&mut g).expect("the machine comes round");

    // **It moved, and it did not arrive — and the second half is the rule, not
    // a shortfall.** Fifteen road tiles is 113 ticks since `Unit_StepOnce`'s
    // sub-tile counter landed (`docs/decisions.md` **C134**), and
    // *nothing in the seven phases waits on the human's armies* — phase 2 is
    // sieges, phase 4 is the AI's. So how far it gets is how long the phases
    // happen to take, and the claim this test exists for is the one above it:
    //
    // `mid.0 < 20` is the pacing assertion and it is the one to ablate: make
    // `cross_sub_tile` return `true` unconditionally and the army arrives
    // inside the turn,
    let mid = g.kingdom.campaign.units.get(id).unwrap().tile();
    assert_ne!(mid, from, "the army did not move at all");
    assert_eq!(mid.1, 10, "along the road it was given");
    assert!(mid.0 > from.0 && mid.0 < 20, "part-way, not arrived: {mid:?}");
    assert!(outcome.steps >= 1, "{} tiles entered over the turn", outcome.steps);
    assert!(outcome.pending_battles.is_empty(), "nobody was in the way");

    // **And the turn stopped it where it stood.** `Units_ResetMoves`
    // (`0x004651B9`) is phase 7 and its loop is unconditional —
    // `g_units[i].moving = 0; g_units[i].movesUsed = 0;` over all 150 slots —
    // so an unfinished march is **dropped at the season boundary, not carried
    // across it**. `[V]`, and it is the reason the fixture below is the shape
    // it is: a player watches his army walk during his own turn (phase 4 has
    // no clock but the turn timer) and presses End Turn after it has arrived.
    assert!(!g.kingdom.campaign.units.get(id).unwrap().moving, "phase 7 stopped it");

    g.order_unit_move(id, (20, 10)).expect("still on the road");
    march(&mut g);
    let to = g.kingdom.campaign.units.get(id).unwrap().tile();
    assert_eq!(to, (20, 10), "and it arrived");
}

#[test]
fn another_realms_army_refuses_the_players_orders() {
    let mut g = with_a_map();
    let theirs = army(&mut g, 2, 5, 10);
    assert_eq!(g.order_unit_move(theirs, (20, 10)), None);
    assert!(!g.kingdom.campaign.units.get(theirs).unwrap().moving);
    turn::end_turn(&mut g).unwrap();
    assert_eq!(g.kingdom.campaign.units.get(theirs).unwrap().tile(), (5, 10));
}

