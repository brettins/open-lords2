//! **Every field of a save-crossing struct must appear in both `encode` and
//! `decode`.**
//!
//! # Why this is a source-text check and not a runtime one
//!
//! `docs/decisions.md` C65: the lockstep digest is `value.encode(&mut c)` and
//! nothing else, so **a field absent from the encoder is absent from every
//! digest, on every peer, identically.** Two timelines that have both lost a
//! field agree perfectly. Measured, not argued: dropping `Industry::has_resource`
//! from the encoder leaves `ten_seasons_from_a_reloaded_game_are_the_same_ten`
//! passing after ten seasons of a different world.
//!
//! The runtime check that *does* work is `assert_eq!(back, game)`, and the
//! reason it works is worth copying: its `PartialEq` is
//! **derived from the field list** while the encoder is **hand written**. Two
//! independently maintained lists that must agree. That is the shape of every
//! check on this project that has ever caught anything — `symbols_md.js`,
//! `figures.js`, the citation lockfile, the test census beside this file.
//!
//! Its hole is that it compares **one fixture**. A field added to the struct and
//! not to the encoder is caught only if `a_game()` sets it to something a
//! defaulted decode would not produce; leave it at `Default` and both sides are
//! equal and nothing fires. This file needs no fixture, so it does not have that
//! hole.
//!
//! # What it does not cover, stated because an unstated limit gets trusted past
//!
//! **1. It cannot tell you a field is encoded *correctly*.** `out.u8(self.a)`
//! twice and never `self.b` passes here and is wrong. This proves nothing was
//! *forgotten*, which is the failure that has now happened six times, and
//! nothing else.
//!
//! **2. It does not cover the importer, which is where both live instances
//! are.** `County::farm_style` (C62) and `Unit::mission` were not
//! dropped by `l2-kingdom`'s encoder — they were dropped by `l2-scenario`
//! building a `County` from a `.sav`, and that path never touches `encode`.
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
//!
//! # The escape hatch is deliberate, explicit and countable
//!
//! A field outside the encoding carries `not-encoded:` and a reason in
//! its doc comment. A field that crosses the codec through a *constructor*
//! carries `codec-via:`, because
//! this check matches names and cannot see one. Both are excuses, both are
//! counted, and neither is a silence. That is the same principle as an invention being a countable
//! status: a decision nothing counts is a decision nobody
//! revisits.

mod parser;
pub use parser::*;
mod verification;
pub use verification::*;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One type's codec, and the crate it was declared in.
#[derive(Default)]
struct Codec {
    encode: Option<String>,
    decode: Option<String>,
    krate: String,
}

/// `crates/<name>/…` — the crate a file belongs to.
fn crate_of(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    s.split("crates/").nth(1).and_then(|r| r.split('/').next()).unwrap_or("").to_string()
}

/// The workspace root, from this crate's manifest.
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
                // `target` holds generated copies of everything and would
                // double every count in here.
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

/// **Codecs that are a pair of free functions**, named
/// one by one.
///
/// `Game` is the whole of the list today and it is the reason the list exists:
/// `l2_game::save` writes a saved game with `encode`/`encode_prefix` and reads
/// it with `decode`/`decode_prefix`, and **`impl Encode for Game` does not
/// exist**, so until now the type at the top of every saved file was the one
/// type this check made no claim about at all. It was found by adding a field
/// to `Game` and watching the check stay green.
///
/// Both halves of each pair are concatenated, because the field list is split
/// across them: `kingdom` is named in the outer function and everything else in
/// the prefix.
///
/// **This is the shape to copy if another such codec appears.** A free-function
/// the prefix is not a `Canonical`
/// value — it is just invisible to a scanner that looks for `impl Encode`, and
/// the cost of that invisibility is `docs/decisions.md` C30's whole family.
const FREE_FUNCTION_CODECS: &[(&str, &str, &[&str], &[&str])] = &[(
    "Game",
    "l2-game",
    &["fn encode(game: &Game)", "fn encode_prefix("],
    &["fn decode(", "fn decode_prefix("],
