//! move-cost-local-rebuild: Map_BeginMoveSelection 00430000.c:4033 `Move_BuildCostMap();`
//! goes through `command::BeginMove`, so every seat of a network game rebuilds g_moveCost at one tick.

use super::*;
use l2_game::command::GameCommand;
use l2_kingdom::map::CostMap;

fn a_move_order_opened(lockstep: bool) -> Game {
    let (mut g, a, mut m) = on_the_map();
    let (here, _) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);
    g.net.lockstep = lockstep;
    g.kingdom.campaign.move_cost = CostMap::uniform(0);
    g.begin_move_order = Some(id);
    tick(&mut m, &mut g, &a);
    g
}

/// Ablation: `ctx.issue(BeginMove {})` in the map's `update` back to `rebuild_move_cost()`
/// (the clicking seat's cost map changes at once and nothing goes out).
#[test]
fn opening_a_move_order_in_a_network_game_rebuilds_the_cost_map_by_command() {
    let mut g = a_move_order_opened(true);
    assert_eq!(g.kingdom.campaign.move_cost, CostMap::uniform(0), "rebuilt on the clicking seat alone");
    let sent: Vec<_> = g.net.outbox.drain(..).filter(|c| matches!(c, GameCommand::BeginMove(_))).collect();
    assert_eq!(sent.len(), 1, "the rebuild goes out as an order");
    g.apply(l2_net::PlayerSlot::new(0), &sent[0]);
    assert_eq!(g.kingdom.campaign.move_cost, g.kingdom.campaign.map.cost_map(), "the order rebuilds it");
}

#[test]
fn opening_a_move_order_alone_rebuilds_the_cost_map_at_once() {
    let g = a_move_order_opened(false);
    assert_eq!(g.kingdom.campaign.move_cost, g.kingdom.campaign.map.cost_map());
}
