//! **The gesture kinds, driven through the screens that answer them.**
//!
//! `crates/l2-game/src/press/mod.rs` has unit tests for the state machine and
//! `crates/l2-game/tests/press.rs` pins the ramp's 48 bytes against the
//! player's own `Lords2.exe`.
//! wrong, which is a screen that owns a [`Press`] and never asks it anything:
//! `docs/arms.json` marked nineteen arms `reproduced` under a kind none of them
//! had, and every test in the tree passed.
//!
//! So these tests drive `Machine::handle` and `Machine::update` with the same
//! `Event` values `main.rs` produces, and assert on **what the player sees**:
//! the number does not move until the countdown expires, the arrow keeps
//! stepping while it is held, the picture changes while it is down.
//!
//! # The ablations
//!
//! Each test names the line that must be deleted to turn it red, because a test
//! written against a passing tree has never been observed failing
//! (`docs/agents.md`).

mod gestures;
pub use gestures::*;

use l2_game::game::Assets;
use l2_game::input::{Event, Rect};
use l2_game::press::{self, Kind, Press, Widget};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::county::{self, Panel};
use l2_game::Game;

pub(crate) fn world() -> (Game, Assets) {
    let mut g = Game::new(4);
    g.kingdom.set_county_count(2);
    for id in 1..=2usize {
        let c = &mut g.kingdom.counties[id];
        c.owner = 1;
        c.population = 1_000;
        c.happiness = 90;
        c.grain = 5_000;
        c.herd = 100;
    }
    g.kingdom.realms[1].in_play = true;
    g.kingdom.realms[1].is_human = true;
    g.player = 1;
    g.selected = 1;
    (g, Assets::placeholder())
}

fn send(m: &mut Machine, g: &mut Game, a: &Assets, e: Event) {
    let mut ctx = Ctx { game: g, assets: a };
    m.handle(e, &mut ctx);
}

fn tick(m: &mut Machine, g: &mut Game, a: &Assets) {
    let mut ctx = Ctx { game: g, assets: a };
    m.update(&mut ctx);
}

fn on(r: Rect) -> (i32, i32) {
    (r.centre_x(), r.y + r.h / 2)
}

/// The tax panel, open on county 1.
fn tax_panel() -> (Game, Assets, Machine) {
    let (g, a) = world();
    let m = Machine::new(ScreenId::County(1, Panel::Tax));
    (g, a, m)
}

