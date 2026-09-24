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
