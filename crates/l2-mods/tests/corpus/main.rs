

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


