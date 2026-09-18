// docs/oracle-checks/*.json: every record parses, and every behaviour names a file
// that exists and an address that file cites. `node tools/oracle/checked.js --verify`
// is the judge; this test is how the suite runs it.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/l2-testkit/../..")
        .to_path_buf()
}

#[test]
fn records_parse_and_their_citations_exist() {
    let dir = root().join("docs/oracle-checks");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .expect("docs/oracle-checks")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".json"))
        .collect();
    names.sort();
    assert!(!names.is_empty(), "docs/oracle-checks holds no record");

    let out = match Command::new("node")
        .args(["tools/oracle/checked.js", "--verify"])
        .current_dir(root())
        .output()
    {
        Ok(o) => o,
        Err(_) => return, // no node: docs/environment.md says the suite still runs
    };
    let said = String::from_utf8_lossy(&out.stdout).to_string()
        + &String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "checked.js --verify:\n{said}");
}
