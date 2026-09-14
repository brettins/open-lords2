//! The whole path, end to end: a `.toml` in a mod directory changes what
//! happens when two figures fight.
//!
//! Everything else in this crate's tests checks a stage — the document parses,
//! the merge resolves, the table loads. This checks that the stages are
//! joined up,
//! believe without evidence.
//!
//! `docs/decisions.md` C11. These numbers
//! live in `Lords2.exe` as instructions.
//! game that reaches them. A test that starts at a text file and ends at a
//! different casualty count is the demonstration that the situation has
//! changed.

mod combat;
pub use combat::*;
mod economy;
pub use economy::*;

use l2_kingdom::tables::{health_band, Tables};
use l2_kingdom::{Kingdom, Options};
use l2_kingdom::tables::{Commodity, JOB_COUNT};

#[path = "../common/mod.rs"]
mod common;

use common::TempDir;
use l2_mods::Platform;
use l2_sim::{Battle, Troop, TroopTable, SIDE_A, SIDE_B};

