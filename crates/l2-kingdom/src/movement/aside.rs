use super::pathfinding::*;
use super::Routing;
use crate::map::{CampaignMap, CostMap};
use crate::unit::{UnitKind, Units};

/// Unit_LinkToTile 0x0046EDDF: `playerDriven = 1` on an empty tile, 0 when chained behind
/// another unit; a garrisoned army is never linked (`if (kind != 1 || garrisonCounty == 0)`) [V].
pub fn link_to_tile(units: &mut Units, id: usize) {
    let Some(u) = units.get(id) else { return };
    if u.kind == UnitKind::Army && u.is_garrisoned() {
        return;
    }
    let (x, y) = u.tile();
    let alone = !units.iter().any(|(i, o)| i != id && o.x == x && o.y == y && !o.is_garrisoned());
    if let Some(u) = units.get_mut(id) {
        u.player_driven = alone;
    }
}

/// FUN_00467196 0x00467196 (00460000.c:4818-4841), from Unit_Step and Castle_EvictTile: a unit
/// chained on a shared tile walks to Map_FindFreeTileNear's tile, refunded the ring in moves,
/// `moving = 2`, flooding the stored g_moveCost (Move_ExtractPath 0x004701AC) with no Move_BuildCostMap [V].
pub fn step_aside(cost: &CostMap, map: &CampaignMap, units: &mut Units, id: usize) -> bool {
    let Some(u) = units.get(id) else { return false };
    let Some((ring, tile)) = crate::divide::free_tile_ring(map, units, u.x, u.y) else { return false };
    let field = flood_fill(cost, u.tile(), Routing::Direct);
    let path = extract_path(cost, &field, tile);
    let Some(u) = units.get_mut(id) else { return false };
    u.moves_used = if u.moves_used < ring { 0 } else { u.moves_used - ring };
    u.moving = true;
    u.ordered = false;
    u.dest = Some(tile);
    if let Some(path) = path {
        u.path = path;
    }
    true
}

/// Unit_Step 00460000.c:4368 reads path[0] with pathLen 0; Path_CopyToUnit 0x004707BE zeroed it on
/// an empty copy, so Dir_FromDelta 0x0046F434 aims at (0, 0) [V]. A walked-out path leaves the unit's
/// own tile there (Dir 8); told apart by `dest` not being that tile [I]. Merchants (Merchant_AdvanceAll
/// 0x004280E9) and transports only: Army_Tick 0x0046521F restarts a stopped army, whose path[0] is then the spent entry [I].
pub fn zeroed_head(units: &mut Units, id: usize) -> bool {
    let Some(u) = units.get_mut(id) else { return false };
    if !matches!(u.kind, UnitKind::Merchant | UnitKind::Transport)
        || !u.path.is_empty() || u.dest.is_none_or(|d| d == u.tile()) || (u.x, u.y) == (0, 0) {
        return false;
    }
    u.path = vec![(u.x - (u.x > 0) as u8, u.y - (u.y > 0) as u8)];
    true
}

/// Castle_EvictTile 0x0046841C (00460000.c:5489), from Castle_Order after Castle_StampTile: the unit
/// heading each of the castle's 2×2 tiles is stepped aside [V]; `units.at` is the lowest slot there,
/// not the chain head [I].
pub fn evict_castle_plot(cost: &CostMap, map: &CampaignMap, units: &mut Units, county: u8) {
    for t in crate::map::castle_plot_tiles(map, county) {
        let (x, y) = crate::map::coords(t);
        if let Some(id) = units.at(x, y) {
            step_aside(cost, map, units, id);
        }
    }
}
