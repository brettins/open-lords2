#![allow(unused_imports)]
use super::*;
use super::free_function_codecs_part::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn struct_fields(name: &str, krate: &str) -> Option<(Vec<String>, Vec<String>)> {
    let files: Vec<PathBuf> = rust_files().into_iter().filter(|p| crate_of(p) == krate).collect();
    for path in files {
        let Ok(src) = std::fs::read_to_string(&path) else { continue };
        for head in [format!("pub struct {name} {{"), format!("struct {name} {{")] {
            let Some(at) = src.find(&head) else { continue };
            let Some(body) = block_after(&src, at) else { continue };
            let mut fields = Vec::new();
            let mut excused = Vec::new();
            let mut depth = 0usize;
            let mut pending_excuse = false;
            for line in body.lines() {
                let t = line.trim();
                if t.contains("not-encoded:") || t.contains("codec-via:") {
                    pending_excuse = true;
                }
                depth += t.matches('{').count();
                depth = depth.saturating_sub(t.matches('}').count());
                if depth > 0 || t.starts_with("//") || t.starts_with('#') {
                    continue;
                }
                let Some((lhs, _)) = t.split_once(':') else { continue };
                let lhs = lhs.trim();
                let lhs = lhs
                    .strip_prefix("pub(crate) ")
                    .or_else(|| lhs.strip_prefix("pub(super) "))
                    .or_else(|| lhs.strip_prefix("pub "))
                    .unwrap_or(lhs)
                    .trim();
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

/// [`mentions`] matches text, and until this existed the text it matched
/// included the prose. The check was ablated by deleting
/// the loop that encodes `Game::player_names`, and it **stayed green**, because
/// the comment above the deleted loop still said the words `player_names`. The
/// better a field is documented at its encoder, the less this check was able to
/// say about it — which is exactly backwards, and is `docs/agents.md`'s *"a
/// check that passes for an accidental reason is indistinguishable from one
/// that passes for the right reason"* with the accident being good writing.
fn without_comments(body: &str) -> String {
    body.lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
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

const UNVERIFIABLE: &[&str] = &[
    "Fixed (l2-net)",
    "Message (l2-net)",
    "Mismatch (l2-net)",
    "Order (l2-net)",
];

#[test]
fn every_field_of_an_encodable_struct_is_encoded_and_decoded() {
    let mut bodies = codec_bodies();
    assert!(bodies.len() >= 15, "found only {} codec impls; the scanner is broken", bodies.len());
    free_function_codecs(&mut bodies);

    let mut missing: Vec<String> = Vec::new();
    let mut checked = 0usize;
    let mut excused_total = 0usize;
    let mut unverifiable: Vec<String> = Vec::new();

    for (ty, c) in &bodies {
        let (enc, dec) = (&c.encode, &c.decode);
        let (Some(enc), Some(dec)) = (enc, dec) else { continue };
        let (enc, dec) = (&without_comments(enc), &without_comments(dec));
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
    // NO claim about.
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

