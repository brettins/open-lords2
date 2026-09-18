
mod fixtures;
pub use fixtures::*;

use std::path::{Path, PathBuf};

pub use l2_formats::save::{Save, SaveError};

pub mod pe;
pub mod worlds;

pub const INSTALL_VAR: &str = "LORDS2_DIR";

pub const FIXTURES_VAR: &str = "LORDS2_FIXTURES";

pub const DEFAULT_INSTALL: &str = r"F:\games\Lords of the Realm II";

pub const DEFAULT_FIXTURES: &str = r"E:\dev\lords2-fixtures";

pub const DOS_INSTALL_VAR: &str = "LORDS2_DOS_DIR";

pub const DEFAULT_DOS_INSTALL: &str = r"F:\games\LORDS2";

pub fn install_dir() -> Option<PathBuf> {
    dir(INSTALL_VAR, DEFAULT_INSTALL)
}

pub fn fixtures_dir() -> Option<PathBuf> {
    dir(FIXTURES_VAR, DEFAULT_FIXTURES)
}

pub fn dos_install_dir() -> Option<PathBuf> {
    dir(DOS_INSTALL_VAR, DEFAULT_DOS_INSTALL)
}

fn dir(var: &str, default: &str) -> Option<PathBuf> {
    match std::env::var(var) {
        Ok(v) if Path::new(&v).is_dir() => Some(PathBuf::from(v)),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Fixture,
    Install,
}

pub struct SaveFile {
    pub name: String,
    pub path: PathBuf,
    pub origin: Origin,
    pub save: Save,
}

impl SaveFile {
    pub fn label(&self) -> String {
        match self.origin {
            Origin::Fixture => format!("fixture {}", self.name),
            Origin::Install => format!("install {} (volatile)", self.name),
        }
    }
}

pub enum FixtureState {
    Ready(Box<Save>),
    Absent(String),
    WrongGame(String),
}

pub fn current_test() -> String {
    std::thread::current().name().unwrap_or("<test>").to_string()
}

#[macro_export]
macro_rules! skip {
    ($($arg:tt)*) => {{
        eprintln!("SKIP {}: {}", $crate::current_test(), format!($($arg)*));
        return;
    }};
}

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

#[macro_export]
macro_rules! executable {
    () => {
        match $crate::executable() {
            Some(e) => e,
            None => $crate::skip!("no Lords2.exe to read ({} unset)", $crate::INSTALL_VAR),
        }
    };
}

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

#[macro_export]
macro_rules! fixture {
    ($name:expr) => {
        match $crate::fixture_save($name) {
            Ok(s) => s,
            Err(why) => $crate::skip!("{why}"),
        }
    };
}

