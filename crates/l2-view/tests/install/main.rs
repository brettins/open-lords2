//! Checks the renderer against a real game install, headlessly.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-view
//! ```
//!
//! Skips when unset, so the suite still runs on a machine
//! without the game. **No window is opened and no process is started.** Every
//! assertion here is on a `Vec<u8>` of palette indices, which is also the shape
//! the eventual pixel diff against `Lords2.exe`'s framebuffer will take.
//!
//! Three kinds of check:
//!
//! * **Corpus** — the sprite frame layout is asserted over all 42 shipped `a2`
//!   sheets, not over one that happened to work (`docs/decisions.md` C1).
//! * **Oracle** — the sub-cell walk offsets are read out of `Lords2.exe`'s
//!   `.data` through the PE section headers and compared against the formula
//!   the renderer uses. Prior art and decompiler listings are leads; the bytes
//!   decide (C5, C8).
//! * **Render** — `USER.SKR`'s twenty battlefields are built and drawn, and a
//!   battle is stepped and drawn, with assertions on the resulting pixels.

mod corpus;
pub use corpus::*;
mod oracle;
pub use oracle::*;
mod render;
pub use render::*;

use std::{fs, path::{Path, PathBuf}};

use l2_sim::runner::{self as battle, BattleRunner};
use l2_sim::terrain;
use l2_sim::{Troop, SIDE_A, SIDE_B};
use l2_view::campaign;
use l2_view::canvas::Canvas;
use l2_view::chrome;
use l2_view::figures::{self, Anim, Colour};
use l2_view::scene::{self, BattleAssets, Camera};
use l2_view::sheet::Sheet;

fn asset_dir() -> Option<PathBuf> {
    l2_testkit::install_dir()
}

/// The install is inconsistent about casing, so every lookup is
/// case-insensitive — the same rule the mod overlay applies.
fn find(dir: &Path, name: &str) -> Option<PathBuf> {
    fs::read_dir(dir).ok()?.filter_map(|e| e.ok()).map(|e| e.path()).find(|p| {
        p.file_name()
            .and_then(|f| f.to_str())
            .is_some_and(|f| f.eq_ignore_ascii_case(name))
    })
}

fn read(dir: &Path, name: &str) -> Option<Vec<u8>> {
    fs::read(find(dir, name)?).ok()
}

/// The six troop types drawn from a per-facing stride. Knights are checked
/// separately because their frames come from a table, not a stride.
const STRIDE_TROOPS: [Troop; 6] = [
    Troop::Peasants,
    Troop::Crossbowmen,
    Troop::Macemen,
    Troop::Swordsmen,
    Troop::Pikemen,
    Troop::Archers,
];

