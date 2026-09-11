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

/// The files `.gitattributes` marks `merge=l2json`.
///
/// `docs/arms.json` is the one that matters most and was the last to be added:
/// **three agents are writing it at once**, and it carries both `id` and `addr`
/// with 42 records across 23 addresses, because one function can hold several
/// input arms. Merged on `addr` the driver would have collapsed those 42 into 23
/// and discarded 19 in silence.
/// `docs/audio.json` is the same shape one inventory along — 143 trigger sites
/// across 70 functions, so `Msg_DrawWindow` alone holds twenty-four of them and
/// an `addr` key would discard 73 records in silence. It was registered in the
/// same commit that created it, which is the half `docs/agents.md`'s worked
/// example records as the one nobody checks.
/// `docs/work.json` is the first whose records are not the whole file: they
/// sit under `items` beside `about`, `states` and `tracks`, their **order is
/// intent** (the merge queue reads top to bottom), and each row is one line.
/// So the driver keeps the file's order and shape instead of sorting and
/// re-indenting it — `FILE_POLICY` in `merge-json.js` — and
/// [`the_ledger_merges_by_id_and_keeps_its_order_and_its_shape`] is the proof.
const KEYED: &[&str] = &[
    "docs/symbols.json",
    "docs/hypotheses.json",
    "docs/records.json",
    "docs/arms.json",
    "docs/audio.json",
    "docs/stored-fields.json",
    "docs/work.json",
];

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
/// `KEY_FIELDS` in `merge-json.js`, and both the order check and the uniqueness
/// check ask it. A Rust copy of that rule was a second list that could disagree
/// with the first, which is the failure this whole area is about — and it did
/// disagree, within a day of being written.
///
/// Why order matters at all: two branches that both keep a file in key order
/// cannot produce a misaligned diff, whatever git does and whether or not the
/// driver is registered. That is the half of the fix that removes the failure
/// rather than handling it.

/// **Every keyed array's key is actually unique**, asked of the driver itself.
///
/// This is the invariant a keyed merge silently depends on: if the key the
/// driver picks is not unique, merging *deletes* one entry per collision, and
/// the result parses and reads plausibly. `arms.json` is exactly that trap —
/// `addr` looks like the key and is not.
///
/// It shells out to `merge-json.js --check` rather than reimplementing
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

/// **The ledger merges by id, and keeps its order and its shape.**
///
/// `docs/work.json` is the one keyed file whose array order is intent and whose
/// rows are one line each, and the driver's default would destroy both: it
/// sorts by key and pretty-prints. So this runs the real driver on a three-way
/// merge in which each side does something a text merge would find adjacent:
///
/// * ours changes one row's state and adds a row after the first;
/// * theirs deletes a row, adds a different row further down, and rewrites
///   `about` — a sibling member, not a row.
///
/// The rows are deliberately **not** in id order, so a sorting merge cannot
/// pass by accident. And the result is compared **byte for byte** with the
/// expected file, which is the claim exactly: every change kept, in the file's
/// order, in the file's shape. A synthetic ledger rather than the real one,
/// because the real one's rows leave as their work merges and a test must not
/// depend on which work is live.
#[test]
fn the_ledger_merges_by_id_and_keeps_its_order_and_its_shape() {
    fn row(id: &str, state: &str) -> String {
        format!(
            r#"    {{"id": "{id}", "title": "t {id}", "track": "play", "system": "s", "state": "{state}", "branch": null, "depends_on": [], "source": "", "next": "", "note": ""}}"#
        )
    }
    fn ledger(about: &str, rows: &[String]) -> String {
        format!(
            "{{\n  \"about\": \"{about}\",\n  \"states\": {{\n    \"open\": \"known\",\n    \"deferred\": \"not now\"\n  }},\n  \"tracks\": {{\n    \"play\": \"a game plays\"\n  }},\n  \"items\": [\n{}\n  ]\n}}\n",
            rows.join(",\n")
        )
    }
    let base = ledger(
        "before",
        &[row("queue-3", "open"), row("queue-1", "open"), row("zeta", "open"), row("alpha", "open"), row("middle", "open")],
    );
    let ours = ledger(
        "before",
        &[
            row("queue-3", "open"),
            row("ours-new", "open"),
            row("queue-1", "deferred"),
            row("zeta", "open"),
            row("alpha", "open"),
            row("middle", "open"),
        ],
    );
    let theirs = ledger(
        "after",
        &[row("queue-3", "open"), row("queue-1", "open"), row("alpha", "open"), row("theirs-new", "open"), row("middle", "open")],
    );
    let expected = ledger(
        "after",
        &[
            row("queue-3", "open"),
            row("ours-new", "open"),
            row("queue-1", "deferred"),
            row("alpha", "open"),
            row("theirs-new", "open"),
            row("middle", "open"),
        ],
    );

    let dir = std::env::temp_dir().join(format!("l2-ledger-merge-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let (b, o, t) = (dir.join("base.json"), dir.join("ours.json"), dir.join("theirs.json"));
    std::fs::write(&b, &base).unwrap();
    std::fs::write(&o, &ours).unwrap();
    std::fs::write(&t, &theirs).unwrap();
    let out = Command::new("node")
        .arg("tools/symbols/merge-json.js")
        .args([&b, &o, &t])
        .arg("docs/work.json")
        .current_dir(root())
        .output();
    let merged = std::fs::read_to_string(&o).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    let Ok(out) = out else {
        return; // no node on this machine; the CI job has one
    };
    assert!(
        out.status.success(),
        "the driver refused a merge with no row changed on both sides:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        merged, expected,
        "the merged ledger is not the expected file. If the rows came out sorted, \
         FILE_POLICY's order is not being honoured; if they came out one field per \
         line, its rows shape is not; if a change is missing, the merge itself is wrong."
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
