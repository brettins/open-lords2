//! *The array onto a screen that is about the player*, which is how the court
//! came to draw `LORD1` with all four of the above green
//! ([`the_court_is_headed_with_the_name_the_player_typed`], `docs/decisions.md`
//! C189). The list was the chain, and the end of the list was not the end of
//! the chain. The fifth hand-off happens on **five** screens and every one of
//! them now goes through one function, `screens::message::lord_name` — the
//! court, the battle prompt, the county strip, the diplomacy screen and its
//! compose dialogs. The three that were not on it drew a `REALM n` of their
//! own; the county strip's and the diplomacy screen's are asserted in
//! `tests/screens_*.rs`, where the install can supply the `L2.eng` group 7 that
//! stands behind the array.

mod setup;
mod persistence;
mod rendering;

use l2_game::game::{Assets, Game};
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Screen, Transition};
use l2_game::screens::setup::{SetupPage, SetupScreen};

pub(crate) fn bare() -> (Game, Assets) {
    (Game::new(1), Assets::placeholder())
}

fn type_into(m: &mut SetupScreen, game: &mut Game, assets: &Assets, s: &str) -> Vec<Transition> {
    let mut out = Vec::new();
    for c in s.chars() {
        let key = if c == ' ' { Key::Space } else { Key::letter(c) };
        for e in [Event::KeyDown(key), Event::Text(c)] {
            let mut ctx = Ctx { game, assets };
            out.push(m.handle(e, &mut ctx));
        }
    }
    out
}

fn press(m: &mut SetupScreen, game: &mut Game, assets: &Assets, key: Key) -> Transition {
    let mut ctx = Ctx { game, assets };
    m.handle(Event::KeyDown(key), &mut ctx)
}

fn name_page(game: &mut Game, assets: &Assets) -> SetupScreen {
    let mut m = SetupScreen::new(SetupPage::Title);
    let mut ctx = Ctx { game, assets };
    // C255: *Multiple players* opens the lobby; page 4 is reached through page 2's custom game.
    m.handle(Event::KeyDown(Key::Enter), &mut ctx);
    for _ in 0..3 {
        m.handle(Event::KeyDown(Key::Down), &mut ctx);
    }
    m.handle(Event::KeyDown(Key::Enter), &mut ctx);
    assert_eq!(m.page(), SetupPage::Shield, "page 2's custom game opens page 4");
    m
}

fn field_of(m: &SetupScreen) -> String {
    m.name()
}

fn pushed(ts: &[Transition]) -> bool {
    ts.iter().any(|t| matches!(t, Transition::Push(_)))
}

