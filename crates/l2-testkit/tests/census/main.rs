//! **The census of install-gated tests**, so that a silent skip stops being
//! the failure mode.
//!
//! # The problem
//!
//! `cargo test --workspace` on this machine and `cargo test --workspace` with
//! `LORDS2_DIR` and `LORDS2_FIXTURES` pointing nowhere — which is what CI does —
//! print **the same** `N passed; 0 failed`, whatever `N` is that week. The two
//! runs assert wildly different amounts and are indistinguishable from their
//! output, because a gated test that finds no game prints a line to stderr and
//! returns green.
//!
//! The gap is [`GATED_TOTAL`] tests, which this file names.
//!
//! Nobody notices a test that stops existing. The reproduction against a real
//! save, the renderer against real sprites, the scenario against the England
//! fixture — the project's strongest evidence — did not run on CI at all, and
//! nothing said so.
//!
//! # The mechanism
//!
//! Every gate in the workspace goes through a macro in `l2-testkit`. This test
//! reads the source of every test file, counts the gated test functions per
//! file and per gate, and asserts the result against [`INVENTORY`] below. It
//! runs everywhere, needs no game, and cannot itself be skipped.
//!
//! So: **add a gated test tomorrow and this goes red** until the inventory is
//! updated, which is a one-line diff that makes the new gate visible in the
//! history. Remove a gate and it goes red the same way. The number is small
//! enough to argue with, which is the point — 91 of 946 test functions were
//! install-gated before anybody counted, and 42 of them resolved the install
//! through a hard-coded path copied between files.
//!
//! It also prints, on every run, how many of those gates the current
//! environment satisfies. A run that asserted a third of what it looks like it
//! asserted now says so.

mod scanner;
pub use scanner::*;
mod assertions;
pub use assertions::*;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Which gate a test sits behind, in the order a test body is searched. The
/// order is the precedence: a test that takes both the fixture and the install
/// is counted against the fixture, because that is the stronger requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Gate {
    /// `l2_testkit::england!()` — needs the England turn-one fixture.
    England,
    /// `l2_testkit::fixture!(..)` — needs one named save from `LORDS2_FIXTURES`.
    Fixture,
    /// `l2_testkit::saves!()` — needs at least one save from anywhere.
    Saves,
    /// `l2_testkit::executable!()` — needs `Lords2.exe`.
    Executable,
    /// `l2_testkit::install!()`, or a local helper built on
    /// `l2_testkit::install_dir()`.
    Install,
    /// A `l2_testkit::skip!` on some other condition — a file missing from an
    /// install that is otherwise present, say. Counted, because a skip is a
    /// skip.
    Other,
}

impl Gate {
    fn name(self) -> &'static str {
        match self {
            Gate::England => "england",
            Gate::Fixture => "fixture",
            Gate::Saves => "saves",
            Gate::Executable => "executable",
            Gate::Install => "install",
            Gate::Other => "other",
        }
    }

    /// Is this gate satisfied in the environment the test process is running
    /// in? `Other` cannot be decided from outside the test, so it is `None`.
    fn satisfied(self) -> Option<bool> {
        match self {
            Gate::England => {
                Some(matches!(l2_testkit::england_turn1(), l2_testkit::FixtureState::Ready(_)))
            }
            Gate::Fixture => Some(l2_testkit::fixtures_dir().is_some()),
            Gate::Saves => Some(!l2_testkit::every_available_save().is_empty()),
            Gate::Executable => Some(l2_testkit::executable().is_some()),
            Gate::Install => Some(l2_testkit::install_dir().is_some()),
            Gate::Other => None,
        }
    }
}

/// **The inventory. Update it deliberately.**
///
/// `(file, gate, number of gated `#[test]` functions)`, sorted. A line here is
/// a statement that this many tests in this file do not run without that
/// input.
const INVENTORY: &[(&str, &str, usize)] = &[
    ("crates/l2-formats/tests/battle_fixtures.rs", "fixture", 3),
    ("crates/l2-formats/tests/corpus.rs", "install", 5),
    ("crates/l2-formats/tests/maps.rs", "install", 6),
    ("crates/l2-formats/tests/save.rs", "executable", 1),
    ("crates/l2-formats/tests/save.rs", "saves", 19),
    ("crates/l2-formats/tests/save_england_turn1.rs", "england", 13),
    ("crates/l2-game/src/audio/mod.rs", "install", 1),
    ("crates/l2-game/src/screens/menubar.rs", "install", 2),
    ("crates/l2-game/tests/ai_war.rs", "england", 2),
    ("crates/l2-game/tests/armoury.rs", "england", 13),
    ("crates/l2-game/tests/arms.rs", "executable", 1),
    ("crates/l2-game/tests/arrival.rs", "install", 1),
    ("crates/l2-game/tests/audio_battle.rs", "executable", 1),
    ("crates/l2-game/tests/audio_battle.rs", "install", 5),
    ("crates/l2-game/tests/audio_install.rs", "england", 1),
    ("crates/l2-game/tests/audio_install.rs", "executable", 1),
    ("crates/l2-game/tests/audio_install.rs", "install", 8),
    ("crates/l2-game/tests/audio_screens.rs", "install", 9),
    ("crates/l2-game/tests/audio_wiring.rs", "install", 17),
    ("crates/l2-game/tests/battle_picture.rs", "install", 16),
    ("crates/l2-game/tests/cattle_shortage.rs", "england", 2),
    ("crates/l2-game/tests/chrome_text.rs", "england", 17),
    ("crates/l2-game/tests/cursor.rs", "england", 3),
    ("crates/l2-game/tests/differential.rs", "fixture", 3),
    ("crates/l2-game/tests/ground.rs", "install", 7),
    ("crates/l2-game/tests/industry.rs", "england", 5),
    ("crates/l2-game/tests/job_bodies.rs", "england", 10),
    ("crates/l2-game/tests/job_bodies.rs", "executable", 1),
    ("crates/l2-game/tests/job_bodies.rs", "fixture", 5),
    ("crates/l2-game/tests/labour_move.rs", "england", 5),
    ("crates/l2-game/tests/labour_move.rs", "fixture", 1),
    ("crates/l2-game/tests/long_game.rs", "england", 4),
    ("crates/l2-game/tests/long_game.rs", "install", 1),
    ("crates/l2-game/tests/long_game.rs", "other", 1),
    ("crates/l2-game/tests/merchant.rs", "england", 7),
    ("crates/l2-game/tests/messages.rs", "install", 5),
    ("crates/l2-game/tests/military.rs", "fixture", 1),
    ("crates/l2-game/tests/minimap.rs", "england", 6),
    ("crates/l2-game/tests/movies.rs", "install", 11),
    ("crates/l2-game/tests/newgame.rs", "install", 3),
    ("crates/l2-game/tests/newgame.rs", "other", 11),
    ("crates/l2-game/tests/overlay_palette.rs", "install", 2),
    ("crates/l2-game/tests/pacing.rs", "england", 5),
    ("crates/l2-game/tests/press.rs", "executable", 1),
    ("crates/l2-game/tests/right_column.rs", "executable", 1),
    ("crates/l2-game/tests/save.rs", "england", 1),
    ("crates/l2-game/tests/save.rs", "install", 1),
    ("crates/l2-game/tests/scenario.rs", "england", 13),
    ("crates/l2-game/tests/scenario.rs", "fixture", 1),
    ("crates/l2-game/tests/screens_battle.rs", "england", 2),
    ("crates/l2-game/tests/screens_county.rs", "england", 37),
    ("crates/l2-game/tests/screens_diplomacy.rs", "england", 1),
    ("crates/l2-game/tests/screens_info.rs", "england", 12),
    ("crates/l2-game/tests/screens_info.rs", "executable", 1),
    ("crates/l2-game/tests/screens_map.rs", "england", 30),
    ("crates/l2-game/tests/screens_menubar.rs", "england", 7),
    ("crates/l2-game/tests/screens_menubar.rs", "executable", 1),
    ("crates/l2-game/tests/screens_shoot.rs", "england", 1),
    ("crates/l2-game/tests/screens_village.rs", "england", 13),
    ("crates/l2-game/tests/screens_village.rs", "install", 2),
    ("crates/l2-game/tests/seam.rs", "fixture", 4),
    ("crates/l2-game/tests/setup.rs", "england", 15),
    ("crates/l2-game/tests/setup.rs", "install", 1),
    ("crates/l2-game/tests/shell.rs", "executable", 1),
    ("crates/l2-game/tests/shell.rs", "install", 7),
    ("crates/l2-game/tests/siege_picture.rs", "executable", 1),
    ("crates/l2-game/tests/siege_picture.rs", "install", 5),
    ("crates/l2-game/tests/standings.rs", "executable", 1),
    ("crates/l2-game/tests/standings.rs", "install", 2),
    ("crates/l2-game/tests/text.rs", "install", 3),
    ("crates/l2-game/tests/tips.rs", "install", 4),
    ("crates/l2-game/tests/tooltips.rs", "executable", 1),
    ("crates/l2-game/tests/tooltips.rs", "install", 1),
    ("crates/l2-game/tests/wheat.rs", "england", 1),
    ("crates/l2-kingdom/tests/cattle.rs", "saves", 3),
    ("crates/l2-kingdom/tests/defence.rs", "fixture", 2),
    ("crates/l2-kingdom/tests/fields.rs", "england", 11),
    ("crates/l2-kingdom/tests/industry_forecast.rs", "england", 4),
    ("crates/l2-kingdom/tests/oracle.rs", "executable", 6),
    ("crates/l2-kingdom/tests/reproduction.rs", "england", 25),
    ("crates/l2-kingdom/tests/siege.rs", "fixture", 5),
    ("crates/l2-kingdom/tests/weapon_choice.rs", "fixture", 1),
    ("crates/l2-mods/tests/corpus.rs", "install", 6),
    ("crates/l2-scenario/tests/explored.rs", "england", 2),
    ("crates/l2-scenario/tests/explored.rs", "fixture", 1),
    ("crates/l2-scenario/tests/explored.rs", "install", 1),
    ("crates/l2-scenario/tests/import.rs", "england", 10),
    ("crates/l2-scenario/tests/import.rs", "fixture", 1),
    ("crates/l2-scenario/tests/import.rs", "saves", 28),
    ("crates/l2-scenario/tests/newgame.rs", "england", 2),
    ("crates/l2-scenario/tests/newgame.rs", "install", 4),
    ("crates/l2-scenario/tests/stored_fields.rs", "other", 1),
    ("crates/l2-scenario/tests/stored_fields.rs", "saves", 1),
    ("crates/l2-sim/tests/castle_layout.rs", "install", 12),
    ("crates/l2-sim/tests/oracle.rs", "executable", 4),
    ("crates/l2-smk/tests/corpus.rs", "install", 5),
    ("crates/l2-view/tests/install.rs", "executable", 3),
    ("crates/l2-view/tests/install.rs", "fixture", 1),
    ("crates/l2-view/tests/install.rs", "install", 29),
];

/// The needles that name a gate, strongest first. A body containing several is
/// counted against the first that matches.
const NEEDLES: &[(&str, Gate)] = &[
    ("england!(", Gate::England),
    ("england_turn1(", Gate::England),
    ("fixture!(", Gate::Fixture),
    ("fixture_save(", Gate::Fixture),
    ("fixtures_dir(", Gate::Fixture),
    ("saves!(", Gate::Saves),
    ("every_available_save(", Gate::Saves),
    ("executable!(", Gate::Executable),
    ("l2_testkit::executable(", Gate::Executable),
    ("install!(", Gate::Install),
    ("install_dir(", Gate::Install),
    ("read_install(", Gate::Install),
    ("l2_testkit::skip!(", Gate::Other),
];

/// The census itself.
#[test]
fn the_install_gated_tests_are_the_ones_we_have_written_down() {
    let found = scan();
    let expected: BTreeMap<(String, &'static str), usize> =
        INVENTORY.iter().map(|&(f, g, n)| ((f.to_string(), g), n)).collect();

    if found != expected {
        let mut lines = String::new();
        for ((file, gate), n) in &found {
            lines.push_str(&format!("    (\"{file}\", \"{gate}\", {n}),\n"));
        }
        panic!(
            "the gate census has moved.\n\n\
             A gated test does not run on CI, and nothing else in the suite says so - which is \
             why this number is written down. If the change is intended, replace INVENTORY in \
             crates/l2-testkit/tests/census.rs with:\n\n\
             const INVENTORY: &[(&str, &str, usize)] = &[\n{lines}];\n\n\
             expected {} entries, found {}",
            expected.len(),
            found.len()
        );
    }
    assert_eq!(
        found.values().sum::<usize>(),
        GATED_TOTAL,
        "{GATED_TOTAL} test functions in this workspace do not exist without a copy of the game"
    );
    eprintln!("gate census: {GATED_TOTAL} install-gated tests across {} files", INVENTORY.len());
}

