//! **No double-encoded text reaches `main`.**
//!
//! This exists because a correction's heading did. An agent wrote
//! `**C146 — The differential's first finding…**` and what landed in
//! `docs/decisions.md` was `c3 a2 e2 82 ac e2 80 9d` where the em-dash belonged
//! — UTF-8 read as CP1252 and encoded to UTF-8 a second time. Every other
//! heading in that file carries the plain `e2 80 94`.
//!
//! # Why nothing caught it, which is the part worth keeping
//!
//! `crates/l2-testkit/tests/encoding.rs` sounds like the check for this and is
//! not: it asserts that every field of an encodable struct survives a round
//! trip through `Canonical`, which is about the *simulation's* bytes and says
//! nothing about the bytes in a document. **A test whose name reads like the
//! check you want is worse than no test, because a reader stops looking.**
//!
//! Nor could the rest of the suite see it. `cargo test` compiles no Markdown.
//! `figures.js` only visits its own markers. `symbols_md.js` compares a
//! generated file against its source and would have propagated the corruption
//! faithfully into both. The census reads paths, not contents.
//!
//! It surfaced only because the mangled bytes happened to sit in a heading that
//! `tools/decisions/corrections.js` parses, so the correction went missing and
//! three citations pointed at nothing. **Had the same corruption landed one line
//! lower, in the prose, it would have shipped and stayed.** That is the argument
//! for the check rather than for care: this one was caught by luck, and luck
//! does not scale to the next document.
//!
//! # What it looks for
//!
//! The mojibake of a UTF-8 sequence is itself a fixed UTF-8 sequence, so this is
//! a literal search and not a heuristic. The listed pairs are the ones whose
//! originals this tree actually uses — em-dash, en-dash, the curly quotes, the
//! ellipsis, the non-breaking space and the multiplication sign. Each entry
//! carries the correct character, so the failure message can say what the text
//! was meant to be rather than only that something is wrong.
//!
//! A file may opt out by naming itself below, and exactly one does: this one.
//! **The exemption is belt-and-braces rather than load-bearing**, and saying so
//! matters because the obvious reason for it is wrong: the patterns are written
//! as `\u{..}` escapes, so this file does *not* contain them as bytes and would
//! pass the scan unexempted. The first draft of the second test assumed the
//! opposite, asserted the bytes were here, and failed on its first run. The
//! exemption stays in case somebody later pastes a real example into a comment.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/l2-testkit/../..")
        .to_path_buf()
}

/// Extensions worth scanning. Everything else is binary or generated.
///
/// Deliberately the same list as `conflict_markers.rs`. Two text-hygiene checks
/// disagreeing about which files are text is a gap shaped exactly like the one
/// this file was written for.
const TEXT: &[&str] = &[
    "rs", "md", "json", "toml", "js", "html", "java", "ps1", "yml", "yaml", "txt", "lock",
];

/// `(mangled, intended, name)`.
///
/// The mangled form is what you get by decoding correct UTF-8 as CP1252 and
/// re-encoding it. Written as `\u{..}` escapes rather than pasted, so that this
/// file cannot itself be corrupted into agreeing with the corruption.
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

/// Files allowed to contain the patterns. Exactly one, and it is this file.
const EXEMPT: &[&str] = &["tests/mojibake.rs"];

struct Hit {
    file: String,
    line: usize,
    name: &'static str,
    intended: char,
    text: String,
}

/// The scan's result, and **how much it looked at**.
///
/// `files` is carried out of here rather than discarded because a scanner that
/// silently visited nothing reports a clean tree, and a clean tree is what this
/// check reports when all is well. The count is the difference between the two.
struct Scan {
    hits: Vec<Hit>,
    files: usize,
}

fn scan() -> Scan {
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
        // Read as bytes and view them as latin-1, so that the mangled sequence
        // is matched as the byte run it is rather than as whatever it decodes
        // to. `read_to_string` would succeed here — double-encoded UTF-8 is
        // still valid UTF-8, which is the whole reason this is invisible.
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

/// Both halves of the check are live: it reads the tree, and it matches.
///
/// `no_double_encoded_text_is_committed` passes when the tree is clean, and a
/// scanner that silently visited nothing — a wrong root, `git ls-files` failing
/// soft, an extension list that excluded everything — passes in **exactly the
/// same way**. That is the shape C138 found in C132's outline assertion this
/// same week: a test whose subject is an absence cannot tell "nothing is wrong"
/// from "nothing was examined".
///
/// So this asserts the two things that separate them.
///
/// It does **not** look for the patterns inside this file. They are written as
/// `\u{..}` escapes precisely so that this file cannot contain them as bytes —
/// the first draft of this test asserted it did, and failed, which is the check
/// catching its own author.
#[test]
fn the_scan_reads_the_tree_and_the_matcher_matches() {
    // Half one: it looked at something. The tree has hundreds of tracked text
    // files; a bound this loose still separates "clean" from "empty".
    let files = scan().files;
    assert!(
        files > 200,
        "the scan visited only {files} files, which is too few for this tree — so a\n\
         clean result means nothing. Check root(), the git ls-files call, and TEXT."
    );

    // Half two: the matcher finds a mangled run in bytes shaped like a real
    // file's. Built from raw bytes and viewed the way scan() views a file, so
    // this exercises the same path rather than a paraphrase of it.
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
