//! Validates the platform against a real game install.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-mods -- --nocapture
//! ```
//!
//! Skips. No assets live
//! in this repository and none are written by these tests: the install is
//! mounted as a read-only layer and every file this test creates goes in a
//! temporary directory.


#[path = "../common/mod.rs"]
mod common;

use common::TempDir;
use l2_mods::seed::{battle_names, parse_troops_eng, to_rules_toml, ROWS};
use l2_mods::{Platform, Ruleset, Side, TroopRules, Vfs};
use std::env;

fn install() -> Option<String> {
    l2_testkit::install_dir().map(|d| d.display().to_string())
}

macro_rules! skip_without_install {
    () => {
        match install() {
            Some(d) => d,
            None => {
                l2_testkit::skip!("LORDS2_DIR not set or not a directory - skipping corpus test");
            }
        }
    };
}

mod indexing;
pub use indexing::*;
mod seeding;
pub use seeding::*;
mod difficulty;
pub use difficulty::*;


