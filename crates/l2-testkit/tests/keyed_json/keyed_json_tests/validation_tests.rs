#![allow(unused_imports)]
use super::*;
use super::merge_tests::*;
use super::driver_helpers::*;
use super::*;
use std::path::{Path, PathBuf};
use std::process::Command;

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

