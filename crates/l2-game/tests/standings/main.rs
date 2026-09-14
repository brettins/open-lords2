//! **The standings** — `Screen_GreatestNoble` (`0x0041593B`), screen `0x20`,
//! and the court button that opens it.
//!
//! A player reported the *Greatest nobles* button in the treasury view as
//! dead. It was: the button was drawn, `court.rs` answered it with
//! `Transition::Stay`, and screen `0x20` did not exist. So this file has two
//! halves and the first one is the smaller:
//!
//! * **the button answers** — a kind-5 press, the handler twenty ticks later,
//! the recount `FUN_00435211` runs before the page is drawn, and the page on
//!   the stack;
//! * **the page is right** — its geometry against the player's own
//! `Lords2.exe` and `Flags.pl8`, and the seven scoring rules of
//! `FUN_00415E42` and the ranking of `FUN_00415BDC` against values built by
//!   hand.
//!
//! The rules are tested by hand-built realms
//! purpose: `docs/decisions.md` C26 — every fixture is turn one with one
//! county each, so **every category is a five-way tie** and the only line the
//! page would ever show from one is *"undecided."*. A test driven by the
//! fixture alone would assert the page's least interesting state and call the
//! screen covered.

mod scoring;
pub use scoring::*;
mod geometry;
pub use geometry::*;
mod interaction;
pub use interaction::*;

use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::nobles::{self, NoblesScreen};
use l2_game::Game;
use l2_kingdom::realm::Realm;
use l2_kingdom::tables::SCORE_INPUT_CASTLES;

// ---------------------------------------------------------------- the rules

/// Five realms in play, every field zero, so a test can set the one it means.
fn five_realms() -> Vec<Realm> {
    let mut realms = vec![Realm::new(); l2_kingdom::MAX_REALMS];
    for (i, r) in realms.iter_mut().enumerate() {
        r.in_play = i != 0;
        r.strength = if i == 0 { 0 } else { 1 };
        r.shield_index = i as u8;
    }
    realms
}

// ------------------------------------------------------------- the behaviour

fn bare_world() -> (Game, Assets) {
    let mut game = Game::new(5);
    game.prefs.tip_screens = false;
    (game, Assets::placeholder())
}

