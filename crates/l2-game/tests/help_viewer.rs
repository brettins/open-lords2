//! C241: `Opt_GameHelpContents` 0x00434942's `WinHelpA` as our own screen.

use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::screen::{Ctx, Screen, Transition};
use l2_game::screens::help::{self, HelpScreen};
use l2_game::Game;

fn world() -> Option<(Game, Assets)> {
    let bytes = std::fs::read(l2_testkit::install_dir()?.join("L2help.hlp")).ok()?;
    let mut assets = Assets::placeholder();
    assets.help = Some(l2_formats::hlp::Hlp::parse(&bytes)?);
    Some((Game::new(1), assets))
}

/// (x, y) of the first word of the row whose text is `name`.
fn find(screen: &mut HelpScreen, ctx: &Ctx, name: &str) -> Option<(i32, i32)> {
    let top = screen.top();
    let page = screen.page(ctx)?;
    let i = page.rows.iter().skip(top).take(help::ROWS).position(|r| {
        r.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ") == name
    })?;
    let w = &page.rows[top + i][0];
    Some((help::TEXT.0 + w.x + 1, help::TEXT.1 + i as i32 * help::ROW_H + 4))
}

/// Ablate `self.follow(ctx, hash)` in `handle` and the click leaves the contents up;
/// ablate `2 => self.back.pop()` and Back stays on Battles.
#[test]
fn the_contents_lists_thirteen_chapters_and_battles_opens_battles_and_back_returns() {
    let Some((mut game, assets)) = world() else { l2_testkit::skip!("no L2help.hlp") };
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    let mut s = HelpScreen::new();
    let page = s.page(&ctx).expect("the contents topic");
    let links = page.rows.iter().filter(|r| r.first().is_some_and(|w| w.jump.is_some())).count();
    assert_eq!(links, 13);

    let (x, y) = find(&mut s, &ctx, "Battles").expect("Battles on the first page");
    assert_eq!(s.handle(Event::Click { x, y }, &mut ctx), Transition::Stay);
    assert_ne!(s.offset(), 0, "a link was followed");
    let page = s.page(&ctx).unwrap();
    assert_eq!(page.title, "Battles");
    assert!(page.rows.iter().flatten().any(|w| w.text == "battlefield."), "a Battles word");

    let (bx, by) = (help::BACK.x + 4, help::BACK.y + 4);
    s.handle(Event::Click { x: bx, y: by }, &mut ctx);
    assert_eq!(s.offset(), 0, "Back is the contents again");
}

/// Ablate the `0 | 1 => self.scroll` arm and `top` never moves.
#[test]
fn the_down_arrow_scrolls_and_ok_or_a_right_click_closes() {
    let Some((mut game, assets)) = world() else { l2_testkit::skip!("no L2help.hlp") };
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    let mut s = HelpScreen::new();
    let (x, y) = find(&mut s, &ctx, "Battles").unwrap();
    s.handle(Event::Click { x, y }, &mut ctx);
    let (dx, dy) = (help::DOWN.0 + 4, help::DOWN.1 + 4);
    s.handle(Event::Click { x: dx, y: dy }, &mut ctx);
    s.handle(Event::Release { x: dx, y: dy }, &mut ctx);
    assert_eq!(s.top(), help::STEP);
    let (ox, oy) = (help::OK.0 + 4, help::OK.1 + 4);
    assert_eq!(s.handle(Event::Click { x: ox, y: oy }, &mut ctx), Transition::Pop);
    assert_eq!(s.handle(Event::RightClick { x: 5, y: 5 }, &mut ctx), Transition::Pop);
}

/// The DOS release ships no L2help.hlp: the screen closes itself.
#[test]
fn with_no_help_file_the_screen_closes() {
    let (mut game, assets) = (Game::new(1), Assets::placeholder());
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    assert_eq!(HelpScreen::new().update(&mut ctx), Transition::Pop);
}
