#![allow(unused_imports)]
use super::*;
use super::layout_tests::*;
use super::accessors_part::*;
use super::value_tests::*;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use l2_formats::save::{Save, COUNTY_BASE, COUNTY_STRIDE, REALM_BASE, REALM_STRIDE};
use l2_kingdom::Kingdom;
use l2_scenario::Scenario;

#[test]
fn every_offset_the_decompilation_touches_has_a_row() {
    let root = repo_root();
    let corpus = std::env::var_os("LORDS2_DECOMP")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("tools/oracle/decomp"));
    if !corpus.is_dir() {
        l2_testkit::skip!(
            "no decompiled corpus at {} (LORDS2_DECOMP overrides; tools/oracle/decompile-all.ps1 makes one)",
            corpus.display()
        );
    }
    let out = match std::process::Command::new("node")
        .arg("tools/oracle/fields.js")
        .arg("--check")
        .env("LORDS2_DECOMP", &corpus)
        .current_dir(&root)
        .output()
    {
        Ok(out) => out,
        Err(e) => l2_testkit::skip!("node is not runnable here: {e}"),
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "node tools/oracle/fields.js --check failed:\n{stdout}{stderr}");
    eprintln!("{stdout}");
}

