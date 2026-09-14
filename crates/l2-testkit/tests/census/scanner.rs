#![allow(unused_imports)]
use super::*;
use super::assertions::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The total the inventory adds up to, stated separately so that a change
/// which moves a test between two files still has to be acknowledged as a
/// change in how much of this suite exists on CI.

/// The workspace root, from this crate's manifest.
pub(crate) fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_path_buf()
}

/// Every `.rs` under `crates/`, excluding this crate.
pub(super) fn source_files(root: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
        paths.sort();
        for p in paths {
            if p.is_dir() {
                if p.file_name().is_some_and(|n| n == "target") {
                    continue;
                }
                walk(&p, out);
            } else if p.extension().is_some_and(|e| e == "rs") {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(&root.join("crates"), &mut out);
    out.retain(|p| !p.components().any(|c| c.as_os_str() == "l2-testkit"));
    out
}

pub(super) fn relative(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).to_string_lossy().replace('\\', "/")
}

/// The gate a chunk of source sits behind, if any, counting only what the
/// chunk itself names.
fn direct_gate(body: &str) -> Option<Gate> {
    NEEDLES.iter().find(|(needle, _)| body.contains(needle)).map(|&(_, g)| g)
}

/// Split a file into its `#[test]` functions. Everything before the first is
/// the preamble — helpers and macro definitions — and is not a test.
fn test_bodies(src: &str) -> Vec<&str> {
    src.split("#[test]").skip(1).collect()
}

/// **One level of indirection has to be followed**, or the census undercounts
/// badly: most gated tests call a file-local `macro_rules!` or helper that
/// holds the gate
///
/// So the preamble is chopped into named items and each is given the gate its
/// own text names; a test naming such an item inherits it.
fn local_items(preamble: &str) -> Vec<(String, Gate)> {
    let mut marks: Vec<(usize, String)> = Vec::new();
    for (pat, skip) in
        [("macro_rules! ", 14usize), ("\nfn ", 4), ("\npub fn ", 8), ("\n    fn ", 8)]
    {
        let mut from = 0usize;
        while let Some(rel) = preamble[from..].find(pat) {
            let at = from + rel;
            let name: String = preamble[at + skip..]
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() {
                marks.push((at, name));
            }
            from = at + pat.len();
        }
    }
    marks.sort();
    let mut out = Vec::new();
    for (i, (at, name)) in marks.iter().enumerate() {
        let end = marks.get(i + 1).map(|(a, _)| *a).unwrap_or(preamble.len());
        if let Some(g) = direct_gate(&preamble[*at..end]) {
            out.push((name.clone(), g));
        }
    }
    out
}

fn gate_of(body: &str, items: &[(String, Gate)]) -> Option<Gate> {
    let mut best = direct_gate(body);
    for (name, gate) in items {
        if body.contains(&format!("{name}!(")) || body.contains(&format!("{name}(")) {
            best = Some(match best {
                Some(b) => b.min(*gate),
                None => *gate,
            });
        }
    }
    best
}

pub(crate) fn scan() -> BTreeMap<(String, &'static str), usize> {
    let root = repo_root();
    let mut found: BTreeMap<(String, &'static str), usize> = BTreeMap::new();
    for path in source_files(&root) {
        let Ok(src) = std::fs::read_to_string(&path) else { continue };
        let mut preamble = src.split("#[test]").next().unwrap_or("").to_string();
        // **A test file's helpers can live in a sibling module.** `mod common;`
        // in a `tests/` directory pulls in `common/mod.rs`, which is where
        // `tests/screens_*.rs` keep the `world!()` that carries their gate. Read
        // it into the preamble, or the census loses every test behind it.
        // **A split target's part reads its root's macros through `use super::*`.**
        // Every `main.rs` or `mod.rs` between the file and `tests/` joins the preamble,
        // and each of those roots names sibling modules of its own.
        let mut roots: Vec<(PathBuf, String)> = Vec::new();
        let mut dir = path.parent();
        while let Some(d) = dir {
            if d.file_name().is_some_and(|n| n == "tests") || !relative(&root, d).contains("/tests") { break; }
            for n in ["main.rs", "mod.rs"] {
                let cand = d.join(n);
                if cand != path { if let Ok(t) = std::fs::read_to_string(&cand) { roots.push((d.to_path_buf(), t)); } }
            }
            dir = d.parent();
        }
        // A `#[path = "..."]` above the declaration names the file outright.
        let mods_of = |text: &str| -> Vec<String> {
            let mut out = Vec::new();
            let mut at: Option<String> = None;
            for l in text.lines() {
                let l = l.trim();
                if let Some(rest) = l.strip_prefix("#[path = \"") { at = rest.split('"').next().map(str::to_string); continue; }
                if let Some(name) = l.strip_prefix("mod ").and_then(|r| r.strip_suffix(';')) {
                    match at.take() { Some(p) => out.push(p), None => { out.push(format!("{name}/mod.rs")); out.push(format!("{name}.rs")); } }
                } else if !l.starts_with("#[") { at = None; }
            }
            out
        };
        let mut wanted: Vec<PathBuf> = mods_of(&preamble).into_iter().map(|m| path.parent().unwrap_or(&root).join(m)).collect();
        for (dir, text) in roots {
            wanted.extend(mods_of(&text).into_iter().map(|m| dir.join(m)));
            preamble.push('\n');
            preamble.push_str(&text);
        }
        for cand in wanted {
            if let Ok(extra) = std::fs::read_to_string(&cand) {
                preamble.push('\n');
                preamble.push_str(&extra);
            }
        }
        // The gate can now sit entirely in the sibling module, so the cheap
        // "does this file mention the testkit at all" filter runs on both.
        if !src.contains("l2_testkit") && !preamble.contains("l2_testkit") {
            continue;
        }
        let items = local_items(&preamble);
        for body in test_bodies(&src) {
            if let Some(gate) = gate_of(body, &items) {
                *found.entry((relative(&root, &path), gate.name())).or_insert(0) += 1;
            }
        }
    }
    found
}

