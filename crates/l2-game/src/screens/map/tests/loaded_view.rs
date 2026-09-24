#![allow(unused_imports)]
use super::*;
use super::super::*;

/// `FUN_004976A1` (`0x004976A1`) keeps the saved `g_mapStartRow`/`Col` (34, 36 in
/// battle-before.sav): the original's right click at the army's feet (228, 272), where
/// `Map_DrawArmies` (`0x00408438`) drew it, picks unit 5's tile (33, 17). Ablation: drop the
/// `loaded_view` arm of `open_on_the_player` and the pick lands elsewhere.
#[test]
fn a_loaded_map_opens_where_the_save_left_it() {
    let Ok(save) = l2_testkit::fixture_save("battle-before.sav") else { return };
    let mut game = crate::scenario::from_save(&save, l2_kingdom::tables::Tables::DEFAULT).unwrap();
    let assets = crate::game::Assets::placeholder();
    let ctx = Ctx { game: &mut game, assets: &assets };
    let mut s = MapScreen::new();
    s.open_on_the_player(&ctx);
    assert_eq!(s.view, Viewport::new(34, 36));
    let c = campaign::tile_centre(s.view, &s.zoom, 33, 17);
    assert_eq!(s.pick_tile(228, 272), Some((33, 17)), "centre of (33,17) is {c:?}");
}

/// Map_DrawTile 0x004063C1 blits `g_tiles[t].frame` from the bank `g_tiles[t].bank & 0x1c`;
/// a loaded save's block 0 is `g_tiles`. Ablation: drop `add_saved_frames` and the map file's
/// frame is drawn (65 of 263 England tiles another variant, draw diff 2026-09-24).
#[test]
fn a_loaded_map_draws_the_saves_tile_frames() {
    let Ok(save) = l2_testkit::fixture_save("battle-before.sav") else { return };
    let mut game = crate::scenario::from_save(&save, l2_kingdom::tables::Tables::DEFAULT).unwrap();
    let assets = crate::game::Assets::placeholder();
    let ctx = Ctx { game: &mut game, assets: &assets };
    let out = MapScreen::tile_graphics(&ctx);
    for tile in 0..l2_kingdom::map::MAP_TILES {
        let t = save.tile(tile).unwrap();
        let (x, y) = l2_kingdom::map::coords(tile);
        assert_eq!(out.get(x as usize, y as usize), Some((t[2], t[3])), "tile {tile}");
    }
}
