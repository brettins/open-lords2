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
mod conquest;
mod eng_part;
mod font_part;
mod glyph;

use std::path::PathBuf;

use l2_game::game::Assets;
use l2_game::shell::Eng;
use l2_game::Game;

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


