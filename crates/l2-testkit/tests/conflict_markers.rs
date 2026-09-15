
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/l2-testkit/../..")
        .to_path_buf()
}

const TEXT: &[&str] = &[
    "rs", "md", "json", "toml", "js", "html", "java", "ps1", "yml", "yaml", "txt", "lock",
];

struct Hit {
    file: String,
    line: usize,
    text: String,
    fenced: bool,
}

fn is_marker(line: &str) -> bool {
    for m in ["<<<<<<<", "=======", ">>>>>>>"] {
        if let Some(rest) = line.strip_prefix(m) {
            if rest.is_empty() || rest.starts_with(' ') {
                return true;
            }
        }
    }
    false
}

pub(crate) fn scan() -> Vec<Hit> {
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
