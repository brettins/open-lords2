//! The county strip, the sidebar and the county panels.
//!
//! Split out of `tests/screens.rs`; the shared helpers are in `tests/common/`.


#[macro_use]
#[path = "../common/mod.rs"]
mod common;

mod strip_and_sidebar;
pub use strip_and_sidebar::*;
mod panels;
pub use panels::*;
mod drawing_and_emboss;
pub use drawing_and_emboss::*;
mod produce_and_pastures;
pub use produce_and_pastures::*;
mod layout_and_labels;
pub use layout_and_labels::*;

use common::*;

use l2_game::game::Assets;
use l2_game::game::MAX_TAX_RATE;
use l2_game::input::Event;
use l2_game::input::Key;
use l2_game::screen::Ctx;
use l2_game::screen::Machine;
use l2_game::screen::Screen;
use l2_game::screen::ScreenId;
use l2_game::screen::Transition;
use l2_game::screens::county::{self as county};
use l2_game::screens::county::CountyScreen;
use l2_game::screens::county::Panel;
use l2_game::screens::map;
use l2_game::screens::map::MapScreen;
use l2_game::screens::village::{self as village_screen};
use l2_game::screens::village::VillageScreen;
use l2_game::shell::font;
use l2_game::Game;
use l2_view::campaign;
use l2_view::Canvas;

/// **Run the frames a turn takes.** Pressing End Turn only starts one — the
/// phase machine is wound on once per fixed tick and the season fade follows —
/// so a test that wants the numbers afterwards has to tick. See
/// `l2_game::turn::TurnRun`.
///
/// **Both halves of a frame**, in `Machine::update`'s order:
/// `Screen::wind_turn` is `Battle_Frame`'s `Turn_Tick(); Units_Tick();`, which
/// the driver runs whatever is on top, and `Screen::update` is
/// `Screen_FrameInput`'s, which it runs for the top screen only.
fn run_turn<S: Screen>(screen: &mut S, game: &mut Game, assets: &Assets) {
    let before = game.kingdom.turn_count;
    let mut done_at = None;
    for n in 1..2_000u32 {
        let mut ctx = Ctx { game, assets };
        screen.wind_turn(&mut ctx);
        screen.update(&mut ctx);
        if done_at.is_none() && game.kingdom.turn_count > before {
            done_at = Some(n);
        }
        if let Some(t) = done_at {
            if n >= t + l2_view::fade::PHASES as u32 {
                return;
            }
        }
    }
    panic!("the turn never came round");
}

/// And the one exception: the achieved ration when it is not the wanted one.
const STRIP_BAD: u8 = l2_game::shell::font::HIGHLIGHT;

