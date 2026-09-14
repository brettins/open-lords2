#![allow(unused_imports)]
use super::*;
use super::view::*;
use super::*;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// **The real ledger passes its schema, and the skipped half says it skipped.**
///
/// The second assertion is not decoration. A check that silently does half its
/// job reads exactly like one that did all of it, so the git half's absence has
/// to be *printed*, the way the census prints what it cannot run.
#[test]
fn the_ledger_passes_its_schema_and_says_it_did_not_compare_git() {
    let Some(out) = work(&["--check", "--schema"]) else {
        return; // no node on this machine; the CI job has one
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "docs/work.json fails its schema. The lead owns the rows, so this is a \
         message for the lead, not a test to adjust:\n{stderr}"
    );
    assert!(
        stdout.contains("SKIP git agreement"),
        "the schema half ran without saying that the git half did not — a skip \
         that prints nothing is a pass that means nothing:\n{stdout}"
    );
    assert!(
        stdout.contains("work: features: schema:") && stdout.contains("SKIP feature references and evidence"),
        "docs/features.json's schema was not checked, or its skipped halves were \
         not said to be skipped:\n{stdout}"
    );
}

