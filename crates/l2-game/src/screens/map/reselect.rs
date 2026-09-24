use super::*;
use l2_kingdom::map::CampaignMap;

impl MapScreen {
    // Map_DrawFrame 0x004050F6 00400000.c:2844: `if (best != g_selectedCounty && g_mapZoom == 0)`.
    pub(super) fn reselect_busiest(&self, ctx: &mut Ctx) {
        if self.zoom.id != NEAR.id {
            return;
        }
        let best = busiest_county(&ctx.game.kingdom.campaign.map, self.view, &self.zoom, ctx.game.selected);
        if best != ctx.game.selected {
            ctx.game.selected = best;
        }
    }

    // Map_Click 0x0043CE1A 00430000.c:7688-7690: select, Map_CentreOnTile, FUN_004050c0 ->
    // Map_DrawFrame 0x004050F6 reselects; then Village_Draw 0x00412143 on g_selectedCounty [V].
    // The centre is townTile +0x70, County_FindTownTile 0x00467FD1's first tile in index order,
    // not the tile clicked [V].
    pub(super) fn open_village(&mut self, ctx: &mut Ctx, county: u8, clicked: usize) -> Transition {
        let town = Self::town(&Ctx { game: ctx.game, assets: ctx.assets }, county)
            .first()
            .copied()
            .unwrap_or(clicked);
        let (tx, ty) = l2_kingdom::map::coords(town);
        ctx.game.select(county);
        self.centre_on_tile(tx as usize, ty as usize);
        self.reselect_busiest(ctx);
        Transition::Push(ScreenId::Village(ctx.game.selected))
    }
}

/// Map_DrawFrame 0x004050F6: g_countyTileTally zeroed, selection 0x1e (2828), then the scan
/// 1..0x20 by strict `<` (2836-2842). Tallies from Map_RenderAlignedRow 0x00405AE9 (every column)
/// and Map_RenderOffsetRow 0x00405C2F (the cols-1 middle ones); Map_RenderIso 0x0040526E's first
/// and last rows tally nothing. +5 on `g_tiles.flags & 0x40`, else +1 [V].
pub(super) fn busiest_county(map: &CampaignMap, view: Viewport, zoom: &Zoom, selected: u8) -> u8 {
    let mut tally = [0i32; 0x21];
    if let Some(t) = tally.get_mut(selected as usize) {
        *t = 0x1e;
    }
    for dr in 1..zoom.rows {
        let first = dr & 1;
        for dc in first..zoom.cols {
            let Some((x, y)) = campaign::cell_to_tile(view.row + dr, view.col + dc) else { continue };
            let t = l2_kingdom::map::index(x as u8, y as u8);
            let add = if map.flags.get(t).is_some_and(|f| f & 0x40 != 0) { 5 } else { 1 };
            if let Some(n) = map.county.get(t).and_then(|&c| tally.get_mut(c as usize)) {
                *n += add;
            }
        }
    }
    let (mut best, mut most) = (selected, 0);
    for (c, &n) in tally.iter().enumerate().take(0x20).skip(1) {
        if most < n {
            (best, most) = (c as u8, n);
        }
    }
    best
}
