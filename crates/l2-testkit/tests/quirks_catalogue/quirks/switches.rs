#![allow(unused_imports)]
use super::*;
use super::dispositions::*;
use super::super::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use Disposition::{Retracted, Switchable, Unswitchable, Unwired};
use Home::{Behavioural, Presentation};

#[test]
fn every_switch_is_read_by_the_simulation() {
    let root = repo_root();
    let variants = quirk_variants(&root);
    assert!(!variants.is_empty(), "no Quirk variants parsed at all");
    let files = sources(&root);

    let mut inert = Vec::new();
    for (variant, entry) in &variants {
        let needle = format!("Quirk::{variant}");
        let read_by: Vec<&str> = files
            .iter()
            .filter(|(path, text)| {
                path != "crates/l2-net/src/quirks/mod.rs"
                    && !path.contains("/tests/")
                    && text.contains(&needle)
                    && text.contains("reproduces(")
            })
            .map(|(path, _)| path.as_str())
            .collect();
        if read_by.is_empty() {
            inert.push(format!(
                "  Quirk::{variant} ({entry}) is named by no simulation file that calls \
                 `reproduces(`. It is a checkbox with nothing behind it."
            ));
        }
    }
    assert!(
        inert.is_empty(),
        "{} quirk(s) are inert:\n{}\n\nEither wire the switch or take the variant out and mark \
         the entry Unwired in crates/l2-testkit/tests/quirks_catalogue/main.rs. Do not ship a \
         checkbox that does nothing.",
        inert.len(),
        inert.join("\n")
    );
}

/// added — and would frame a quirk as a *rule*, which it is not. It belongs on
/// `Options`, which is in the save *body* and therefore already inside the
/// per-tick lockstep digest. `docs/bugs.md` §6.3, `docs/decisions.md` C62.
#[test]
fn no_quirk_is_filed_under_tables_where_it_would_reach_the_save_header() {
    let root = repo_root();
    let tables = read(&root, "crates/l2-kingdom/src/tables/mod.rs");
    for needle in ["Quirk", "quirks"] {
        assert!(
            !tables.contains(needle),
            "crates/l2-kingdom/src/tables/mod.rs names `{needle}`. A quirk on `Tables` is hashed \
             into the save header (`ruleset_fingerprint`), so adding one invalidates every \
             existing save — and it frames a quirk as a rule. Put it on \
             `l2_kingdom::kingdom::Options::quirks`, which is in the save body and in the \
             per-tick digest. docs/bugs.md §6.3, docs/decisions.md C62."
        );
    }
    let kingdom = read(&root, "crates/l2-kingdom/src/kingdom/mod.rs");
    assert!(
        kingdom.contains("pub quirks: l2_net::Quirks"),
        "l2_kingdom::kingdom::Options::quirks has moved. It is where the quirk set lives; \
         docs/decisions.md C62."
    );
    let save = read(&root, "crates/l2-kingdom/src/save/mod.rs");
    assert!(
        save.contains("out.encode(&self.options.quirks)"),
        "the quirk set is not encoded into the save body any more. That body IS the per-tick \
         lockstep digest (`save::checksum` is `Canonical::hash_of(kingdom)`), so dropping it \
         makes two peers with different quirks agree on a checksum while computing different \
         games. docs/netcode.md §6."
    );
}

#[test]
fn the_switchable_count_is_reported() {
    let root = repo_root();
    let catalogue = catalogue(&root);
    let inventory: BTreeMap<&str, Disposition> = DISPOSITIONS.iter().copied().collect();

    let count = |f: fn(&Disposition) -> bool| {
        catalogue.iter().filter(|id| inventory.get(id.as_str()).is_some_and(f)).count()
    };
    let behavioural = count(|d| matches!(d, Switchable(Behavioural)));
    let presentation = count(|d| matches!(d, Switchable(Presentation)));
    let switchable = behavioural + presentation;
    let unwired = count(|d| matches!(d, Unwired(_)));
    let unswitchable = count(|d| matches!(d, Unswitchable(_)));
    let retracted = count(|d| matches!(d, Retracted));

    eprintln!(
        "quirk catalogue: {} entries in docs/bugs.md §2 — {switchable} switchable \
         ({behavioural} behavioural, {presentation} presentation), {unwired} unwired, \
         {unswitchable} unswitchable, {retracted} retracted",
        catalogue.len()
    );
    assert_eq!(
        switchable + unwired + unswitchable + retracted,
        catalogue.len(),
        "an entry fell out of every bucket"
    );
    assert_eq!(
        behavioural,
        quirk_variants(&root).len(),
        "Switchable(Behavioural) rows and l2_net::Quirk variants must be the same number"
    );
    assert_eq!(
        presentation,
        presentation_rows(&root),
        "Switchable(Presentation) rows and l2_game::game::PRESENTATION rows must be the same number"
    );
}


