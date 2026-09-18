//! What was missing is the other half of `Unit_StepOnce` (`0x0046634D`) — the
//! sub-tile counter that decides *how many ticks a tile takes* — so the unit
//! of measure that catches it is **ticks per tile**, and the unit that catches
//! the frame driver getting it wrong instead is **ticks per frame**.
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

const FEWEST_TICKS_PER_TILE: u32 = 8;

const GIVE_UP: u32 = 1_500;

macro_rules! england {
    () => {{
        let save = l2_testkit::england!();
        let mut game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
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

