#![allow(unused_imports)]
use super::*;
use super::validation_tests::*;
use super::driver_helpers::*;
use super::*;
use std::path::{Path, PathBuf};
use std::process::Command;

/// **The ledger merges by id, and keeps its order and its shape.**
///
/// `docs/work.json` is the one keyed file whose array order is intent and whose
/// rows are one line each
/// sorts by key and pretty-prints. So this runs the real driver on a three-way
/// merge in which each side does something a text merge would find adjacent:
///
/// * ours changes one row's state and adds a row after the first;
/// * theirs deletes a row, adds a different row further down, and rewrites
///   `about` — a sibling member, not a row.
///
/// The rows are deliberately **not** in id order
/// pass by accident. And the result is compared **byte for byte** with the
/// expected file, which is the claim exactly: every change kept, in the file's
/// order, in the file's shape. A synthetic ledger
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

/// **`docs/stored-fields.json` merges by id and stays one row per line.**
///
/// On the mercenaries merge (C164) the driver rewrote this file from 274 lines to
/// 1,617, pretty-printed, and reported the merge clean. The content was the right
/// union; the layout broke the file's contract, which is one object per line
/// because `crates/l2-scenario/tests/stored_fields/main.rs` scans it by line, and three
/// of those tests went red.
///
/// So this merges **the real file** three ways — ours rewrites one row and adds a
/// County row, theirs deletes a row and adds a Realm row — and compares the
/// result byte for byte with the file those four line edits make. The new ids
/// sort after every County row and after every Realm row respectively, so the
/// expected positions follow from the file's own order (County then Realm, each
/// by offset, which is key order). A driver that pretty-prints, drops a side, or
/// sorts differently cannot produce these bytes.
#[test]
fn stored_fields_merges_by_id_and_stays_one_row_per_line() {
    let base = std::fs::read_to_string(root().join("docs/stored-fields.json")).expect("docs/stored-fields.json");
    let (head, rows, tail) = split_rows(&base);
    assert_eq!(join_rows(&head, &rows, &tail), base, "docs/stored-fields.json is not one row per line to begin with");
    let id = |row: &str| row.split("\"id\": \"").nth(1).and_then(|s| s.split('"').next()).unwrap_or_default().to_string();
    let first_realm = rows.iter().position(|r| id(r).starts_with("Realm+")).expect("a Realm row");
    assert!(first_realm > 3, "this test needs at least four County rows");

    let changed = format!(r#"    {{"id": "{}", "name": "rewrittenByOurs", "type": "u8", "status": "excluded", "why": "ours rewrote this row"}}"#, id(&rows[1]));
    let ours_new = r#"    {"id": "County+0xZZ1", "name": "addedByOurs", "type": "u8", "status": "excluded", "why": "ours added this row"}"#.to_string();
    let theirs_new = r#"    {"id": "Realm+0xZZ2", "name": "addedByTheirs", "type": "u8", "status": "excluded", "why": "theirs added this row"}"#.to_string();

    let mut ours = rows.clone();
    ours[1] = changed.clone();
    ours.insert(first_realm, ours_new.clone());
    let mut theirs = rows.clone();
    theirs.remove(2);
    theirs.push(theirs_new.clone());
    let mut expected = rows.clone();
    expected[1] = changed;
    expected.insert(first_realm, ours_new);
    expected.remove(2);
    expected.push(theirs_new);

    let Some((ok, merged, said)) = drive(
        "docs/stored-fields.json",
        &base,
        &join_rows(&head, &ours, &tail),
        &join_rows(&head, &theirs, &tail),
    ) else {
        return; // no node on this machine; the CI job has one
    };
    assert!(ok, "the driver refused a merge with no row changed on both sides:\n{said}");
    if merged != join_rows(&head, &expected, &tail) {
        let got = merged.lines().count();
        panic!(
            "the merged stored-fields.json is not the expected file ({got} lines, expected {}). \
             Far more lines than expected means it was pretty-printed, which is the C164 defect; \
             the same count means a row was lost, kept, or misplaced.",
            join_rows(&head, &expected, &tail).lines().count()
        );
    }
}

/// **The driver refuses a merge that would change a file's layout.**
///
/// The same one-row-per-line rows, merged under a path that has no
/// `FILE_POLICY`: the driver's default layout for such a path is plain two-space
/// JSON
/// the `ours` file untouched, and `REFUSED` said — so git shows a conflict
#[test]
fn the_driver_refuses_a_merge_that_would_reformat_the_file() {
    let base = std::fs::read_to_string(root().join("docs/stored-fields.json")).expect("docs/stored-fields.json");
    let (head, rows, tail) = split_rows(&base);
    let mut ours = rows.clone();
    ours.remove(1);
    let mut theirs = rows.clone();
    theirs.remove(2);
    let ours = join_rows(&head, &ours, &tail);
    let Some((ok, left, said)) = drive("docs/a-file-with-no-layout-policy.json", &base, &ours, &join_rows(&head, &theirs, &tail)) else {
        return; // no node on this machine; the CI job has one
    };
    assert!(!ok, "the driver reported a clean merge for a file it would have reformatted");
    assert_eq!(left, ours, "the driver refused but still rewrote our side");
    assert!(said.contains("REFUSED"), "the refusal did not say so:\n{said}");
}

/// **The `merge=l2json` driver is registered in this clone.**
///
/// `.gitattributes` names it; git refuses to *run* a driver a repository merely
/// names, which is a sensible refusal to execute code on checkout. So the
/// attribute alone does nothing
/// matters: the merge falls back to text, which is safe only because it is
/// noisy — and tonight git took both sides of three duplicates without a murmur
/// so "noisy" is not a property to lean on.
///
/// Skipped
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


