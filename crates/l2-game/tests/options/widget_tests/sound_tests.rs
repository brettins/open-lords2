#![allow(unused_imports)]
use super::*;
use super::layout_tests::*;
use super::interaction_tests::*;
use super::timing_tests::*;
use super::close_tests::*;
use super::*;
use super::page_quirks::*;
use l2_game::game::{Assets, Prefs, PRESENTATION};
use l2_game::input::{Event, Key};
use l2_game::press;
use l2_game::screen::{Ctx, Machine, Screen, ScreenId, Transition};
use l2_game::screens::options::{self, OptionsScreen, Page, Setting};
use l2_game::Game;
use l2_kingdom::{Quirk, Quirks};

/// **A row clicks on the press and is silent when it acts** — through the
/// machine, which is the wire `audio::Director` listens to.
///
/// `Widget_Test`'s `Sound_RestartSlot(1)` is on the kind-5 press; the countdown
/// that runs the handler twenty frames later plays nothing.
///
/// **Ablation, run:** return `0` from `OptionsScreen::take_clicks` and the first
/// assertion goes red (`left: 0`). Deleting the countdown instead turns the
/// second red, which is what that assertion is for: a silence after a toggle
/// that never ran would prove nothing.
#[test]
fn a_row_clicks_on_the_press_and_is_silent_when_it_acts() {
    let (mut game, assets) = world();
    // The machine runs the tip screens; this test is not about them.
    game.prefs.tip_screens = false;
    let before = game.prefs.music;
    let mut m = Machine::new(ScreenId::Options(Page::Sound));
    let row = Page::Sound.rows().iter().find(|r| r.setting == Setting::Music).unwrap();
    let (x, y) = mid(row.hit());
    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::Click { x, y }, &mut ctx);
        m.handle(Event::Release { x, y }, &mut ctx);
    }
    assert_eq!(m.clicks(), 1, "the press is Widget_Test's kind-5 Sound_RestartSlot(1)");
    for _ in 0..press::DELAYED_FRAMES {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        m.update(&mut ctx);
    }
    assert_ne!(game.prefs.music, before, "the toggle must have run for its silence to mean anything");
    assert_eq!(m.clicks(), 1, "the delayed fire plays nothing");
}

