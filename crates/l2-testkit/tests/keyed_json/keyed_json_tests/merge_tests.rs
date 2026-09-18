#![allow(unused_imports)]
use super::*;
use super::validation_tests::*;
use super::driver_helpers::*;
use super::*;
use std::path::{Path, PathBuf};
use std::process::Command;

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

/// On the mercenaries merge (C164) the driver rewrote this file from 274 lines to
/// 1,617, pretty-printed, and reported the merge clean. The content was the right
/// union; the layout broke the file's contract, which is one object per line
/// because `crates/l2-scenario/tests/stored_fields/main.rs` scans it by line, and three
/// of those tests went red.
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


