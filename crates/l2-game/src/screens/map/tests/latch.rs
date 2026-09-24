#![allow(unused_imports)]
use super::*;
use super::super::*;

fn far_map(ctx: &mut Ctx) -> (MapScreen, i32, i32, i32) {
    let _ = ctx;
    let mut s = MapScreen::new();
    s.zoom = FAR;
    s.view = Viewport::new(0, 14).clamped(&FAR);
    let c = s.map_clip();
    (s, (c.x0 + c.x1) / 2, (c.y0 + c.y1) / 2, c.x1 + 20)
}

// Screen_FrameInput 00420000.c:6795, Map_Click :7669: a network game's release after End Turn
// never reaches the clear [D] netcode. Ablate: `!closed &&` in the `Event::Release` arm, red.
#[test]
fn a_release_after_end_turn_leaves_the_latch_set() {
    let assets = crate::game::Assets::placeholder();
    let mut game = crate::Game::new(1);
    game.multiplayer = true;
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    let (mut s, x, y, _) = far_map(&mut ctx);
    s.handle(Event::Click { x, y }, &mut ctx);
    ctx.game.turn = Some(Default::default());
    s.handle(Event::Release { x, y }, &mut ctx);
    ctx.game.turn = None;
    assert!(matches!(s.handle(Event::RightClick { x, y }, &mut ctx), Transition::Stay));
}

// App_WinMain 00400000.c:8248 DAT_00553018 = 1: in single player the gate stays open after End
// Turn and the release clears the latch. Ablate: map_input_shut's single-player arm made
// `turn_in_flight(game)`, red.
#[test]
fn a_single_player_release_after_end_turn_clears_the_latch() {
    let assets = crate::game::Assets::placeholder();
    let mut game = crate::Game::new(1);
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    let (mut s, x, y, _) = far_map(&mut ctx);
    s.handle(Event::Click { x, y }, &mut ctx);
    ctx.game.turn = Some(Default::default());
    s.handle(Event::Release { x, y }, &mut ctx);
    ctx.game.turn = None;
    assert!(!ctx.game.map_click_latch, "Map_Click :7669 ran");
}

// FUN_0043CAF4 00430000.c:7507: a right release at far zoom zooms in, and Map_ZoomInAtTile :2522 latches.
// Ablate: the far-zoom block in the right-click arm, first assertion red;
// its `ctx.game.map_click_latch = true;`, second red.
#[test]
fn a_far_right_click_zooms_in_and_latches() {
    let assets = crate::game::Assets::placeholder();
    let mut game = crate::Game::new(1);
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    let (mut s, x, y, _) = far_map(&mut ctx);
    assert!(matches!(s.handle(Event::RightClick { x, y }, &mut ctx), Transition::Stay));
    assert_eq!(s.zoom.id, NEAR.id);
    assert!(matches!(s.handle(Event::RightClick { x, y }, &mut ctx), Transition::Stay));
    assert!(matches!(s.handle(Event::RightClick { x, y }, &mut ctx), Transition::Push(_)));
}

// Map_Click 00430000.c:7670 spends the latch on a left click on the map.
// Ablate: the `if ctx.game.map_click_latch` test before `map_click_dismiss`, red.
#[test]
fn a_left_click_on_the_map_spends_the_latch() {
    let assets = crate::game::Assets::placeholder();
    let mut game = crate::Game::new(1);
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    let (mut s, x, y, off) = far_map(&mut ctx);
    s.handle(Event::Click { x, y }, &mut ctx);
    s.handle(Event::Release { x: off, y }, &mut ctx);
    s.handle(Event::Click { x, y }, &mut ctx);
    assert!(matches!(s.handle(Event::RightClick { x, y }, &mut ctx), Transition::Push(_)));
}
