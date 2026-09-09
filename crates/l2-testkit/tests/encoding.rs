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
//! passing after ten seasons of a genuinely different world.
//!
//! The runtime check that *does* work is `assert_eq!(back, game)`, and the
//! reason it works is worth copying rather than trusting: its `PartialEq` is
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
//! actually are.** `County::farm_style` (C62) and `Unit::mission` were not
//! dropped by `l2-kingdom`'s encoder — they were dropped by `l2-scenario`
//! building a `County` from a `.sav`, and that path never touches `encode`.
//! This check would not have caught either. The fix there is a different one and
//! it is not a lint: **the importer assigns field by field onto a default**
//! (`c.farm_style = s.farm_style;`), and a struct literal with no `..` would
//! have made every one of those omissions a *compile error*. See the failing
//! note in `docs/decisions.md` C65.
//!
//! So: **this narrows the half of the boundary it can see, and the half it cannot
//! see is the half that has actually bitten us twice.** It does not close the
//! hole. Stated here in the file rather than only in the correction, because a
//! check whose limits are unstated gets trusted past them — and this is now the
//! third artefact on the project people will reach for when asking *"is this
//! field covered?"*, after a round trip that compares one fixture and a digest
//! that is structurally incapable of answering (`docs/decisions.md` C65).
//!
//! # The escape hatch is deliberate, explicit and countable
//!
//! A field genuinely outside the encoding carries `not-encoded:` and a reason in
//! its doc comment. A field that crosses the codec through a *constructor*
//! rather than by name — `Quirks::from_bits` — carries `codec-via:`, because
//! this check matches names and cannot see one. Both are excuses, both are
//! counted, and neither is a silence. That is the same principle as an invention being a countable
//! status rather than an absence: a decision nothing counts is a decision nobody
//! revisits.

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

/// `struct T { … }`'s field names, plus the ones excused by `not-encoded:`.
fn struct_fields(name: &str, krate: &str) -> Option<(Vec<String>, Vec<String>)> {
    // **Resolve inside the codec's own crate first.** `l2_kingdom::unit::Unit`
    // and `l2_formats::save::Unit` are different types with the same short
    // name, and taking whichever the directory walk reached first reported
    // twelve fields of the raw `.sav` record as missing from a codec that has
    // never seen them. A scanner that resolves a name wrongly produces a clean,
    // plausible, wrong answer, which is this project's commonest tool failure.
    // **Required, not preferred.** Falling back to the workspace resolved
    // `l2-net`'s test-fixture `Order` to `l2_kingdom::trade::Order` and reported
    // six fields of a trade order as missing from a network codec that has never
    // heard of it. A name that does not resolve in its own crate is a name this
    // check cannot verify, and saying so is better than a plausible answer about
    // the wrong type — the failure mode `docs/agents.md` catalogues five of.
    let files: Vec<PathBuf> = rust_files().into_iter().filter(|p| crate_of(p) == krate).collect();
    for path in files {
        let Ok(src) = std::fs::read_to_string(&path) else { continue };
        for head in [format!("pub struct {name} {{"), format!("struct {name} {{")] {
            let Some(at) = src.find(&head) else { continue };
            // `struct Foo {` must not match `struct FooBar {`; find() on the
            // brace-terminated form already prevents that.
            let Some(body) = block_after(&src, at) else { continue };
            let mut fields = Vec::new();
            let mut excused = Vec::new();
            let mut depth = 0usize;
            let mut pending_excuse = false;
            for line in body.lines() {
                let t = line.trim();
                // Two markers, and they mean different things. `not-encoded:`
                // is a field outside the codec. `codec-via:` is a field that
                // crosses it through a constructor rather than by name —
                // `Quirks::from_bits` is the first — which this check cannot
                // see, because it matches names. Both are excuses and both are
                // counted; neither is a silence.
                if t.contains("not-encoded:") || t.contains("codec-via:") {
                    pending_excuse = true;
                }
                depth += t.matches('{').count();
                depth = depth.saturating_sub(t.matches('}').count());
                if depth > 0 || t.starts_with("//") || t.starts_with('#') {
                    continue;
                }
                let Some((lhs, _)) = t.split_once(':') else { continue };
                let lhs = lhs.trim().trim_start_matches("pub ").trim();
                if lhs.is_empty()
                    || !lhs.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                    || lhs.chars().next().is_some_and(|c| c.is_ascii_uppercase())
                {
                    continue;
                }
                if pending_excuse {
                    excused.push(lhs.to_string());
                } else {
                    fields.push(lhs.to_string());
                }
                pending_excuse = false;
            }
            if !fields.is_empty() || !excused.is_empty() {
                return Some((fields, excused));
            }
        }
    }
    None
}

fn mentions(body: &str, field: &str) -> bool {
    let bytes = body.as_bytes();
    let mut from = 0;
    while let Some(rel) = body[from..].find(field) {
        let at = from + rel;
        from = at + field.len();
        let before_ok = at == 0 || !is_word(bytes[at - 1]);
        let after = at + field.len();
        let after_ok = after >= bytes.len() || !is_word(bytes[after]);
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// **Types with a codec that this check makes no claim about**, each read and
/// classified rather than merely tolerated:
///
/// * `Fixed` — `pub struct Fixed(i32)`, a tuple struct. It has no named fields,
///   so there is no list to compare; its single value is checked by
///   `l2-net`'s own round trip.
/// * `Message`, `Mismatch` — enums. A variant list is a different contract from
///   a field list and needs a different check; the tag-and-payload round trip in
///   `l2-net/tests/canonical.rs` is what covers them today.
/// * `Order` — an enum, and a fixture in `l2-net/tests/common`. Not shipped.
///
/// **A growing list is a signal, which is why it is asserted rather than
/// counted.** Every entry is a field list nothing is checking.
const UNVERIFIABLE: &[&str] = &[
    "Fixed (l2-net)",
    "Message (l2-net)",
    "Mismatch (l2-net)",
    "Order (l2-net)",
];

/// **The check.** Every field of every type with an `Encode`/`Decode` pair must
/// be named in both.
#[test]
fn every_field_of_an_encodable_struct_is_encoded_and_decoded() {
    let bodies = codec_bodies();
    assert!(bodies.len() >= 15, "found only {} codec impls; the scanner is broken", bodies.len());

    let mut missing: Vec<String> = Vec::new();
    let mut checked = 0usize;
    let mut excused_total = 0usize;
    let mut unverifiable: Vec<String> = Vec::new();

    for (ty, c) in &bodies {
        let (enc, dec) = (&c.encode, &c.decode);
        // A type with only one half is an enum wire format or a hand-rolled
        // pair; this check is about structs whose field list is the contract.
        let (Some(enc), Some(dec)) = (enc, dec) else { continue };
        let Some((fields, excused)) = struct_fields(ty, &c.krate) else {
            unverifiable.push(format!("{ty} ({})", c.krate));
            continue;
        };
        excused_total += excused.len();
        for f in &fields {
            checked += 1;
            let in_enc = mentions(enc, f);
            let in_dec = mentions(dec, f);
            if !in_enc || !in_dec {
                let where_ = match (in_enc, in_dec) {
                    (false, false) => "neither encode nor decode",
                    (false, true) => "encode",
                    _ => "decode",
                };
                missing.push(format!("  {ty}.{f} — not named in {where_}"));
            }
        }
    }

    assert!(checked > 100, "only {checked} fields checked; the scanner is broken");

    // **Unverifiable is a verdict, not a silence, and it is written down so that
    // a growing list goes red.** These are types with a codec whose struct does
    // not resolve in the codec own crate: enums with a hand-rolled wire form,
    // and fixtures that live in a test module. Each is a type this check makes
    // NO claim about, which is worth exactly as much as knowing which ones it
    // does.
    unverifiable.sort();
    assert_eq!(
        unverifiable, UNVERIFIABLE,
        "the set of types this check cannot verify has changed.

A type here \n         is one whose codec exists but whose struct does not resolve in the same \n         crate - an enum, or a test fixture. If a new one appeared, either it is \n         a struct that should resolve (and the scanner is wrong), or it is a \n         genuine enum (and it belongs in the list). Do not let the list grow \n         without reading each addition: an unverifiable type is a field list \n         nothing is checking."
    );
    assert!(
        missing.is_empty(),
        "{} field(s) of a save-crossing struct are not named in both halves of \
         the codec.\n\n{}\n\nA field the encoder never writes is invisible to the \
         lockstep digest as well, because the digest IS the encoder \
         (docs/decisions.md C65) — both peers hash the same smaller thing and \
         agree. Either encode and decode it, or put `not-encoded: <reason>` in \
         its doc comment, which is a decision this check will then count \
         ({excused_total} so far) rather than a silence nobody revisits.",
        missing.len(),
        missing.join("\n"),
    );
}
