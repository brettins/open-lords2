use super::*;
use l2_game::screens::saveload::{FRONT_END, CANCEL};
use l2_game::screens::setup::item_rect;

fn to_load_page(assets: &Assets, game: &mut Game) -> SetupScreen {
    let mut screen = SetupScreen::new(SetupPage::Options);
    let r = item_rect(1);
    click(&mut screen, game, assets, r.x + 20, r.y + r.h / 2);
    assert_eq!(screen.page(), SetupPage::Load, "FUN_004335F0 hotspot 4 opens page 3");
    screen
}

/// SaveLoad_Cancel 0x00434308 `g_setupPage = DAT_00553E5C`, and Screen_FrameInput
/// 00420000.c:7461-7466 the right release: ablate `load_closed`'s Pop arm and both fail.
#[test]
fn the_front_end_load_box_closes_back_to_page_2() {
    let assets = assets!();
    let mut game = Game::new(scenario::SEED);
    let mut screen = to_load_page(&assets, &mut game);
    let (dx, dy) = (FRONT_END.0 - 0x10, FRONT_END.1 - 0x90);
    click(&mut screen, &mut game, &assets, FRONT_END.0 + 0x24, FRONT_END.1 + 0x70);
    assert_eq!(screen.page(), SetupPage::Load, "a click on the file list stays on page 3");
    click(&mut screen, &mut game, &assets, CANCEL.0 + dx + 4, CANCEL.1 + dy + 4);
    assert_eq!(screen.page(), SetupPage::Options);

    let mut screen = to_load_page(&assets, &mut game);
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    screen.handle(Event::RightClick { x: 300, y: 300 }, &mut ctx);
    assert_eq!(screen.page(), SetupPage::Options);
}

/// arm 0x004BA9C8/setup-load-name: page 3's key arm edits DAT_004EA130.
#[test]
fn the_front_end_load_box_takes_a_typed_name() {
    let assets = assets!();
    let mut game = Game::new(scenario::SEED);
    let mut screen = to_load_page(&assets, &mut game);
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    for c in ['z', 'q'] {
        screen.handle(Event::Text(c), &mut ctx);
    }
    assert_eq!(screen.page(), SetupPage::Load, "typing stays on page 3");
    assert!(screen.load_name().contains("zq"), "{:?}", screen.load_name());
}
