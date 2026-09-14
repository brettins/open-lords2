//! Where the test suite's inputs live, and what it does when they are missing.
//!
//! # The problem this crate exists to fix
//!
//! Ninety-one test functions in this workspace need a copy of the game. Each of
//! them found it for itself, with a copy of
//!
//! ```text
//! std::env::var("LORDS2_DIR").ok()...or_else(|| r"F:\games\Lords of the Realm II")
//! ```
//!
//! and each of them, when the copy was missing, printed a line to stderr and
//! returned green. Two consequences followed, and both had already bitten:
//!
//! * **CI runs no install, so those tests do not exist there.** They pass
//! locally, they pass on CI, and the passing means two different things. The
//!   project's strongest evidence — the reproduction against a real save, the
//!   renderer against real sprites — was invisible in the place that gates
//!   merges.
//! * **`lastturn.sav` is not a fixture.** It is the game's rolling autosave,
//!   rewritten every turn a human plays. Nine tests asserted one particular
//!   game's numbers against it and stayed green for months because nobody had
//!   played; the first ten minutes of play turned them all red at once. A
//!   *path* was being treated as an *identity*.
//!
//! So this crate does three things: it puts the search for an install in one
//! place, it makes a fixture something you name and verify
//! you happen to open, and it gives every gate a shape that
//! [`tests/census.rs`](../../tests/census.rs) can count.
//!
//! # Two directories, two variables
//!
//! | variable | default | holds |
//! |---|---|---|
//! | `LORDS2_DIR` | `F:\games\Lords of the Realm II` | the read-only game install: `Lords2.exe`, the `.pl8` art, `USER.SKR` |
//! | `LORDS2_FIXTURES` | `E:\dev\lords2-fixtures` | preserved saves, **outside** the install, which nothing rewrites |
//!
//! The install is read-only (`CLAUDE.md` rule 2) and its saves are volatile.
//! Fixtures live outside it precisely so that playing the game cannot destroy
//! them. Neither directory is ever committed: `.gitignore` refuses `*.sav`, and
//! `tests/census.rs` asserts that no `.sav` has crept into the tree.
//!
//! # Named fixtures
//!
//! A fixture is a *name*, a *file*, and a *fingerprint*. [`england_turn1`]
//! returns [`FixtureState::WrongGame`]
//! expected path is some other game, so the three states stay distinct:
//!
//! * **ready** — the file is there and it is the right game;
//! * **absent** — nothing is configured, and the test skips, visibly;
//! * **wrong game** — something is there and it is not what the test describes,
//! and the test *fails*, with a message saying so.
//!
//! Conflating the last two is how this went unnoticed: nine tests failing with
//! bare assertion diffs read as "our reader broke", when the truth was "this is
//! a different saved game".

mod fixtures;
pub use fixtures::*;

use std::path::{Path, PathBuf};

pub use l2_formats::save::{Save, SaveError};

pub mod pe;

/// Environment variable naming the game install.
pub const INSTALL_VAR: &str = "LORDS2_DIR";

/// Environment variable naming the fixture directory.
pub const FIXTURES_VAR: &str = "LORDS2_FIXTURES";

/// Where the install is on the machine this project was developed on. A
/// default, not a requirement — `LORDS2_DIR` overrides it and CI sets neither.
///
/// It is spelled out in exactly one place so that the census can assert it is
/// spelled out in exactly one place.
pub const DEFAULT_INSTALL: &str = r"F:\games\Lords of the Realm II";

/// Likewise for the preserved saves. Deliberately **not** inside the install:
/// the install is read-only and its own saves are overwritten by play.
pub const DEFAULT_FIXTURES: &str = r"E:\dev\lords2-fixtures";

/// Environment variable naming the older DOS install, which a couple of tests
/// diff the Windows files against.
pub const DOS_INSTALL_VAR: &str = "LORDS2_DOS_DIR";

/// Where the DOS install is on this machine. Also read-only.
pub const DEFAULT_DOS_INSTALL: &str = r"F:\games\LORDS2";

/// The game install, or `None`.
pub fn install_dir() -> Option<PathBuf> {
    dir(INSTALL_VAR, DEFAULT_INSTALL)
}

/// The fixture directory, or `None`.
pub fn fixtures_dir() -> Option<PathBuf> {
    dir(FIXTURES_VAR, DEFAULT_FIXTURES)
}

/// The older DOS install, or `None`.
pub fn dos_install_dir() -> Option<PathBuf> {
    dir(DOS_INSTALL_VAR, DEFAULT_DOS_INSTALL)
}

fn dir(var: &str, default: &str) -> Option<PathBuf> {
    match std::env::var(var) {
        Ok(v) if Path::new(&v).is_dir() => Some(PathBuf::from(v)),
        // An explicitly set variable pointing nowhere is a mistake worth
        // hearing about,
        // machine.
        Ok(v) if !v.is_empty() => {
            eprintln!("{var} is set to {v:?}, which is not a directory - ignoring it");
            None
        }
        _ => {
            let p = PathBuf::from(default);
            p.is_dir().then_some(p)
        }
    }
}

/// Where a save was found. Only [`Origin::Fixture`] is stable across a session
/// of play.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// `LORDS2_FIXTURES` — preserved, outside the install, nothing rewrites it.
    Fixture,
    /// Inside the game install. **Volatile.** `lastturn.sav`, `safeturn.sav`
    /// and `old_turn.sav` are rewritten every turn a human plays, and
    /// `incombat.sav` and `endedcom.sav` every battle. Fine to assert
    /// *invariants* over; never a fixture.
    Install,
}

/// One save file that is present, already opened.
pub struct SaveFile {
    /// The file name, lowercased, e.g. `battle-before.sav`.
    pub name: String,
    pub path: PathBuf,
    pub origin: Origin,
    pub save: Save,
}

impl SaveFile {
    /// How a failure message should name it.
    pub fn label(&self) -> String {
        match self.origin {
            Origin::Fixture => format!("fixture {}", self.name),
            Origin::Install => format!("install {} (volatile)", self.name),
        }
    }
}

/// What [`england_turn1`] found.
pub enum FixtureState {
    /// The file is there and it is the game the tests describe.
    Ready(Box<Save>),
    /// Nothing is configured. The test should skip, visibly.
    Absent(String),
    /// Something is configured and it is a different saved game. The test
    /// should **fail**, saying so.
    WrongGame(String),
}

/// The name cargo gave the running test, for a skip line that says which test
/// went quiet.
pub fn current_test() -> String {
    std::thread::current().name().unwrap_or("<test>").to_string()
}

/// Skip the current test, loudly enough to be found in a log.
///
/// Every gate in the workspace goes through one of the macros below, and
/// `l2-testkit/tests/census.rs` counts them. That is the whole mechanism: a
/// gate that is not counted is a gate that does not compile past the census.
#[macro_export]
macro_rules! skip {
    ($($arg:tt)*) => {{
        eprintln!("SKIP {}: {}", $crate::current_test(), format!($($arg)*));
        return;
    }};
}

/// The game install directory, or skip.
#[macro_export]
macro_rules! install {
    () => {
        match $crate::install_dir() {
            Some(d) => d,
            None => $crate::skip!(
                "no game install ({} unset and the default is absent)",
                $crate::INSTALL_VAR
            ),
        }
    };
}

/// `Lords2.exe`'s bytes, or skip. The oracle tests' gate.
#[macro_export]
macro_rules! executable {
    () => {
        match $crate::executable() {
            Some(e) => e,
            None => $crate::skip!("no Lords2.exe to read ({} unset)", $crate::INSTALL_VAR),
        }
    };
}

/// Every save this machine can offer, or skip when there are none. The gate an
/// **invariant** sits behind: it does not care which game it is handed.
#[macro_export]
macro_rules! saves {
    () => {{
        let found = $crate::every_available_save();
        if found.is_empty() {
            $crate::skip!(
                "no save files reachable (neither {} nor {} yields one)",
                $crate::INSTALL_VAR,
                $crate::FIXTURES_VAR
            );
        }
        found
    }};
}

/// The England turn-one fixture, or skip when it is absent — and **panic** when
/// something is there that is not it.
#[macro_export]
macro_rules! england {
    () => {
        match $crate::england_turn1() {
            $crate::FixtureState::Ready(s) => *s,
            $crate::FixtureState::Absent(why) => $crate::skip!("{why}"),
            $crate::FixtureState::WrongGame(why) => panic!("{why}"),
        }
    };
}

/// A named fixture from `LORDS2_FIXTURES`, or skip. Used by the army oracle,
/// whose three saves are one battle caught at three moments.
#[macro_export]
macro_rules! fixture {
    ($name:expr) => {
        match $crate::fixture_save($name) {
            Ok(s) => s,
            Err(why) => $crate::skip!("{why}"),
        }
    };
}

