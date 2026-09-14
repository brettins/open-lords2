//! **The options panels, and the quirks page.**
//!
//! Driven with [`Event`] values, never with a real pointer — nothing here
//! touches the OS input queue and nothing needs a window.
//!
//! Three things this file is written to catch, all of which have already
//! reached a player from other screens:
//!
//! * **a near-miss acting anyway.** `crates/l2-game/src/screens/map/mod.rs`'s header
//!   records three wrong-screen bugs in one evening, every one a click that fell
//!   through to county selection. A page of check boxes is a page of small
//!   hotspots with the same exposure, so every test that clicks a box also
//!   clicks *beside* it and asserts nothing moved;
//! * **a window that closes when you click inside it**, reported by a player in
//!   those words;
//! * **a control that acts on the press where the original waits.** Every row
//!   of the four original panels is `Widget_Test` **kind 5** — the picture goes
//! down on the press and the toggle runs twenty frames later — and ours acted
//!   on the click. That is the yes/no gauntlets' defect (*"clicking yes/no is
//!   instant whereas the game waited"*), and `docs/input.md` §4 is the model.

mod widget_tests;
pub use widget_tests::*;
mod page_quirks;
pub use page_quirks::*;

use l2_game::game::{Assets, Prefs, PRESENTATION};
use l2_game::input::{Event, Key};
use l2_game::press;
use l2_game::screen::{Ctx, Machine, Screen, ScreenId, Transition};
use l2_game::screens::options::{self, OptionsScreen, Page, Setting};
use l2_game::Game;
use l2_kingdom::{Quirk, Quirks};

fn world() -> (Game, Assets) {
    (Game::new(1), Assets::placeholder())
}

/// The four pages that are the original's.
const ORIGINALS: [Page; 4] = [Page::Advanced, Page::Sound, Page::Display, Page::Help];

/// Click at a point and return what the screen asked the machine to do.
fn click(screen: &mut OptionsScreen, game: &mut Game, assets: &Assets, x: i32, y: i32) -> Transition {
    let mut ctx = Ctx { game, assets };
    screen.handle(Event::Click { x, y }, &mut ctx)
}

/// **A whole click on a kind-5 row**: press, release, and the twenty ticks the
/// original waits before the handler runs. Returns the first transition that
/// was not `Stay`, from any of the three.
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

/// The middle of a rectangle.
fn mid(r: l2_game::input::Rect) -> (i32, i32) {
    (r.x + r.w / 2, r.y + r.h / 2)
}

fn value(setting: Setting, game: &mut Game, assets: &Assets) -> bool {
    options::value(setting, &Ctx { game, assets })
}

// ---------------------------------------------------------------------------
// The four the original has
// ---------------------------------------------------------------------------

