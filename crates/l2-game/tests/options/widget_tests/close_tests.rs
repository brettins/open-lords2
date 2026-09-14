#![allow(unused_imports)]
use super::*;
use super::layout_tests::*;
use super::interaction_tests::*;
use super::timing_tests::*;
use super::sound_tests::*;
use super::*;
use super::page_quirks::*;
use l2_game::game::{Assets, Prefs, PRESENTATION};
use l2_game::input::{Event, Key};
use l2_game::press;
use l2_game::screen::{Ctx, Machine, Screen, ScreenId, Transition};
use l2_game::screens::options::{self, OptionsScreen, Page, Setting};
use l2_game::Game;
use l2_kingdom::{Quirk, Quirks};

/// **The close button closes on the release, not the press.**
///
/// All four arms call `Ui_OkButtonClicked` (`0x0040E7E4`), whose first line is
/// `if (g_mouseLeftReleased == 0) return 0;`. Ours closed on the press.
///
/// **Ablation, run:** move the close test back to the `Event::Click` arm and the
/// first assertion goes red on the quirks page and on all four panels.
#[test]
fn the_close_button_answers_the_release_and_not_the_press() {
    for page in Page::ALL {
        let (mut game, assets) = world();
        let mut screen = OptionsScreen::new(page);
        let (x, y) = mid(page.close_hit());
        assert_eq!(click(&mut screen, &mut game, &assets, x, y), Transition::Stay, "{page:?} closed on the press");
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        assert_eq!(screen.handle(Event::Release { x, y }, &mut ctx), Transition::Pop, "{page:?} release");
    }
}

/// The close button closes, and so do `Escape` and a right-click — the
/// original's own two and our one, and no fourth.
#[test]
fn the_panel_closes_three_ways_and_only_three() {
    for page in Page::ALL {
        let (mut game, assets) = world();
        let mut screen = OptionsScreen::new(page);
        let (x, y) = mid(page.close_hit());
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        screen.handle(Event::Click { x, y }, &mut ctx);
        assert_eq!(screen.handle(Event::Release { x, y }, &mut ctx), Transition::Pop, "{page:?} button");
        assert_eq!(
            screen.handle(Event::KeyDown(Key::Escape), &mut ctx),
            Transition::Pop,
            "{page:?} escape"
        );
        assert_eq!(
            screen.handle(Event::RightClick { x: 1, y: 1 }, &mut ctx),
            Transition::Pop,
            "{page:?} right-click"
        );
        // And the one that must NOT close it: Enter and Space, which every
        // shell treated as "confirm". These are not shells any more.
        assert_eq!(screen.handle(Event::KeyDown(Key::Enter), &mut ctx), Transition::Stay);
        assert_eq!(screen.handle(Event::KeyDown(Key::Space), &mut ctx), Transition::Stay);
    }
}

