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
fn records(root: &Path) -> Vec<Record> {
    let text = std::fs::read_to_string(root.join("docs/arms.json")).expect("docs/arms.json");
    let field = |line: &str, name: &str| -> Option<String> {
        let key = format!("\"{name}\":");
        let rest = line.trim().strip_prefix(&key)?;
        let rest = rest.trim().trim_end_matches(',').trim();
        Some(rest.trim_matches('"').to_string())
    };
    let mut out: Vec<Record> = Vec::new();
    let mut id: Option<String> = None;
    for line in text.lines() {
        if let Some(v) = field(line, "id") {
            assert!(id.is_none(), "two \"id\" lines with no \"status\" between them: {v}");
            id = Some(v);
        } else if let Some(v) = field(line, "status") {
            let key = id.take().expect("a \"status\" line with no \"id\" above it");
            out.push(Record { id: key, status: v, ours: None, removed: false });
        } else if let Some(v) = field(line, "ours") {
            if let Some(last) = out.last_mut() {
                last.ours = Some(v);
            }
        } else if let Some(v) = field(line, "removed") {
            if let Some(last) = out.last_mut() {
                last.removed = v == "true";
            }
        }
    }
    assert!(id.is_none(), "the last record has an \"id\" and no \"status\"");
    assert!(out.len() > 20, "only {} records parsed — the scanner lost the file", out.len());
    out
}

/// One record of `docs/arms.json`.
struct Record {
    id: String,
    status: String,
    ours: Option<String>,
    /// `invention` only: whether the code has since been taken out.
    removed: bool,
}

impl Record {
    /// **Is this arm in the tree right now**, and therefore required to carry a
    /// marker?
    ///
    /// Not the same question as *"do we implement it"*, and the difference is
    /// the whole reason this is a method rather than a set membership test. A
    /// **removed** invention is something we implemented and took out: it stays
    /// in the file, because the count of inventions is half the 1:1
    /// measurement and deleting the record would delete the evidence, but there
    /// is no code left to mark.
    ///
    /// The original rule was `status == "reproduced"`, which is right today and
    /// only by accident: every invention on file happens to be removed. An
    /// invention we decided to KEEP would need a marker and would have slipped
    /// through, which is the hole this closes.
    fn in_the_tree(&self) -> bool {
        match self.status.as_str() {
            "reproduced" | "dead-reproduced" => true,
            "invention" => !self.removed,
            _ => false,
        }
    }
}

/// Every status the file defines. The cross-product of *what the original has*
/// and *what we have*, minus the cell that is not a record.
const STATUSES: &[&str] = &["reproduced", "missing", "dead", "invention", "dead-reproduced"];

/// **The check.**
#[test]
fn every_reproduced_arm_has_a_marker_and_every_marker_has_a_record() {
    let root = repo_root();
    let found = markers(&root);
    let recorded = records(&root);

    let claimed: BTreeSet<String> = recorded
        .iter()
        .filter(|r| r.in_the_tree())
        .map(|r| r.id.clone())
        .collect();
    let marked: BTreeSet<String> = found.keys().cloned().collect();

    let unkept: Vec<&String> = claimed.difference(&marked).collect();
    let unrecorded: Vec<&String> = marked.difference(&claimed).collect();

    assert!(
        unkept.is_empty(),
        "docs/arms.json claims these are implemented and no `// arm:` marker says so:\n  {}",
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
/// every status is one the file defines.
#[test]
fn the_inventory_uses_the_defined_statuses_and_the_marker_shape() {
    let root = repo_root();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for r in records(&root) {
        let (id, status) = (&r.id, &r.status);
        assert!(
            id.starts_with("0x") || id.starts_with("ours/"),
            "{id} is neither an address nor an invention",
        );
        assert!(
            STATUSES.contains(&status.as_str()),
            "{id} has status {status}, which is not one of {STATUSES:?}",
        );
        if r.in_the_tree() {
            let ours = r.ours.clone().unwrap_or_default();
            assert!(ours.starts_with("crates/"), "{id} does not say where it lives");
        }
        *counts.entry(status.clone()).or_default() += 1;
    }
    // Four of the five must be populated — they are the four things this file
    // exists to count, and an empty one means the enumeration stopped early.
    //
    // `dead-reproduced` is the exception and is expected to be EMPTY: it names
    // work we should not have done. It is asserted the other way round, because
    // a category that quietly acquires members is exactly how "we built an arm
    // no player can reach" would stop being a finding and become a bucket.
    for s in ["reproduced", "missing", "dead", "invention"] {
        assert!(counts.get(s).copied().unwrap_or(0) > 0, "no {s} arms at all");
    }
    let built_dead = counts.get("dead-reproduced").copied().unwrap_or(0);
    assert_eq!(
        built_dead, 0,
        "{built_dead} arm(s) are marked dead-reproduced: we have built input the shipped \
         game cannot reach. That is a finding, not a status to live with — read each one \
         and delete the code, or prove the screen is reachable after all and re-file it. \
         If it is genuinely worth keeping, change this assertion deliberately and say why."
    );
}
