//! `docs/decisions.md` C65: the lockstep digest is `value.encode(&mut c)` and
//! nothing else, so **a field absent from the encoder is absent from every
//! digest, on every peer, identically.** Two timelines that have both lost a
//! field agree perfectly. Measured, not argued: dropping `Industry::has_resource`
//! from the encoder leaves `ten_seasons_from_a_reloaded_game_are_the_same_ten`
//! passing after ten seasons of a different world.
//!
//! **2. It does not cover the importer, which is where both live instances
//! are.** `County::farm_style` (C62) and `Unit::mission` were not
//! dropped by `l2-kingdom`'s encoder — they were dropped by `l2-scenario`
//! building a `County` from a `.sav`, and that path never touches `encode`.
//!
//! This check would not have caught either. The fix there is a different one and
//! **the importer assigns field by field onto a default**
//! (`c.farm_style = s.farm_style;`), and a struct literal with no `..` would
//! have made every one of those omissions a *compile error*. See the failing
//! note in `docs/decisions.md` C65.
//!
//! So: **this narrows the half of the boundary it can see, and the half it cannot
//! see is the half that has bitten us twice.** It does not close the
//! hole. Stated here in the file, because a
//! check whose limits are unstated gets trusted past them — and this is now the
//! third artefact on the project people will reach for when asking *"is this
//! field covered?"*, after a round trip that compares one fixture and a digest
//! that is structurally incapable of answering (`docs/decisions.md` C65).

mod free_function_codecs_part;
pub use free_function_codecs_part::*;
mod struct_analysis;
pub use struct_analysis::*;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Default)]
struct Codec {
    encode: Option<String>,
    decode: Option<String>,
    krate: String,
}

fn crate_of(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    s.split("crates/").nth(1).and_then(|r| r.split('/').next()).unwrap_or("").to_string()
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/l2-testkit/../..")
        .to_path_buf()
}

fn rust_files() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root().join("crates")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                if p.file_name().is_some_and(|n| n == "target") {
                    continue;
                }
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

fn block_after(src: &str, head_at: usize) -> Option<&str> {
    let open = src[head_at..].find('{')? + head_at;
    let mut depth = 0usize;
    for (i, ch) in src[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&src[open + 1..open + i]);
                }
            }
            _ => {}
        }
    }
    None
}

fn codec_bodies() -> BTreeMap<String, Codec> {
    let mut out: BTreeMap<String, Codec> = BTreeMap::new();
    for path in rust_files() {
        let Ok(src) = std::fs::read_to_string(&path) else { continue };
        for (trait_name, slot) in [("Encode", 0usize), ("Decode", 1usize)] {
            let needle = format!("impl {trait_name} for ");
            let mut from = 0;
            while let Some(rel) = src[from..].find(&needle) {
                let at = from + rel;
                from = at + needle.len();
                let rest = &src[from..];
                let end = rest.find(" {").unwrap_or(0);
                let ty = rest[..end].trim();
                if ty.is_empty() || ty.contains('<') || ty.contains('&') {
                    continue;
                }
                let short = ty.rsplit("::").next().unwrap_or(ty).to_string();
                let Some(body) = block_after(&src, at) else { continue };
                let e = out.entry(short).or_default();
                e.krate = crate_of(&path);
                let target = if slot == 0 { &mut e.encode } else { &mut e.decode };
                let mut s = target.take().unwrap_or_default();
                s.push('\n');
                s.push_str(body);
                *target = Some(s);
            }
        }
    }
    out
}

