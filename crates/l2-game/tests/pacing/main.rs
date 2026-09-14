//! **How fast the campaign map moves**, measured through the real screen
//! machine.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" LORDS2_FIXTURES="E:\dev\lords2-fixtures" \
//!     cargo test -p l2-game --test pacing
//! ```
//!
//! # Why this file exists beside `tests/scenario.rs`'s merchant walk
//!
//! `a_merchant_walks_its_route_over_the_frames_of_a_turn_rather_than_teleporting`
//! asserts three things: a turn takes many ticks, a merchant stands on several
//! distinct tiles across them, and it never enters more than one tile in a
//! tick. All three are true of a merchant that stands perfectly still for
//! thirty-four frames and then crosses eleven tiles in eleven — which is
//! exactly what a player reported:
//!
//! > *"The merchants don't move right when you click End Turn, and then… move
//! > insanely fast. That's a bit odd."*
//!
//! It is `docs/agents.md`'s *test that passes for an accidental reason*, and
//! the accident is that it is written in **tiles per tick**. Tiles per tick was
//! never wrong: `Unit_Step` enters at most one tile per call and always has.
//! What was missing is the other half of `Unit_StepOnce` (`0x0046634D`) — the
//! sub-tile counter that decides *how many ticks a tile takes* — so the unit
//! of measure that catches it is **ticks per tile**, and the unit that catches
//! the frame driver getting it wrong instead is **ticks per frame**.
//!
//! Both are here, and neither can be satisfied by the other.
//!
//! # The numbers are typed, not computed
//!
//! `SUBTILE_SPAN / SUBTILE_STEP_SOLO = 8` admissions a tile, and a road admits
//! one tick in one. A test that wrote that expression would pass with every
//! constant in it ablated to 1 — `docs/agents.md`, *ablating a constant while
//! computing your probe from that same constant tests nothing at all* — so the
//! 8 below is a literal, and this is where it comes from:
//!
//! ```c
//! /* Unit_StepOnce, 0x0046634D, the arm that does not enter a tile */
//! cVar1 = onRoad ? 0 : 3;
//! if (cVar1 < ++field_0x14a) {
//!     field_0x14a = 0;
//!     field_0x149 += (g_multiplayer == 0) ? 2 : 4;
//!     if (field_0x149 >= 0x10) { field_0x14b |= 1; field_0x149 = 0; return 2; }
//! }
//! return 1;
//! ```
//!
//! Sixteen in twos is eight admissions; a road admits every tick and open
//! ground one in four. **8 ticks a road tile, 32 an open one.** `[V]`


use l2_game::audio::{self, Audio};
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::scenario;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_kingdom::tables::Tables;
use l2_kingdom::UnitKind;

/// The fewest ticks any unit can take to enter a tile: eight admissions of the
/// sub-tile counter, one admitted per tick because the unit is on a road.
/// Typed; see the module note.
const FEWEST_TICKS_PER_TILE: u32 = 8;

/// How long a turn is allowed to run before this file gives up on it. Well
/// under `turn::MAX_TICKS` and well over the England turn's real length.
const GIVE_UP: u32 = 1_500;

macro_rules! england {
    () => {{
        let save = l2_testkit::england!();
        let mut game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
        // **Tip screens: No.** This file watches the map for a whole turn, and
        // a new game's tips come up over it and hold its input on screen
        // `0x27`, which is right and is `tests/tips.rs`'s subject.
        game.prefs.tip_screens = false;
        game
    }};
}

mod pacing;
pub use pacing::*;

fn merchant_slots(game: &l2_game::Game) -> Vec<usize> {
    game.kingdom
        .campaign
        .units
        .iter()
        .filter(|(_, u)| u.kind == UnitKind::Merchant)
        .map(|(id, _)| id)
        .collect()
}

