#![allow(unused_imports)]
use super::*;
use super::merge_tests::*;
use super::driver_helpers::*;
use super::*;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Every file `.gitattributes` hands to the driver is in [`KEYED`], and every
/// file in [`KEYED`] is handed to the driver.
///
/// The two lists are maintained by different work — one by whoever adds a file
/// to the driver, one by whoever adds a test — so they must agree, which is the
/// only shape of check that has ever caught anything here. Adding
/// `docs/arms.json` to `.gitattributes` and forgetting it here would have left
/// the file this test exists for outside the test.
#[test]
fn the_attributes_file_and_this_test_name_the_same_keyed_files() {
    let text = std::fs::read_to_string(root().join(".gitattributes")).expect(".gitattributes");
    let mut declared: Vec<String> = text
        .lines()
        .filter(|l| l.contains("merge=l2json") && !l.trim_start().starts_with('#'))
        .filter_map(|l| l.split_whitespace().next())
        .map(|s| s.to_string())
        .collect();
    declared.sort();
    let mut expected: Vec<String> = KEYED.iter().map(|s| s.to_string()).collect();
    expected.sort();
    assert_eq!(
        declared, expected,
        "The .gitattributes file and KEYED disagree about which files merge by key. \
         A file in one and not the other is either merged by a driver nothing \
         checks, or checked by a test no merge will ever use."
    );
}

/// **Every keyed array is in its own key's order**, asked of the driver.
///
/// This test used to scan for `"addr"` lines itself and require them to
/// ascend. That was right for `symbols.json` and **wrong for `arms.json`**,
/// which carries both `id` and `addr` and is keyed by `id` because one address
/// holds several arms — so sorting it correctly, by `id`, made this test fail.
///
/// The rule about what a file's key IS now lives in exactly one place,
/// `KEY_FIELDS` in `merge-json.js`
/// check ask it. A Rust copy of that rule was a second list that could disagree
/// with the first, which is the failure this whole area is about — and it did
/// disagree, within a day of being written.
///
/// Why order matters at all: two branches that both keep a file in key order
/// cannot produce a misaligned diff, whatever git does and whether or not the
/// driver is registered. That is the half of the fix that removes the failure
///

/// **Every keyed array's key is
///
/// This is the invariant a keyed merge silently depends on: if the key the
/// driver picks is not unique, merging *deletes* one entry per collision, and
/// the result parses and reads plausibly. `arms.json` is exactly that trap —
/// `addr` looks like the key and is not.
///
/// It shells out to `merge-json.js --check`
/// `KEY_FIELDS` here **on purpose**. A Rust copy of the key rule would be a
/// second list that can drift from the first, which is the failure this whole
/// area is about; the driver's own logic is what a merge will use, so the
/// driver's own logic is what has to be asked.
#[test]
fn every_keyed_arrays_key_is_unique() {
    let root = root();
    let mut cmd = Command::new("node");
    cmd.arg("tools/symbols/merge-json.js").arg("--check");
    for rel in KEYED {
        cmd.arg(rel);
    }
    let Ok(out) = cmd.current_dir(&root).output() else {
        return; // no node on this machine; the CI job has one
    };
    assert!(
        out.status.success(),
        "a keyed array has a non-unique key, so merging it would delete entries:\n{}",
        String::from_utf8_lossy(&out.stderr),
    );
}

