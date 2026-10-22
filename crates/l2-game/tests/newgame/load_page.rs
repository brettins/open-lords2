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

/// SaveLoad_DrawStatus 0x004149EC `FUN_004b414a(x - 2, y - 1, 0x3f)`; Widget_Draw from g_systemSheet,
/// System2.pl8 on the front end (`Res_LoadButtons(0)`). Red on ablation of `OUTLINE` as the bar's
/// colour, or of the `is_front_end` arm in `draw_status`.
#[test]
fn the_front_end_load_box_draws_a_0x3f_bar_and_system2_buttons() {
    use l2_game::screens::saveload::{CONFIRM, FRONT_END_SYSTEM, OUTLINE, SCROLL_UP};
    let assets = assets!();
    let dir = std::env::temp_dir().join(format!("l2-screen-fixes-load-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("a.{}", l2_game::save::EXTENSION)), b"x").unwrap();
    let _scope = l2_game::saves::scoped_dir(&dir);
    let mut game = Game::new(scenario::SEED);
    let mut screen = to_load_page(&assets, &mut game);
    click(&mut screen, &mut game, &assets, FRONT_END.0 + 0x24, FRONT_END.1 + 0x70);
    let mut canvas = l2_view::Canvas::screen();
    screen.draw(&Ctx { game: &mut game, assets: &assets }, &mut canvas);
    let _ = std::fs::remove_dir_all(&dir);
    let row = (FRONT_END.0 + 0x20, FRONT_END.1 + 0x6C);
    assert_eq!(canvas.at((row.0 - 2) as usize, (row.1 - 1) as usize), OUTLINE);
    let (dx, dy) = (FRONT_END.0 - 0x10, FRONT_END.1 - 0x90);
    let sheet = assets.shell.sheet(FRONT_END_SYSTEM).expect("System2.pl8 is in the install");
    for w in [CONFIRM, SCROLL_UP] {
        let f = sheet.frame(w.2).unwrap();
        let (fw, fh) = (f.width as usize, f.height as usize);
        let mut seen = 0;
        for (i, _) in f.opaque.iter().enumerate().filter(|(_, &o)| o) {
            let (x, y) = ((w.0 + dx) as usize + i % fw, (w.1 + dy) as usize + i / fw);
            assert_eq!(canvas.at(x, y), f.indices[i], "frame {} at ({x}, {y}) of {fw}x{fh}", w.2);
            seen += 1;
        }
        assert!(seen > 0, "frame {} has art", w.2);
    }
}
