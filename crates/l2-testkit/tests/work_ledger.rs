//! **The work ledger's schema, checked where the suite runs.**
//!
//! `docs/work.json` holds every piece of live work as a row of *intent*, and
//! `tools/pm/work.js` derives everything git can answer about it. It exists
//! because `docs/plan.md`'s in-flight lists read as current long after they
//! were not — `docs/agents.md`, *The work ledger*.
//!
//! `work.js --check` has two halves. The **git half** needs the clone the work
//! happens in — the agent branches — and a CI runner is a fresh clone that has
//! none, so there it skips and says so. The **schema half** needs only the
//! file, and this is where it runs on every push.
//!
//! Both tests shell out to the tool rather than re-reading the JSON here. A Rust
//! copy of the schema would be a second list maintained by whoever maintains
//! the first, in the same commit, for the same reason — two artefacts that agree
//! because they were written to, which `docs/agents.md` records as the pattern
//! that lies.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/l2-testkit/../..")
        .to_path_buf()
}

fn work(args: &[&str]) -> Option<Output> {
    Command::new("node")
        .arg("tools/pm/work.js")
        .args(args)
        .current_dir(root())
        .output()
        .ok()
}

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
}

/// A ledger with one defect per row, and one row with none.
const BROKEN: &str = r##"{
  "about": "a ledger broken on purpose, one defect per row",
  "states": {"open": "known", "in-flight": "being worked"},
  "tracks": {"play": "a game plays"},
  "items": [
    {"id": "fine", "title": "t", "track": "play", "system": "s", "state": "open", "branch": null, "depends_on": [], "source": "", "next": "", "note": ""},
    {"id": "bad-state", "title": "t", "track": "play", "system": "s", "state": "shipped", "branch": null, "depends_on": [], "source": "", "next": "", "note": ""},
    {"id": "bad-track", "title": "t", "track": "sound", "system": "s", "state": "open", "branch": null, "depends_on": [], "source": "", "next": "", "note": ""},
    {"id": "no-next", "title": "t", "track": "play", "system": "s", "state": "open", "branch": null, "depends_on": [], "source": "", "note": ""},
    {"id": "typed-git", "title": "t", "track": "play", "system": "s", "state": "open", "branch": null, "depends_on": [], "source": "", "next": "", "note": "", "merged": true},
    {"id": "twice", "title": "t", "track": "play", "system": "s", "state": "open", "branch": null, "depends_on": [], "source": "", "next": "", "note": ""},
    {"id": "twice", "title": "t", "track": "play", "system": "s", "state": "open", "branch": null, "depends_on": [], "source": "", "next": "", "note": ""},
    {"id": "gone-dep", "title": "t", "track": "play", "system": "s", "state": "open", "branch": null, "depends_on": ["landed-and-left"], "source": "", "next": "", "note": ""},
    {"id": "loop-a", "title": "t", "track": "play", "system": "s", "state": "open", "branch": null, "depends_on": ["loop-b"], "source": "", "next": "", "note": ""},
    {"id": "loop-b", "title": "t", "track": "play", "system": "s", "state": "open", "branch": null, "depends_on": ["loop-a"], "source": "", "next": "", "note": ""},
    {"id": "branchless", "title": "t", "track": "play", "system": "s", "state": "in-flight", "branch": null, "depends_on": [], "source": "", "next": "", "note": ""}
  ]
}
"##;

/// **Every schema rule goes red, on the row it is about, and on no other.**
///
/// This is the ablation made permanent: `docs/agents.md` asks that a check
/// whose subject is a whole file be made to fail deliberately once, by
/// corrupting its input, and its message read. Here that happens on every run,
/// and what is asserted is the *identity* of each failure — which row, which
/// rule — rather than a count, because a count of nine can be nine of the
/// wrong things.
#[test]
fn a_broken_ledger_fails_and_names_every_broken_row() {
    let dir = std::env::temp_dir().join(format!("l2-work-ledger-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let file = dir.join("work.json");
    std::fs::write(&file, BROKEN).expect("write the broken ledger");
    let out = work(&["--check", "--schema", "--file", file.to_str().expect("utf-8 temp path")]);
    let _ = std::fs::remove_dir_all(&dir);
    let Some(out) = out else {
        return; // no node on this machine; the CI job has one
    };
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "a ledger with nine defects passed:\n{stderr}");

    let said = |row: &str, words: &str| {
        stderr
            .lines()
            .any(|l| l.starts_with(&format!("work: {row}: ")) && l.contains(words))
    };
    for (row, words) in [
        ("bad-state", "unknown state \"shipped\""),
        ("bad-track", "unknown track \"sound\""),
        ("no-next", "missing field \"next\""),
        ("typed-git", "carries \"merged\", which git answers"),
        ("twice", "duplicate id"),
        ("gone-dep", "depends_on \"landed-and-left\", which is not a row"),
        ("loop-a", "dependency cycle: loop-a -> loop-b -> loop-a"),
        ("branchless", "is in-flight with no branch"),
    ] {
        assert!(said(row, words), "no line names {row} with \"{words}\":\n{stderr}");
    }
    assert!(
        !stderr.contains("work: fine:"),
        "the one well-formed row was reported, so the check is not about rows:\n{stderr}"
    );
}
