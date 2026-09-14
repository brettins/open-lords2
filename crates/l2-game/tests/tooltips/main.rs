//! **The tool tips, played.**
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-game --test tooltips
//! ```
//!
//! `FUN_00476E95` (`0x00476E95`) and the six functions under it
//! built: the Help Options panel's *"Tool tips"* row flipped a flag nothing
//! read. `crate::tooltip` has the decompilation.
//!
//! What is asserted is the subject: **which tip** a pointer position gets on
//! which screen, **the tick** it appears on, **what takes it away**, **where the
//! box is**, and **the words in the box** — never a whole canvas. Every number
//! the oracle gives is typed as a literal read from the constant
//! under test (`docs/agents.md`, *ablating a constant while computing your probe
//! from that same constant tests nothing at all*).
//!
//! Tip screens are off in every world here, because a tip screen is `g_screenId
//! 0x27` and `DAT_004D6FB8[0x27]` is 0 — the one test that is about that turns
//! one on.

mod behavior;
pub use behavior::*;
mod rendering;
pub use rendering::*;
mod battle;
pub use battle::*;

use l2_game::game::Assets;
use l2_game::input::{Event, Rect};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::county::Panel;
use l2_game::screens::options::{self, Setting};
use l2_game::shell::Pen;
use l2_game::tooltip::{self, Shown, Sidebar};
use l2_game::Game;
use l2_kingdom::unit::{TroopType, Unit, UnitKind};
use l2_view::Canvas;

// ---------------------------------------------------------------------- setup

fn tick(m: &mut Machine, g: &mut Game, a: &Assets) {
    let mut ctx = Ctx { game: g, assets: a };
    m.update(&mut ctx);
}

fn ticks(m: &mut Machine, g: &mut Game, a: &Assets, n: usize) {
    for _ in 0..n {
        tick(m, g, a);
    }
}

fn send(m: &mut Machine, g: &mut Game, a: &Assets, e: Event) {
    let mut ctx = Ctx { game: g, assets: a };
    m.handle(e, &mut ctx);
}

fn point(m: &mut Machine, g: &mut Game, a: &Assets, x: i32, y: i32) {
    send(m, g, a, Event::Pointer { x, y });
}

pub(crate) fn world() -> Game {
    let mut g = Game::new(0x7195);
    g.player = 1;
    // **Tip screens: No.** See the module header.
    g.prefs.tip_screens = false;
    g.kingdom.set_county_count(6);
    l2_testkit::chain_neighbours!(g.kingdom);
    g.kingdom.counties[1].owner = 1;
    g.selected = 1;
    g
}

fn campaign() -> (Game, Assets, Machine) {
    (world(), Assets::placeholder(), Machine::new(ScreenId::Campaign))
}

/// The End Turn strip, `y 460 …`: `FUN_00477320`'s last arm, id 14.
const END_TURN: (i32, i32) = (560, 470);

fn shown(m: &Machine) -> Option<Shown> {
    m.tooltips().shown()
}

// ------------------------------------------------------------------ the rest

