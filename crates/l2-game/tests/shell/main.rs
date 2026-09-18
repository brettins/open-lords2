//! * **Fidelity**, which needs the install: `L2.eng` really does say what the
//!   painters' `(group, index)` pairs claim, the two fonts really do map
//!   characters the way `g_glyphWidths` says, and that table really is the
//!   224 bytes at `0x004D71F0` in the user's own `Lords2.exe`. Those
//!   skip without `LORDS2_DIR`.
//!
//! The second kind is what makes the first kind mean anything. A shell that
//! draws group 11 index 0 is only worth having if group 11 index 0 is
//! *"Lords of the Realm 2"*.

mod navigation;
pub use navigation::*;
mod conquest;
pub use conquest::*;
mod eng_part;
pub use eng_part::*;
mod font_part;
pub use font_part::*;
mod glyph;
pub use glyph::*;

use std::path::PathBuf;

use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, Screen, ScreenId};
use l2_game::screens::county::Panel;
use l2_game::screens::conquest::ConquestScreen;
use l2_game::screens::setup::{self, SetupPage};
use l2_game::screens::court::CourtScreen;
use l2_game::screens::ratings::RatingsScreen;
use l2_game::shell::{font, Eng};
use l2_game::Game;
use l2_view::Canvas;

fn install() -> Option<PathBuf> {
    l2_testkit::install_dir()
}

fn eng() -> Option<Eng> {
    let dir = install()?;
    Eng::parse(std::fs::read(dir.join("L2.eng")).ok()?).ok()
}

pub(crate) fn bare() -> (Game, Assets) {
    (Game::new(1), Assets::placeholder())
}


