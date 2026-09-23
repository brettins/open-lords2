
mod widget_tests;
mod page_quirks;
mod slider_tests;

use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::press;
use l2_game::screen::{Ctx, Screen, ScreenId, Transition};
use l2_game::screens::options::{self, OptionsScreen, Page, Setting};
use l2_game::Game;

pub(crate) fn world() -> (Game, Assets) {
    (Game::new(1), Assets::placeholder())
}

const ORIGINALS: [Page; 4] = [Page::Advanced, Page::Sound, Page::Display, Page::Help];

pub(crate) fn click(screen: &mut OptionsScreen, game: &mut Game, assets: &Assets, x: i32, y: i32) -> Transition {
    let mut ctx = Ctx { game, assets };
    screen.handle(Event::Click { x, y }, &mut ctx)
}

fn press_and_wait(
    screen: &mut OptionsScreen,
    game: &mut Game,
    assets: &Assets,
    x: i32,
    y: i32,
) -> Transition {
    let mut ctx = Ctx { game, assets };
    let mut out = screen.handle(Event::Click { x, y }, &mut ctx);
    let released = screen.handle(Event::Release { x, y }, &mut ctx);
    if out == Transition::Stay {
        out = released;
    }
    for _ in 0..press::DELAYED_FRAMES {
        let t = screen.update(&mut ctx);
        if out == Transition::Stay {
            out = t;
        }
    }
    out
}

fn mid(r: l2_game::input::Rect) -> (i32, i32) {
    (r.x + r.w / 2, r.y + r.h / 2)
}

fn value(setting: Setting, game: &mut Game, assets: &Assets) -> bool {
    options::value(setting, &Ctx { game, assets })
}


