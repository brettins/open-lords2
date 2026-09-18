//! `docs/decisions.md` C11. These numbers
//! live in `Lords2.exe` as instructions.

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

