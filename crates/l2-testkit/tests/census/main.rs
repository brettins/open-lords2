//! So: **add a gated test tomorrow and this goes red** until the inventory is
//! updated, which is a one-line diff that makes the new gate visible in the
//! history. Remove a gate and it goes red the same way. The number is small
//! enough to argue with, which is the point — 91 of 946 test functions were
//! install-gated before anybody counted, and 42 of them resolved the install
//! through a hard-coded path copied between files.

mod scanner;
pub use scanner::*;
mod assertions;
pub use assertions::*;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Gate {
    England,
    Fixture,
    Saves,
    Executable,
    Install,
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



pub(crate) const GATED_TOTAL: usize = 0;

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
             crates/l2-testkit/tests/census/main.rs with:\n\n\
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

