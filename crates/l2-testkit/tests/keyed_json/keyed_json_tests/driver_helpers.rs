#![allow(unused_imports)]
use super::*;
use super::validation_tests::*;
use super::merge_tests::*;
use super::*;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A one-row-per-line file as (the lines before the first row, the rows without
/// their trailing commas, the lines after the last row).
pub(super) fn split_rows(text: &str) -> (Vec<String>, Vec<String>, Vec<String>) {
    let lines: Vec<&str> = text.lines().collect();
    let is_row = |l: &str| l.trim_start().starts_with("{\"id\": ");
    let first = lines.iter().position(|l| is_row(l)).expect("a one-line row");
    let last = lines.iter().rposition(|l| is_row(l)).expect("a one-line row");
    let own = |s: &[&str]| s.iter().map(|l| l.to_string()).collect::<Vec<_>>();
    let rows = lines[first..=last].iter().map(|l| l.trim_end_matches(',').to_string()).collect();
    (own(&lines[..first]), rows, own(&lines[last + 1..]))
}

pub(super) fn join_rows(head: &[String], rows: &[String], tail: &[String]) -> String {
    format!("{}\n{}\n{}\n", head.join("\n"), rows.join(",\n"), tail.join("\n"))
}

/// Runs the driver on three texts under a registered path, returning whether it
/// reported success, what it left in the `ours` file, and what it said.
pub(super) fn drive(label: &str, base: &str, ours: &str, theirs: &str) -> Option<(bool, String, String)> {
    let dir = std::env::temp_dir().join(format!(
        "l2-drive-{}-{}",
        std::process::id(),
        label.replace(['/', '.'], "-")
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let (b, o, t) = (dir.join("base.json"), dir.join("ours.json"), dir.join("theirs.json"));
    std::fs::write(&b, base).unwrap();
    std::fs::write(&o, ours).unwrap();
    std::fs::write(&t, theirs).unwrap();
    let out = Command::new("node")
        .arg("tools/symbols/merge-json.js")
        .args([&b, &o, &t])
        .arg(label)
        .current_dir(root())
        .output();
    let left = std::fs::read_to_string(&o).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    let out = out.ok()?;
    Some((out.status.success(), left, String::from_utf8_lossy(&out.stderr).to_string()))
}

