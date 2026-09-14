//! **The tip screens, played.**
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-game --test tips
//! ```
//!
//! `Tip_Update` (`0x00476AA7`), `Tip_Show` (`0x00476DA9`) and `FUN_00476E21`
//! (`0x00476E21`), none of which existed here: the advice a new player gets,
//! its words, and the forty narration files that read it out. `crate::tip` has
//! the decompilation.
//!
//! Everything asserted is the subject and nothing is a canvas: **which tip
//! fires for which screen**, the **frame count** of the delay, **where the OK
//! button is** for a given wrap, **the words**, and **the tick each clip starts
//! on**. Every number the oracle gives is typed as a literal
//! from the constant under test, so ablating the constant cannot move the
//! probe with it — `docs/agents.md`.

mod tip_tests;
pub use tip_tests::*;

use std::collections::BTreeMap;

use l2_game::audio::{self, Audio};
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::message::{self, Frame};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::options::{self, Page, Setting};
use l2_game::tip::{self, Tips, View};
use l2_game::Game;
use l2_kingdom::units_tick::Incursion;

// ---------------------------------------------------------------------- setup

fn tick(m: &mut Machine, g: &mut Game, a: &Assets) {
    let mut ctx = Ctx { game: g, assets: a };
    m.update(&mut ctx);
}

fn send(m: &mut Machine, g: &mut Game, a: &Assets, e: Event) {
    let mut ctx = Ctx { game: g, assets: a };
    m.handle(e, &mut ctx);
}

pub(crate) fn world() -> Game {
    let mut g = Game::new(0x7195);
    g.player = 1;
    g.kingdom.set_county_count(6);
    g
}

/// A running game on the campaign map, with nothing posted.
fn campaign() -> (Game, Assets, Machine) {
    (world(), Assets::placeholder(), Machine::new(ScreenId::Campaign))
}

/// Tick until `Tip_Show` posts, and say on which tick it did.
fn ticks_until_posted(m: &mut Machine, g: &mut Game, a: &Assets, limit: usize) -> Option<usize> {
    let before = g.tips.shows();
    for n in 1..=limit {
        tick(m, g, a);
        if g.tips.shows() != before {
            return Some(n);
        }
    }
    None
}

fn open_group(g: &Game) -> Option<u16> {
    g.messages.open().map(|r| r.group)
}

/// `Msg_HandleInput`'s right-button branch: `Msg_Dismiss`, whatever it is.
fn right_click() -> Event {
    Event::RightClick { x: 5, y: 5 }
}

fn view(screen: u8) -> View {
    View { enabled: true, in_play: true, screen: Some(screen), ..View::default() }
}

/// A `Tips` whose start-up twenty frames have already run out.
fn armed() -> Tips {
    let mut t = Tips::new();
    let elsewhere = View { enabled: true, in_play: true, screen: None, ..View::default() };
    for _ in 0..20 {
        assert_eq!(tip::update(&mut t, &elsewhere), None);
    }
    assert_eq!(t.delay(), 0);
    t
}

// ------------------------------------------------------------------ the ladder

