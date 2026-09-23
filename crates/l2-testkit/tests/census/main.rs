//! So: **add a gated test tomorrow and this goes red** until `gated.txt` is
//! rewritten (`L2_CENSUS_BLESS=1`), a one-line diff that makes the new gate
//! visible in the history and merges by union. Remove a gate and it goes red the same way. The number is small
//! enough to argue with, which is the point — 91 of 946 test functions were
//! install-gated before anybody counted, and 42 of them resolved the install
//! through a hard-coded path copied between files.

mod scanner;
use scanner::*;
mod assertions;

use std::collections::BTreeMap;

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

const RECORD: &str = "crates/l2-testkit/tests/census/gated.txt";

type Found = BTreeMap<(String, &'static str), usize>;

fn render(found: &Found) -> String {
    found.iter().map(|((file, gate), n)| format!("{file} {gate} {n}\n")).collect()
}

/// A `merge=union` merge of two edits to one line leaves both: a duplicate key, refused.
fn verdict(record: &str, found: &Found) -> Result<usize, String> {
    let lines: Vec<&str> = record.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let mut seen = std::collections::BTreeSet::new();
    for line in &lines {
        let mut it = line.split_whitespace();
        let key = (it.next().unwrap_or(""), it.next().unwrap_or(""));
        if !seen.insert(key) {
            return Err(format!("{} {} is recorded twice", key.0, key.1));
        }
    }
    let have: String = lines.iter().map(|l| format!("{l}\n")).collect();
    if have != render(found) {
        return Err(format!("the record has {} entries, the tree {}", lines.len(), found.len()));
    }
    Ok(found.values().sum())
}

#[test]
fn the_install_gated_tests_are_the_ones_we_have_written_down() {
    let found = scan();
    let path = repo_root().join(RECORD);
    let record = std::fs::read_to_string(&path).unwrap_or_default();
    match verdict(&record, &found) {
        Ok(total) => eprintln!("gate census: {total} install-gated tests, {} entries", found.len()),
        Err(why) if std::env::var_os("L2_CENSUS_BLESS").is_some() => {
            std::fs::write(&path, render(&found)).expect("write the census record");
            eprintln!("gate census rewritten: {why}");
        }
        Err(why) => panic!(
            "the gate census has moved: {why}.\n\n\
             A gated test does not run on CI, and nothing else in the suite says so. If the \
             change is intended, rerun with L2_CENSUS_BLESS=1 to rewrite {RECORD}:\n\n{}",
            render(&found)
        ),
    }
}

#[test]
fn a_union_merge_that_doubled_a_line_is_refused() {
    let found: Found = [(("crates/a.rs".to_string(), "install"), 2)].into_iter().collect();
    assert_eq!(verdict("crates/a.rs install 2\n", &found), Ok(2));
    assert!(verdict("crates/a.rs install 1\ncrates/a.rs install 2\n", &found).is_err());
    assert!(verdict("crates/a.rs install 3\n", &found).is_err());
    assert!(verdict("", &found).is_err());
}

#[test]
fn the_record_merges_by_union() {
    let attrs = std::fs::read_to_string(repo_root().join(".gitattributes")).unwrap();
    assert!(attrs.lines().any(|l| l.trim() == format!("{RECORD} merge=union")));
}

