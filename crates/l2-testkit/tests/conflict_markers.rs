//! **No conflict marker reaches `main`.**
//!
//! This exists because four of them did. The `input-arms` merge left
//! `<<<<<<< HEAD` / `=======` / `>>>>>>> inputarms` hunks committed in
//! `README.md`, `docs/plan.md` and twice in `docs/status.html`, and they sat on
//! `main` through a push, a full test run and all five checks.
//!
//! The reason they were invisible is the whole lesson: **both sides of every one
//! of those hunks were textually identical.** Git raised the conflict on
//! surrounding context, not on content, so resolving it was a no-op — and a
//! no-op resolution leaves nothing that reads wrong. Worse, `figures.js` went on
//! rewriting the marked number inside *both* halves, twice per figure, and
//! reported success: a generator that finds its markers does not care that it
//! found two of them.
//!
//! So no existing instrument could have seen it. `cargo test` compiles no
//! Markdown. `figures.js --check` was satisfied. `symbols_md.js`,
//! `corrections.js` and the census test all look at files that happened not to
//! be hit. It was found by eye, in a `git diff` run for an unrelated reason,
//! and only because that diff put the two identical halves next to each other.
//!
//! The check is trivial and belongs in the suite rather than in a reviewer's
//! head: *prefer a shape that cannot be wrong to a check that notices when it
//! is* — and where the shape cannot be fixed, at least let the noticing be
//! automatic.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/l2-testkit/../..")
        .to_path_buf()
}

/// Extensions worth scanning. Everything else is either binary or generated.
const TEXT: &[&str] = &[
    "rs", "md", "json", "toml", "js", "html", "java", "ps1", "yml", "yaml", "txt", "lock",
];

/// A marker line, and whether it is inside a fenced code block.
struct Hit {
    file: String,
    line: usize,
    text: String,
    fenced: bool,
}

fn is_marker(line: &str) -> bool {
    for m in ["<<<<<<<", "=======", ">>>>>>>"] {
        if let Some(rest) = line.strip_prefix(m) {
            // `=======` may stand alone; the other two always name a side.
            if rest.is_empty() || rest.starts_with(' ') {
                return true;
            }
        }
    }
    false
}

/// Every marker line in the tracked tree, fenced ones included.
///
/// Fenced hits are kept rather than filtered here so the caller can assert that
/// the exemption is actually exercised. An exemption no test travels is an
/// exemption that could be swallowing everything.
fn scan() -> Vec<Hit> {
    let root = root();
    let out = Command::new("git")
        .arg("ls-files")
        .current_dir(&root)
        .output()
        .expect("git ls-files");
    assert!(out.status.success(), "git ls-files failed");

    let mut hits = Vec::new();
    for rel in String::from_utf8_lossy(&out.stdout).lines() {
        let ext = Path::new(rel).extension().and_then(|e| e.to_str()).unwrap_or("");
        if !TEXT.contains(&ext) {
            continue;
        }
        // This file necessarily contains the patterns it hunts for.
        if rel.ends_with("tests/conflict_markers.rs") {
            continue;
        }
        let Ok(body) = std::fs::read_to_string(root.join(rel)) else {
            continue;
        };
        let markdown = ext == "md";
        let mut fenced = false;
        for (i, line) in body.lines().enumerate() {
            if markdown && line.trim_start().starts_with("```") {
                fenced = !fenced;
                continue;
            }
            if is_marker(line) {
                hits.push(Hit {
                    file: rel.to_string(),
                    line: i + 1,
                    text: line.chars().take(60).collect(),
                    fenced,
                });
            }
        }
    }
    hits
}

#[test]
fn no_conflict_marker_is_committed() {
    let live: Vec<_> = scan().into_iter().filter(|h| !h.fenced).collect();
    if live.is_empty() {
        return;
    }
    let mut msg = format!(
        "{} conflict marker(s) are committed to the tree:\n\n",
        live.len()
    );
    for h in &live {
        msg.push_str(&format!("  {}:{}  {}\n", h.file, h.line, h.text));
    }
    msg.push_str(
        "\nA merge was resolved by committing the markers. Note that BOTH SIDES may be\n\
         identical — that is how four of these reached main — so the absence of an\n\
         obvious wrong value is not evidence that the hunk is harmless. Open each one,\n\
         decide what the text should be, and delete all three marker lines.\n\
         If a marker is deliberate prose, put it in a ``` fenced block.\n",
    );
    panic!("{msg}");
}

/// The fence exemption is real, and this is the file that proves it.
///
/// `docs/agents.md` quotes a genuine conflict hunk — the nine-hunk
/// `symbols.json` misalignment — inside a code fence, and it must not be
/// flagged. Asserting that it is *found and excused* is what stops the fence
/// logic from silently swallowing live markers too: a filter with nothing on
/// either side of it is untested in both directions.
#[test]
fn the_fence_exemption_is_exercised_by_a_real_example() {
    let hits = scan();
    let fenced: Vec<_> = hits.iter().filter(|h| h.fenced).collect();
    assert!(
        fenced.iter().any(|h| h.file.ends_with("docs/agents.md")),
        "docs/agents.md's quoted conflict hunk was not found inside a fence.\n\
         Either the example was removed — in which case pick another fenced\n\
         example or delete this test deliberately — or the fence tracking in\n\
         scan() has broken, in which case no_conflict_marker_is_committed is\n\
         now excusing markers it should be reporting."
    );
}
