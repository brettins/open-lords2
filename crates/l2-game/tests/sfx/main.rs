//! So the marker is `// sfx: <id>[,<id>…]` and the rule the check actually
//! enforces is the one that matters: **no id is claimed twice**, and the set of
//! claimed ids equals the set the file calls `reproduced`.

mod sfx_tests;
pub use sfx_tests::*;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/l2-game has two ancestors")
        .to_path_buf()
}

/// The same shape as `tests/arms.rs`'s: everything before `// sfx:` has to be
/// comment punctuation, so a mention of the convention inside prose is not a
/// marker. That rule is what lets this file and `docs/audio.json` both describe
/// the convention without either of them counting as a use of it.
fn marker_on(line: &str) -> Option<Vec<String>> {
    let t = line.trim();
    let i = t.find("// sfx:")?;
    if !t[..i].chars().all(|c| matches!(c, '/' | '!' | '*' | ' ' | '\t')) {
        return None;
    }
    let rest = t[i + "// sfx:".len()..].split_whitespace().next()?;
    let ids: Vec<String> = rest.split(',').filter(|s| !s.is_empty()).map(str::to_string).collect();
    (!ids.is_empty()).then_some(ids)
}

/// Every `// sfx:` claim in the workspace's Rust, by id, with the file it is in.
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
                let rel =
                    path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
                for line in text.lines() {
                    for id in marker_on(line).unwrap_or_default() {
                        out.entry(id).or_default().push(rel.clone());
                    }
                }
            }
        }
    }
    out
}

struct Site {
    id: String,
    addr: String,
    class: String,
    status: String,
    ours: Option<String>,
    note: Option<String>,
}

const TRIGGER_SITES: usize = 143;

const STATUSES: &[&str] = &["reproduced", "blocked", "dead", "missing"];

/// | id | why it cannot run | bugs.md |
/// |---|---|---|
/// | `FUN_0040d6ad#1`, `FUN_0040d7b8#1` | the two arrows of a slider widget whose hit-tester `FUN_0040D3F5` has **no caller**: zero rel32 calls, zero jumps and zero absolute references in the image, against 36 rel32 callers for `Widget_Test` as the control | `D39` |
/// | `Battle_PauseButton#1` | guarded on a word that alternates between 0 and −1; the only writer that could make it 1, `FUN_00434E68`, has no reference either | D38 |
/// | `FUN_004b39e8#1` | the degraded castle's three lines: the thunk that plays them has **no caller** — zero rel32 calls, zero jumps and zero dword references in the image, against one rel32 caller for its sibling `FUN_004B3714` as the control | `D40` |
///
/// **A pinned set, not a relaxed check.** The fourth entry cost exactly what the
/// first did, and so will the fifth: say in `note` which guard can never hold —
/// or, as `FUN_004b39e8#1` does, that *nothing calls the function at all* — add
/// it to `docs/bugs.md`, and add its id here.
const DEAD_IN_THE_SHIPPED_GAME: &[&str] =
    &["Battle_PauseButton#1", "FUN_0040d6ad#1", "FUN_0040d7b8#1", "FUN_004b39e8#1"];

