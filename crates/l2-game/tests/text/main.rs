//! **Can a player type their name?**
//!
//! ```text
//! cargo test -p l2-game --test text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-game --test text
//! ```
//!
//! The report was one line — *"I can't type my name in the start menu?"* — and
//! the answer was that this workspace had no keyboard text entry at all. So the
//! first test here is the report, driven the way a person drives it: through
//! [`Machine::handle`] with [`Event`] values, no window, no focus, no cursor
//! (`docs/agents.md`).
//!
//! # What the rest of the file is guarding against
//!
//! `docs/agents.md` names the failure this feature is shaped exactly like:
//! **a field is only tested if something a test reads was written by something
//! the game runs.** Six instances so far, every one behind a green suite. A
//! typed name has to survive four separate hand-offs and a test that skips any
//! of them is checking its own fixture:
//!
//! 1. the keystroke into the field ([`typing_a_name_reaches_the_field`]);
//! 2. the field into `g_playerNames` ([`start_puts_the_typed_name_into_the_realm`]);
//! 3. `g_playerNames` into the file and back
//!    ([`a_typed_name_survives_the_save_and_the_reload`]);
//! 4. the field onto the screen ([`the_name_and_its_caret_are_painted`]).
//!
//! Only (2) needs the install, because only *Start* needs a map to build.
//!
//! **There is a fifth, and it was not on this list while it was broken.**
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
pub use setup::*;
mod persistence;
pub use persistence::*;
mod rendering;
pub use rendering::*;

use l2_game::game::{Assets, Game};
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Screen, Transition};
use l2_game::screens::setup::{SetupPage, SetupScreen, NAME_PLATE_X, NAME_PLATE_Y, NAME_X};
use l2_game::text::{FontMetrics, Kind, PlayerName, TextField, NAME_MAX_TYPED, PLAYER_NAME_LEN};
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::Tables;
use l2_view::Canvas;

/// Everything the front end needs and nothing else. `Assets::placeholder` is
/// used only where the assertion is about a *value*; the drawing tests below
/// refuse it by name, because `docs/agents.md` records that every campaign-map
/// test ran on the placeholder, the one configuration where a broken hit test
/// and the picture agree.
fn bare() -> (Game, Assets) {
    (Game::new(1), Assets::placeholder())
}

/// Type a string the way `main.rs` delivers it: `WM_KEYDOWN` **and** `WM_CHAR`,
/// in that order, for every printable key.
///
/// **Both, deliberately.** Sending only `Event::Text` would let a screen that
/// wrongly acts on the `KeyDown` half pass — which is the bug this pair was
/// introduced to make impossible, and it was live for one compile: `Space`
/// typed a space *and* pressed the highlighted button.
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

/// The setup screen on page 4, **reached the way a person reaches it** — the
/// title menu's *Multiple players*, whose arm is one of the three that runs
/// `Edit_Begin`. Building `SetupScreen::new(SetupPage::Shield)` would skip the
/// seeding and test a field nothing had opened.
fn name_page(game: &mut Game, assets: &Assets) -> SetupScreen {
    let mut m = SetupScreen::new(SetupPage::Title);
    let mut ctx = Ctx { game, assets };
    m.handle(Event::KeyDown(Key::Down), &mut ctx);
    m.handle(Event::KeyDown(Key::Enter), &mut ctx);
    assert_eq!(m.page(), SetupPage::Shield, "the title menu's second item opens page 4");
    m
}

fn field_of(m: &SetupScreen) -> String {
    m.name()
}

fn pushed(ts: &[Transition]) -> bool {
    ts.iter().any(|t| matches!(t, Transition::Push(_)))
}

