//! **`docs/arms.json` against the tree, in both directions.**
//!
//! `CLAUDE.md` rule 5 says a feature must name the original function it
//! reproduces. `docs/decisions.md` C61 says stated rules do not hold on this
//! project and checked ones do — the numbering protocol was written after four
//! collisions and did not prevent the fifth. So this is the check, and it is the
//! only reason the file's shape is what it is.
//!
//! # The check
//!
//! The set of `id`s in `docs/arms.json` with status `reproduced` is **equal** to
//! the set of `// arm: <id>` markers in `crates/`. Both differences are named
//! when it fails, because they mean different things:
//!
//! * **a record with no marker** is a claim nobody kept — the file says we do
//!   something and no line of code says it does it;
//! * **a marker with no record** is an arm nobody wrote down, which is the exact
//!   failure C61 measured: behaviour that exists and is not in the inventory.
//!
//! # What it deliberately does not check
//!
//! That the *implementation* is right. Nothing mechanical can. What it buys is
//! that the inventory cannot rot silently, which is what happened to
//! `input.rs`'s "the only reader of `g_mouseLeftDoubleClick`" — a claim that was
//! true when it was written, false by the time anybody looked, and checked by
//! nothing.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    // `CARGO_MANIFEST_DIR` is `<root>/crates/l2-game`.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/l2-game has two ancestors")
        .to_path_buf()
}

/// The marker on one line, if it holds one.
///
/// A marker is `// arm: <id>` **on its own**, and it is allowed to be nested
/// inside a doc comment — `/// // arm: 0x…` — because the natural place for it
/// is the last line of the doc comment that explains the arm. So everything
/// before it on the line has to be comment punctuation and nothing else, which
/// is what stops a mention of the convention inside prose from counting as one.
fn marker_on(line: &str) -> Option<String> {
    let t = line.trim();
    let i = t.find("// arm:")?;
    if !t[..i].chars().all(|c| matches!(c, '/' | '!' | '*' | ' ' | '\t')) {
        return None;
    }
    let id = t[i + "// arm:".len()..].split_whitespace().next()?;
    (!id.is_empty()).then(|| id.to_string())
}

/// Every `// arm: <id>` in the workspace's Rust, with the file it is in.
///
/// A hand-rolled walk rather than a crate: this test must not add a dependency
/// to build, and the tree is a few hundred files.
fn markers(root: &Path) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut stack = vec![root.join("crates")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let path = e.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n == "target") {
                    continue;
                }
                stack.push(path);
            } else if path.extension().is_some_and(|x| x == "rs") {
                let Ok(text) = std::fs::read_to_string(&path) else { continue };
                for line in text.lines() {
                    let Some(id) = marker_on(line) else { continue };
                    let rel = path
                        .strip_prefix(root)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .replace('\\', "/");
                    out.entry(id.to_string()).or_default().push(rel);
                }
            }
        }
    }
    out
}

/// The `arms` array's `(id, status)` pairs, without pulling in a JSON crate.
///
/// The file is written by hand and read by a person; a three-line scanner over
/// `"id"` and `"status"` lines is enough and cannot pull a dependency into the
/// build. It **fails loudly** if the two do not alternate, which is what would
/// happen if somebody reformatted the file onto one line.
fn records(root: &Path) -> Vec<(String, String, Option<String>)> {
    let text = std::fs::read_to_string(root.join("docs/arms.json")).expect("docs/arms.json");
    let field = |line: &str, name: &str| -> Option<String> {
        let key = format!("\"{name}\":");
        let rest = line.trim().strip_prefix(&key)?;
        let rest = rest.trim().trim_end_matches(',').trim();
        Some(rest.trim_matches('"').to_string())
    };
    let mut out = Vec::new();
    let mut id: Option<String> = None;
    let mut ours: Option<String> = None;
    for line in text.lines() {
        if let Some(v) = field(line, "id") {
            assert!(id.is_none(), "two \"id\" lines with no \"status\" between them: {v}");
            id = Some(v);
        } else if let Some(v) = field(line, "status") {
            let key = id.take().expect("a \"status\" line with no \"id\" above it");
            out.push((key, v, ours.take()));
        } else if let Some(v) = field(line, "ours") {
            if let Some(last) = out.last_mut() {
                last.2 = Some(v);
            }
        }
    }
    assert!(id.is_none(), "the last record has an \"id\" and no \"status\"");
    assert!(out.len() > 20, "only {} records parsed — the scanner lost the file", out.len());
    out
}

/// **The check.**
#[test]
fn every_reproduced_arm_has_a_marker_and_every_marker_has_a_record() {
    let root = repo_root();
    let found = markers(&root);
    let recorded = records(&root);

    let claimed: BTreeSet<String> = recorded
        .iter()
        .filter(|(_, status, _)| status == "reproduced")
        .map(|(id, _, _)| id.clone())
        .collect();
    let marked: BTreeSet<String> = found.keys().cloned().collect();

    let unkept: Vec<&String> = claimed.difference(&marked).collect();
    let unrecorded: Vec<&String> = marked.difference(&claimed).collect();

    assert!(
        unkept.is_empty(),
        "docs/arms.json claims these are reproduced and no `// arm:` marker says so:\n  {}",
        unkept.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n  "),
    );
    assert!(
        unrecorded.is_empty(),
        "these `// arm:` markers are in the code and not in docs/arms.json:\n  {}",
        unrecorded
            .iter()
            .map(|id| format!("{id}  ({})", found[*id].join(", ")))
            .collect::<Vec<_>>()
            .join("\n  "),
    );
}

/// One marker per arm, so a record cannot silently mean two places.
#[test]
fn no_arm_is_marked_twice() {
    let root = repo_root();
    for (id, files) in markers(&root) {
        assert_eq!(files.len(), 1, "{id} is marked in {} places: {files:?}", files.len());
    }
}

/// Every id starts with something a `grep` for the marker convention finds, and
/// every status is one of the four the file defines.
#[test]
fn the_inventory_uses_the_four_statuses_and_the_marker_shape() {
    let root = repo_root();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for (id, status, ours) in records(&root) {
        assert!(
            id.starts_with("0x") || id.starts_with("ours/"),
            "{id} is neither an address nor an invention",
        );
        assert!(
            matches!(status.as_str(), "reproduced" | "missing" | "dead" | "invention"),
            "{id} has status {status}",
        );
        if status == "reproduced" {
            let ours = ours.unwrap_or_default();
            assert!(ours.starts_with("crates/"), "{id} does not say where it lives");
        }
        *counts.entry(status).or_default() += 1;
    }
    // The four are all populated, which is the point of having four.
    for s in ["reproduced", "missing", "dead", "invention"] {
        assert!(counts.get(s).copied().unwrap_or(0) > 0, "no {s} arms at all");
    }
}
