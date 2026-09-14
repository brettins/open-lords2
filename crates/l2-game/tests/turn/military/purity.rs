#![allow(unused_imports)]
use super::*;
use super::army_movement::*;
use super::combat::*;
use super::tile_effects::*;
use super::merchants_and_mobs::*;
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

/// Movement does not cost determinism. The same kingdom, with the same orders,
/// ended twice, lands on the same tiles — the property `docs/netcode.md` binds
/// and the one a unit sweep is most likely to break.
#[test]
fn a_turn_with_units_moving_is_still_a_pure_function_of_where_it_started() {
    let orders = |g: &mut Game| {
        let a = army(g, 1, 5, 10);
        let b = army(g, 1, 40, 10);
        let mut m = Unit::new(UnitKind::Merchant, 6, 2, 10);
        m.county = 1;
        m.year_formed = 1;
        g.kingdom.campaign.units.spawn(m).unwrap();
        g.kingdom.campaign.routes.set_route(0, &[1, 4, 7]);
        g.order_unit_move(a, (20, 10)).unwrap();
        g.order_unit_move(b, (25, 10)).unwrap();
    };

    let mut x = with_a_map();
    let mut y = with_a_map();
    orders(&mut x);
    orders(&mut y);
    assert_eq!(x.kingdom, y.kingdom, "the two start equal");

    let ox = turn::end_turn(&mut x).unwrap();
    let oy = turn::end_turn(&mut y).unwrap();
    assert_eq!(ox.ticks, oy.ticks);
    assert_eq!(ox.steps, oy.steps);
    assert_eq!(ox.contacts, oy.contacts);
    assert_eq!(x.kingdom, y.kingdom);

    // And a second turn, because a divergence that only shows on the next one
    // is what a single-turn test misses.
    turn::end_turn(&mut x).unwrap();
    turn::end_turn(&mut y).unwrap();
    assert_eq!(x.kingdom, y.kingdom);
}

