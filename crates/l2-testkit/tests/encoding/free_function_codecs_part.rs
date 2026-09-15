#![allow(unused_imports)]
use super::*;
use super::struct_analysis::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// **This is the shape to copy if another such codec appears.** A free-function
/// the prefix is not a `Canonical`
/// value — it is just invisible to a scanner that looks for `impl Encode`, and
/// the cost of that invisibility is `docs/decisions.md` C30's whole family.
const FREE_FUNCTION_CODECS: &[(&str, &str, &[&str], &[&str])] = &[(
    "Game",
    "l2-game",
    &["fn encode(game: &Game)", "fn encode_prefix("],
    &["fn decode(", "fn decode_prefix("],
)];

pub(super) fn free_function_codecs(out: &mut BTreeMap<String, Codec>) {
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

