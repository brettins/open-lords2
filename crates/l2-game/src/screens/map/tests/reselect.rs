#![allow(unused_imports)]
use super::*;
use super::super::*;
use super::super::reselect::busiest_county;

fn loaded(view: (i32, i32)) -> Option<crate::Game> {
    let save = l2_testkit::fixture_save("battle-before.sav").ok()?;
    let mut game = crate::scenario::from_save(&save, l2_kingdom::tables::Tables::DEFAULT).unwrap();
    game.loaded_view = Some(view);
    Some(game)
}

/// Map_DrawFrame 0x004050F6 00400000.c:2844 runs on the first campaign frame after a load at
/// zoom 0. battle-before.sav saves county 3; at (88, 29) county 4 fills the view.
/// Ablation: drop `self.reselect_busiest(ctx)` in `update` and county 3 stays.
#[test]
fn a_load_whose_county_is_off_screen_reselects_on_the_first_frame() {
    let Some(mut game) = loaded((88, 29)) else { return };
    assert_eq!(game.selected, 3);
    let assets = crate::game::Assets::placeholder();
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    let mut s = MapScreen::new();
    s.update(&mut ctx);
    assert_eq!(ctx.game.selected, 4);
}

/// 00400000.c:2828 `g_countyTileTally[g_selectedCounty] = 0x1e`: at (10, 20) county 1 outdraws
/// county 3 by fewer than 30. Ablation: drop the `0x1e` head start and county 1 wins.
#[test]
fn the_selected_county_keeps_a_thirty_tile_head_start() {
    let Some(game) = loaded((10, 20)) else { return };
    let m = &game.kingdom.campaign.map;
    assert_eq!(busiest_county(m, Viewport::new(10, 20), &NEAR, 0), 1, "county 1 has more tiles");
    assert_eq!(busiest_county(m, Viewport::new(10, 20), &NEAR, 3), 3);
}

/// `if (... && g_mapZoom == 0)`: the far view never reselects.
/// Ablation: drop the `self.zoom.id != NEAR.id` return and county 3 is replaced.
#[test]
fn the_far_view_keeps_the_selection() {
    let Some(mut game) = loaded((88, 29)) else { return };
    let assets = crate::game::Assets::placeholder();
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    let mut s = MapScreen::new();
    s.opened = true;
    s.zoom = FAR;
    s.view = Viewport::new(0, 0);
    s.update(&mut ctx);
    assert_eq!(ctx.game.selected, 3);
}

/// Map_RenderIso 0x0040526E reads `g_screenLattice`; `cell_to_tile` inverts `tile_to_cell`.
#[test]
fn cell_to_tile_inverts_tile_to_cell() {
    for y in 0..64 {
        for x in 0..64 {
            let (r, c) = campaign::tile_to_cell(x, y);
            assert_eq!(campaign::cell_to_tile(r, c), Some((x, y)));
        }
    }
    assert_eq!(campaign::cell_to_tile(0, 0), None);
}
