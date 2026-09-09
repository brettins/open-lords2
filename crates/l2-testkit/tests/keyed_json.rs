//! **The symbol databases are keyed files, and this keeps them mergeable.**
//!
//! `docs/symbols.json` once produced nine conflict hunks in which the `ours`
//! body of every hunk sat under the **wrong entry's `name`** — git had aligned
//! two arrays that were ordered differently, so the diff paired unrelated
//! records and offered `County_PlaceBlacksmith`'s comment as a candidate body
//! for `Move_BuildCostMap`. Nothing was wrong with the data; only the order
//! differed. A textual resolution would have produced a symbol database that
//! parses, reads plausibly, and lies. `docs/agents.md`, *A file that looks like
//! data is usually a claim*.
//!
//! There are two fixes and they are not equal.
//!
//! `tools/symbols/merge-json.js` **handles** the case: a three-way merge keyed
//! per array. The canonical sort **removes** it: two branches that both keep the
//! file in address order cannot produce a misaligned diff in the first place,
//! whatever git does and whether or not the driver is registered. That is the
//! better half, and it is this file's first test — *prefer a shape that cannot
//! be wrong to a check that notices when it is*.
//!
//! The second test is about the driver's one weakness: **git will not run a
//! merge driver a repository merely names.** `.gitattributes` asking for
//! `merge=l2json` does nothing until someone registers it in their own clone,
//! and nobody reads setup instructions. So the check is here, where it runs
//! rather than waits to be read, and it fails with the command in the message.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/l2-testkit/../..")
        .to_path_buf()
}

/// The three files `.gitattributes` marks `merge=l2json`.
const KEYED: &[&str] = &["docs/symbols.json", "docs/hypotheses.json", "docs/records.json"];

/// **Every address-keyed array is in address order.**
///
/// Only the `addr` arrays are asserted, and deliberately: they are the large
/// ones — 812 functions, 512 globals — and the only ones a diff has ever
/// misaligned. `sections`, `claims` and `corrections` are short lists whose
/// authors' order may mean something, so they are left alone rather than sorted
/// on the assumption that it does not.
#[test]
fn every_address_keyed_array_is_in_address_order() {
    let mut wrong = Vec::new();
    let mut checked = 0usize;

    for rel in KEYED {
        let path = root().join(rel);
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{rel}: {e}"));

        // A text scan, not a parser, and that is not a shortcut: what is being
        // asserted IS a property of the text -- the order entries appear in the
        // file, which is exactly what a line-based diff aligns on. These files
        // are written by `JSON.stringify(_, null, 2)`, so a top-level array
        // opens at two spaces and its entries carry `"addr"` at six.
        let mut current: Option<String> = None;
        let mut addrs: Vec<String> = Vec::new();
        let mut close = |name: &Option<String>, addrs: &mut Vec<String>, wrong: &mut Vec<String>, checked: &mut usize| {
            if let (Some(name), false) = (name.as_ref(), addrs.is_empty()) {
                *checked += 1;
                if let Some(at) = addrs.windows(2).position(|w| w[0] > w[1]) {
                    wrong.push(format!(
                        "  {rel}: {name} is out of order at {} / {}",
                        addrs[at],
                        addrs[at + 1]
                    ));
                }
            }
            addrs.clear();
        };

        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("  \"") {
                if let Some((name, tail)) = rest.split_once("\": [") {
                    if tail.is_empty() {
                        close(&current, &mut addrs, &mut wrong, &mut checked);
                        current = Some(name.to_string());
                        continue;
                    }
                }
            }
            if line == "  ]," || line == "  ]" {
                close(&current, &mut addrs, &mut wrong, &mut checked);
                current = None;
                continue;
            }
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("\"addr\": \"") {
                if let Some((value, _)) = rest.split_once('"') {
                    addrs.push(value.to_ascii_lowercase());
                }
            }
        }
        close(&current, &mut addrs, &mut wrong, &mut checked);
    }
    assert!(checked >= 4, "only {checked} address-keyed arrays found; the scanner is broken");
    assert!(
        wrong.is_empty(),
        "{} address-keyed array(s) are not in address order:\n{}\n\n\
         Sorting them is not tidiness. Two branches that both keep these files \
         in address order cannot produce a misaligned diff, whatever git does \
         and whether or not the merge driver is registered — which is the only \
         defence that works on a clone nobody configured. An unsorted array \
         restores the failure that once offered one function's comment as a \
         candidate body for another (docs/agents.md, and tools/symbols/merge-json.js \
         sorts what it merges, so re-running a merge through the driver fixes this).",
        wrong.len(),
        wrong.join("\n"),
    );
}

/// **The `merge=l2json` driver is registered in this clone.**
///
/// `.gitattributes` names it; git refuses to *run* a driver a repository merely
/// names, which is a sensible refusal to execute code on checkout. So the
/// attribute alone does nothing, and the failure is silent in the direction that
/// matters: the merge falls back to text, which is safe only because it is
/// noisy — and tonight git took both sides of three duplicates without a murmur,
/// so "noisy" is not a property to lean on.
///
/// Skipped rather than failed outside a git checkout: a source tarball has no
/// `.git`, and there is nothing to configure or to protect.
#[test]
fn the_keyed_json_merge_driver_is_registered() {
    let root = root();
    if !root.join(".git").exists() {
        return; // not a checkout; nothing to merge
    }
    let out = Command::new("git")
        .args(["config", "--get", "merge.l2json.driver"])
        .current_dir(&root)
        .output();
    let Ok(out) = out else { return }; // no git on the path
    let value = String::from_utf8_lossy(&out.stdout).trim().to_string();

    assert!(
        out.status.success() && value.contains("merge-json.js"),
        "the keyed JSON merge driver is not registered in this clone.\n\n\
         Run:  sh tools/symbols/install-merge-driver.sh\n\n\
         `.gitattributes` marks {} as `merge=l2json`, but git will not run a \
         driver a repository merely names — it would be executing code on \
         checkout. Until it is registered those files fall back to the ordinary \
         text merge, which once produced nine conflict hunks whose `ours` bodies \
         sat under the wrong entries' names. This check is here rather than in a \
         README because a setup instruction that is not run is not a defence.",
        KEYED.join(", "),
    );
}
