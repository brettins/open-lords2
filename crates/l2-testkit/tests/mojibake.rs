//! This exists because a correction's heading did. An agent wrote
//! `**C146 — The differential's first finding…**` and what landed in
//! `docs/decisions.md` was `c3 a2 e2 82 ac e2 80 9d` where the em-dash belonged
//! — UTF-8 read as CP1252 and encoded to UTF-8 a second time. Every other
//! heading in that file carries the plain `e2 80 94`.

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

const DOUBLE_ENCODED: &[(&str, char, &str)] = &[
    ("\u{c3}\u{a2}\u{e2}\u{82}\u{ac}\u{e2}\u{80}\u{9d}", '—', "em dash"),
    ("\u{c3}\u{a2}\u{e2}\u{82}\u{ac}\u{e2}\u{80}\u{9c}", '–', "en dash"),
    ("\u{c3}\u{a2}\u{e2}\u{82}\u{ac}\u{cb}\u{9c}", '\u{2018}', "left single quote"),
    ("\u{c3}\u{a2}\u{e2}\u{82}\u{ac}\u{e2}\u{84}\u{a2}", '\u{2019}', "right single quote"),
    ("\u{c3}\u{a2}\u{e2}\u{82}\u{ac}\u{c5}\u{93}", '\u{201c}', "left double quote"),
    ("\u{c3}\u{a2}\u{e2}\u{82}\u{ac}\u{c2}\u{a6}", '…', "ellipsis"),
    ("\u{c3}\u{82}\u{c2}\u{a0}", '\u{a0}', "non-breaking space"),
    ("\u{c3}\u{83}\u{c2}\u{97}", '×', "multiplication sign"),
];

const EXEMPT: &[&str] = &["tests/mojibake.rs"];

struct Hit {
    file: String,
    line: usize,
    name: &'static str,
    intended: char,
    text: String,
}

struct Scan {
    hits: Vec<Hit>,
    files: usize,
}

pub(crate) fn scan() -> Scan {
    let root = root();
    let out = Command::new("git")
        .arg("ls-files")
        .current_dir(&root)
        .output()
        .expect("git ls-files");
    assert!(out.status.success(), "git ls-files failed");

    let mut hits = Vec::new();
    let mut files = 0usize;
    for rel in String::from_utf8_lossy(&out.stdout).lines() {
        let ext = Path::new(rel).extension().and_then(|e| e.to_str()).unwrap_or("");
        if !TEXT.contains(&ext) || EXEMPT.iter().any(|e| rel.ends_with(e)) {
            continue;
        }
        let Ok(bytes) = std::fs::read(root.join(rel)) else {
            continue;
        };
        let body: String = bytes.iter().map(|&b| b as char).collect();
        files += 1;
        for (i, line) in body.lines().enumerate() {
            for (mangled, intended, name) in DOUBLE_ENCODED {
                if line.contains(mangled) {
                    hits.push(Hit {
                        file: rel.to_string(),
                        line: i + 1,
                        name,
                        intended: *intended,
                        text: line.chars().take(60).collect(),
                    });
                    break;
                }
            }
        }
    }
    Scan { hits, files }
}

#[test]
fn no_double_encoded_text_is_committed() {
    let hits = scan().hits;
    if hits.is_empty() {
        return;
    }
    let mut msg = format!("{} line(s) carry double-encoded text:\n\n", hits.len());
    for h in &hits {
        msg.push_str(&format!(
            "  {}:{}  {} should be '{}'\n    {}\n",
            h.file, h.line, h.name, h.intended, h.text
        ));
    }
    msg.push_str(
        "\nThis is UTF-8 that was read as CP1252 and encoded again. It is still valid\n\
         UTF-8, so nothing else in the suite can see it, and it renders as visible\n\
         rubbish only where somebody happens to look. Replace each mangled run with\n\
         the character named above.\n\n\
         The usual source is a tool that rewrites a file without an encoding layer.\n\
         If you are editing documents from a script, read and write bytes.\n",
    );
    panic!("{msg}");
}

/// `no_double_encoded_text_is_committed` passes when the tree is clean, and a
/// scanner that silently visited nothing — a wrong root, `git ls-files` failing
/// soft, an extension list that excluded everything — passes in **exactly the
/// same way**. That is the shape C138 found in C132's outline assertion this
/// same week: a test whose subject is an absence cannot tell "nothing is wrong"
/// from "nothing was examined".
#[test]
fn the_scan_reads_the_tree_and_the_matcher_matches() {
    let files = scan().files;
    assert!(
        files > 200,
        "the scan visited only {files} files, which is too few for this tree — so a\n\
         clean result means nothing. Check root(), the git ls-files call, and TEXT."
    );

    let (mangled, intended, name) = DOUBLE_ENCODED[0];
    let raw: Vec<u8> = b"**C146 "
        .iter()
        .copied()
        .chain(mangled.chars().map(|c| c as u8))
        .chain(b" a heading.**".iter().copied())
        .collect();
    let viewed: String = raw.iter().map(|&b| b as char).collect();
    assert!(
        viewed.contains(mangled),
        "the {name} pattern (for '{intended}') does not match a line built from its own\n\
         bytes, so the escapes in DOUBLE_ENCODED and the latin-1 view in scan()\n\
         disagree and the check is looking for something that cannot occur."
    );

    assert!(
        EXEMPT.iter().any(|e| "crates/l2-testkit/tests/mojibake.rs".ends_with(e)),
        "this file must be exempt, or the check reports itself forever"
    );
}
