#![allow(unused_imports)]
use super::*;
use super::verification::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The body of the block that opens on the line matching `head`, by counting
/// braces. Crude, and adequate: this source has no braces inside string
/// literals in the shapes we scan.
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

/// Every `impl Encode for T` / `impl Decode for T` in the workspace, as
/// `T -> (encode body, decode body)`. `T` is reduced to its last path segment,
/// because `crate::unit::Unit` and `Unit` are the same type seen from two files.
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
                // Skip generic impls and anything that is not a plain path.
                if ty.is_empty() || ty.contains('<') || ty.contains('&') {
                    continue;
                }
                let short = ty.rsplit("::").next().unwrap_or(ty).to_string();
                let Some(body) = block_after(&src, at) else { continue };
                // The impl block holds one fn; take the whole block, which also
                // picks up any helper the impl defines beside it.
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

)];

/// Add [`FREE_FUNCTION_CODECS`] to what `codec_bodies` found.
fn free_function_codecs(out: &mut BTreeMap<String, Codec>) {
    let files = rust_files();
    for (ty, krate, enc_heads, dec_heads) in FREE_FUNCTION_CODECS {
        let mut enc = String::new();
        let mut dec = String::new();
        for path in &files {
            if crate_of(path) != *krate {
                continue;
            }
            let Ok(src) = std::fs::read_to_string(path) else { continue };
            for (heads, into) in [(enc_heads, &mut enc), (dec_heads, &mut dec)] {
                for head in heads.iter() {
                    let Some(at) = src.find(head) else { continue };
                    let Some(body) = block_after(&src, at) else { continue };
                    into.push('\n');
                    into.push_str(body);
                }
            }
        }
        assert!(
            !enc.is_empty() && !dec.is_empty(),
            "{ty}'s free-function codec did not resolve — the heads in \
             FREE_FUNCTION_CODECS have been renamed, and a check that silently \
             stops checking is worse than no check"
        );
        let e = out.entry(ty.to_string()).or_default();
        e.krate = krate.to_string();
        e.encode = Some(enc);
        e.decode = Some(dec);
    }
}

