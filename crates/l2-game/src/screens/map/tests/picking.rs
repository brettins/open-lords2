#![allow(unused_imports)]
use super::*;
use super::layout::*;
use super::super::*;

#[test]
fn the_diamonds_leave_no_pixel_unpicked() {
    for zoom in [NEAR, FAR] {
        let mut s = MapScreen::new();
        s.zoom = zoom;
        s.view = Viewport::new(60, 30).clamped(&zoom);
        let (tx, ty) = (1..l2_kingdom::MAP_DIM - 1)
            .flat_map(|y| (1..l2_kingdom::MAP_DIM - 1).map(move |x| (x, y)))
            .find(|&(x, y)| {
                [(x, y), (x + 1, y), (x, y + 1), (x - 1, y), (x, y - 1)].iter().all(|&(a, b)| {
                    campaign::tile_centre(s.view, &zoom, a, b).is_some_and(|(cx, cy)| {
                        let c = s.map_clip();
                        cx - zoom.pitch > c.x0
                            && cx + zoom.pitch < c.x1
                            && cy - zoom.row_step * 2 > c.y0
                            && cy + zoom.row_step * 2 < c.y1
                    })
                })
            })
            .expect("a tile with room around it");
        let (cx, cy) = campaign::tile_centre(s.view, &zoom, tx, ty).expect("in view");

        let mut misses = Vec::new();
        for dy in -zoom.row_step..=zoom.row_step {
            for dx in -zoom.half_pitch..=zoom.half_pitch {
                if s.pick_tile(cx + dx, cy + dy).is_none() {
                    misses.push((dx, dy));
                }
            }
        }
        assert!(
            misses.is_empty(),
            "zoom {}: {} pixels around a tile centre pick nothing, first at {:?} \
             — the diamonds do not tile the plane",
            zoom.id,
            misses.len(),
            misses.first(),
        );
    }
}

#[test]
fn the_pick_diamond_is_the_lattice_and_not_the_picture() {
    for zoom in [NEAR, FAR] {
        assert_eq!(zoom.half_pitch * 2, zoom.pitch, "zoom {}", zoom.id);
    }
    assert_eq!(NEAR.tile_w / 2, 29);
    assert_eq!(NEAR.half_pitch, 30);
    assert_eq!(FAR.tile_w / 2, 5);
    assert_eq!(FAR.half_pitch, 6);
}

#[test]
fn the_screen_opens_at_the_originals_zoom_and_scroll_origin() {
    let s = MapScreen::new();
    assert_eq!(s.zoom().id, NEAR.id);
    assert_eq!(s.viewport(), Viewport::new(0x4A, 0x14));
    assert_eq!(s.map_clip(), NEAR.clip());
}

#[test]
fn the_zoom_toggle_saves_and_restores_the_near_views_position() {
    let assets = crate::game::Assets::placeholder();
    let mut game = crate::Game::new(1);
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    let mut s = MapScreen::new();
    s.view = Viewport::new(60, 30);
    s.toggle_zoom(&mut ctx);
    assert_eq!(s.zoom().id, FAR.id);
    assert!(ctx.game.map_zoom_far, "the far zoom did not reach Game");
    assert_eq!(
        s.viewport(),
        Viewport::new(0, 14),
        "row clamps to 0, col 0x0E stands"
    );
    assert!(!s.scroll(Dir::E));
    assert!(!s.scroll(Dir::N));
    s.toggle_zoom(&mut ctx);
    assert_eq!(s.zoom().id, NEAR.id);
    assert!(!ctx.game.map_zoom_far, "the near zoom did not reach Game");
    assert_eq!(s.viewport(), Viewport::new(60, 30), "back where it was");
    assert!(s.scroll(Dir::E), "and the near view scrolls again");
    assert_eq!(s.viewport(), Viewport::new(60, 31));
}

/// Ablation: make `mode_screen_id` answer `None`
/// goes red.
#[test]
fn picking_up_an_army_is_screen_0x10_and_putting_it_down_is_not() {
    let assets = crate::game::Assets::placeholder();
    let mut game = crate::Game::new(1);
    let unit = game
        .kingdom
        .campaign
        .units
        .spawn(l2_kingdom::unit::Unit::new(l2_kingdom::UnitKind::Army, 1, 10, 10))
        .expect("a free slot");
    let mut s = MapScreen::new();
    assert_eq!(s.mode_screen_id(), None, "the map is 0, which its ScreenId already says");
    let ctx = Ctx { game: &mut game, assets: &assets };
    s.begin_move_selection(&ctx, unit);
    assert_eq!(s.mode_screen_id(), Some(0x10));
    s.cancel_move_selection();
    assert_eq!(s.mode_screen_id(), None);
}

