use super::*;
use crate::game::{Assets, Game};
use crate::saves::Entry;

fn opened_from(opener: SetupPage) -> SetupScreen {
    let mut screen = SetupScreen::new(opener);
    screen.go(SetupPage::Load);
    assert_eq!(screen.page, SetupPage::Load);
    screen
}

/// SaveLoad_Cancel 0x00434308 / Screen_FrameInput 00420000.c:7465 `g_setupPage = DAT_00553e5c`:
/// ablate `self.load_opener = self.page` in `go` and Custom returns to Options.
#[test]
fn the_load_box_returns_to_the_page_that_opened_it() {
    let assets = Assets::placeholder();
    let mut game = Game::new(crate::scenario::SEED);
    for opener in [SetupPage::Options, SetupPage::Custom, SetupPage::CustomMulti] {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        let mut screen = opened_from(opener);
        screen.load_event(Event::RightClick { x: 300, y: 300 }, &mut ctx);
        assert_eq!(screen.page, opener, "right release from {opener:?}");

        let mut screen = opened_from(opener);
        screen.load_closed(Transition::Pop);
        assert_eq!(screen.page, opener, "Cancel from {opener:?}");
        assert!(screen.load.is_none());
    }
}

/// Screen_FrameInput 00420000.c:7462 `if (FUN_0043400b(1) != 0) goto`: ablate the
/// `load_list_hit` early return and the right release in the same frame closes the box.
#[test]
fn a_file_picked_this_frame_swallows_the_right_release() {
    let assets = Assets::placeholder();
    let mut game = Game::new(crate::scenario::SEED);
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    let mut screen = opened_from(SetupPage::Custom);
    let entry = Entry { name: "save00".into(), path: "save00.l2sav".into(), bytes: 0 };
    screen.load_box().set_entries(vec![entry]);
    let r = screen.load_box().row(0);
    screen.load_event(Event::Click { x: r.x + 2, y: r.y + r.h / 2 }, &mut ctx);
    screen.load_event(Event::RightClick { x: 300, y: 300 }, &mut ctx);
    assert_eq!(screen.page, SetupPage::Load, "same frame: FUN_0043400b(1) wins");
    screen.load_update(&mut ctx);
    screen.load_event(Event::RightClick { x: 300, y: 300 }, &mut ctx);
    assert_eq!(screen.page, SetupPage::Custom, "next frame: DAT_00553e5c");
}

/// FUN_004335F0 0x004335F0 00430000.c:981 latches the map click; FUN_00432CC8 :625-637 does not.
/// Ablate: `ctx.game.map_click_latch = true;` in the Custom pages' Load arm.
#[test]
fn the_skirmish_pages_load_latches_the_map_click_and_the_options_pages_does_not() {
    let assets = Assets::placeholder();
    for (opener, item, latched) in [(SetupPage::Custom, 3, true), (SetupPage::CustomMulti, 3, true), (SetupPage::Options, 1, false)] {
        let mut game = Game::new(crate::scenario::SEED);
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        let mut screen = SetupScreen::new(opener);
        screen.item(item, &mut ctx);
        assert_eq!(screen.page, SetupPage::Load, "{opener:?}");
        assert_eq!(ctx.game.map_click_latch, latched, "{opener:?}");
    }
}
