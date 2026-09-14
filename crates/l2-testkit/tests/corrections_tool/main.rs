//! **`tools/decisions/corrections.js`, asked about trees built to break it.**
//!
//! The tool guards every merge: it numbers the two logs' entries, checks their
//! citations, and assigns the placeholders branches write. CI runs its `--check`
//! against this tree, which says the tree is clean and says nothing about
//! whether the tool can see the things it exists to see. That second question is
//! this file's, and each test is a shape that
//!
//! * **a heading the tool cannot read** — two branches wrote a Markdown heading
//!   over a placeholder, and C146's em-dash arrived double-encoded, so its
//!   correction did not exist to the tool and three citations pointed at nothing
//!   (`docs/decisions.md` C147);
//! * **a placeholder family the tool did not know** — a dead-code entry in
//!   `docs/bugs.md` was written with a `D` placeholder, and nothing reported or
//!   assigned it;
//! * **assignment by `perl -pi`** — the integrator's only command for it, and a
//!   tool with no encoding layer is what double-encodes UTF-8.
//!
//! Each test builds a small tree in the temp directory and points the tool at it
//! with `--root`. Placeholders are assembled at run time by [`tag`], because this
//! file is itself in the tree `corrections.js --check` scans, and a literal one
//! here would be reported as unassigned.
//!
//! Every test states the ablation that turns it red; they were run, not assumed.
//! Skipped when, as `keyed_json.rs` is: CI has one.

mod corrections_tool_tests;
pub use corrections_tool_tests::*;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/l2-testkit/../..")
        .to_path_buf()
}

/// A placeholder, assembled so that this file does not contain one.
fn tag(letter: char, slug: &str) -> String {
    format!("{letter}NEW-{slug}")
}

/// The UTF-8 of an em-dash read as CP1252 and encoded again — the bytes that
/// landed in C146's heading. Written as numbers, as `mojibake.rs` writes them, so
/// this file cannot be corrupted into agreeing with the corruption.
const MANGLED_EM_DASH: [u8; 8] = [0xC3, 0xA2, 0xE2, 0x82, 0xAC, 0xE2, 0x80, 0x9D];

const DECISIONS: &str = "\
# Decisions and corrections

## Decisions

**D1 — The first decision.**

**D4 — A later decision, after a gap.**

## Corrections

**C1 — The first correction.**

Some prose.

**C2 — The second correction, which cites C1.**

Correction C1 was wrong about this.
";

const BUGS: &str = "\
# The original's bugs

# 2. The catalogue — the original's bugs, reproduced

### B1 — The first bug

| | the original's bug | evidence | our code |
|---|---|---|---|
| **B2** | A row. | [V] | here |
| ~~**B3**~~ | A retracted row. | — | — |

# 3. The original's bugs we do **not** reproduce

| **N1** | Not reproduced. | ours |

# 4. Surprising, and **not** a bug

### S1 — Not a bug

# 5. Dead and unreachable code in the original

### D1 — The first dead code

| **D2** | More dead code. | — |
";

const SOURCE: &str = "// The rule in docs/decisions.md C2 says so.\npub fn f() {}\n";

struct Tree {
    dir: PathBuf,
}

impl Tree {
    /// The base tree, relocked. `None` when.
    fn new(name: &str) -> Option<Tree> {
        Command::new("node").arg("--version").output().ok()?;
        let dir = std::env::temp_dir().join(format!("l2-corrections-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let tree = Tree { dir };
        tree.write("docs/decisions.md", DECISIONS);
        tree.write("docs/bugs.md", BUGS);
        tree.write("crates/x/src/lib.rs", SOURCE);
        tree.relock();
        Some(tree)
    }

    fn write(&self, rel: &str, bytes: impl AsRef<[u8]>) {
        let p = self.dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, bytes).unwrap();
    }

    fn append(&self, rel: &str, text: &str) {
        let mut bytes = self.read(rel);
        bytes.extend_from_slice(text.as_bytes());
        self.write(rel, bytes);
    }

    fn read(&self, rel: &str) -> Vec<u8> {
        std::fs::read(self.dir.join(rel)).unwrap()
    }

    fn text(&self, rel: &str) -> String {
        String::from_utf8(self.read(rel)).unwrap()
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new("node")
            .arg(repo().join("tools/decisions/corrections.js"))
            .args(args)
            .arg("--root")
            .arg(&self.dir)
            .output()
            .expect("node ran once already")
    }

    fn relock(&self) {
        let out = self.run(&["--relock"]);
        assert!(out.status.success(), "--relock failed on a tree built to pass:\n{}", err(&out));
    }

    /// Every file and its bytes, to show a refusal changed nothing.
    fn snapshot(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fn walk(dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else {
                    out.insert(p.clone(), std::fs::read(&p).unwrap());
                }
            }
        }
        let mut out = BTreeMap::new();
        walk(&self.dir, &mut out);
        out
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn err(out: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
}

/// The line a string sits on in a tree file, 1-based, for asserting that an
/// error names it.
fn line_of(text: &str, needle: &str) -> usize {
    text.lines().position(|l| l.contains(needle)).expect("the needle is in the file") + 1
}

/// Asserts `--check` fails because of rule 7, naming `file:line`.
fn assert_malformed(tree: &Tree, file: &str, needle: &str, why: &str) {
    let out = tree.run(&["--check"]);
    let e = err(&out);
    let at = format!("{file}:{}", line_of(&tree.text(file), needle));
    assert!(!out.status.success(), "--check passed a tree with the line `{needle}` in {file}");
    assert!(
        e.contains("look like a numbered entry") && e.contains(&at) && e.contains(why),
        "--check failed on `{needle}` but not as a malformed heading at {at} saying \"{why}\":\n{e}"
    );
}

