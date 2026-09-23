#![allow(unused_imports)]
use super::*;
use super::switches::*;
use super::super::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use Disposition::{Retracted, Switchable, Unswitchable, Unwired};
use Home::{Behavioural, Presentation};

#[test]
fn every_bug_has_a_disposition_every_switch_has_a_bug_and_every_switch_is_in_its_own_home() {
    let root = repo_root();
    let catalogue = catalogue(&root);
    assert!(
        catalogue.len() > 50,
        "only {} entries found in docs/bugs.md §2 — the section's shape has changed and this \
         check is now reading nothing. Fix the parser, do not lower the bound.",
        catalogue.len()
    );

    let inventory: BTreeMap<&str, Disposition> = DISPOSITIONS.iter().copied().collect();
    assert_eq!(inventory.len(), DISPOSITIONS.len(), "an entry id appears twice in DISPOSITIONS");

    let variants = quirk_variants(&root);
    let behavioural: BTreeMap<&str, &str> =
        variants.iter().map(|(v, e)| (e.as_str(), v.as_str())).collect();
    let (rows, fields) = presentation(&root);
    let presentational: BTreeMap<&str, &str> =
        rows.iter().map(|(f, e)| (e.as_str(), f.as_str())).collect();

    let mut problems: Vec<String> = Vec::new();

    for entry in behavioural.keys() {
        if presentational.contains_key(entry) {
            problems.push(format!(
                "  {entry} is implemented in BOTH homes — Quirk::{} on Options and the field \
                 `{}` on Assets. One of them is wrong and the wrong one is probably the cheap \
                 one; docs/bugs.md §6.3a has the test.",
                behavioural[entry], presentational[entry]
            ));
        }
    }

    for id in &catalogue {
        let id = id.as_str();
        let in_b = behavioural.contains_key(id);
        let in_p = presentational.contains_key(id);
        match inventory.get(id) {
            None => problems.push(format!(
                "  docs/bugs.md has {id} and DISPOSITIONS does not. Say what its switch is, and \
                 which home it is in."
            )),
            Some(Switchable(Behavioural)) if !in_b => problems.push(format!(
                "  {id} is marked Switchable(Behavioural) and no l2_net::Quirk variant names it.{}",
                if in_p { " It is on Assets instead — that is the wrong home for a rule." } else { "" }
            )),
            Some(Switchable(Presentation)) if !in_p => problems.push(format!(
                "  {id} is marked Switchable(Presentation) and no row of \
                 l2_game::game::PRESENTATION names it.{}",
                if in_b {
                    " It is on Options instead — that costs a save bump and a handshake field \
                     for something that only changes pixels."
                } else {
                    ""
                }
            )),
            Some(d) if !matches!(d, Switchable(_)) && in_b => problems.push(format!(
                "  {id} has the switch Quirk::{} and is not marked Switchable.",
                behavioural[id]
            )),
            Some(d) if !matches!(d, Switchable(_)) && in_p => problems.push(format!(
                "  {id} has the presentation switch `{}` and is not marked Switchable.",
                presentational[id]
            )),
            _ => {}
        }
    }
    for (id, _) in DISPOSITIONS {
        if !catalogue.iter().any(|c| c == id) {
            problems.push(format!(
                "  DISPOSITIONS has {id} and docs/bugs.md §2 does not. A switch for a bug that \
                 is not in the catalogue is a switch nobody can look up."
            ));
        }
    }
    for (variant, entry) in &variants {
        if !catalogue.iter().any(|c| c == entry) {
            problems.push(format!(
                "  Quirk::{variant} claims docs/bugs.md entry {entry}, which is not in §2."
            ));
        }
    }
    for (field, entry) in &rows {
        if !fields.iter().any(|f| f == field) {
            problems.push(format!(
                "  PRESENTATION names `{field}`, which is not a field of l2_game::game::Quirks."
            ));
        }
        if !catalogue.iter().any(|c| c == entry) {
            problems.push(format!(
                "  PRESENTATION's `{field}` claims docs/bugs.md entry {entry}, which is not in §2."
            ));
        }
    }
    for field in &fields {
        if !rows.iter().any(|(f, _)| f == field) {
            problems.push(format!(
                "  l2_game::game::Quirks has the field `{field}` and PRESENTATION does not name \
                 it, so the quirks page cannot show it and nothing joins it to a catalogue entry."
            ));
        }
    }

    if !problems.is_empty() {
        let mut lines = String::new();
        for id in &catalogue {
            let id = id.as_str();
            let d = match (
                inventory.get(id),
                behavioural.contains_key(id),
                presentational.contains_key(id),
            ) {
                (_, true, _) => "Switchable(Behavioural)".to_string(),
                (_, _, true) => "Switchable(Presentation)".to_string(),
                (Some(Retracted), _, _) => "Retracted".to_string(),
                (Some(Unwired(w)), _, _) => format!("Unwired({w:?})"),
                (Some(Unswitchable(w)), _, _) => format!("Unswitchable({w:?})"),
                (Some(Switchable(_)), _, _) => "Unwired(\"SAY WHERE\")".to_string(),
                (None, _, _) => {
                    "Unwired(\"SAY WHERE, or Unswitchable(\\\"why\\\")\")".to_string()
                }
            };
            lines.push_str(&format!("    ({id:?}, {d}),\n"));
        }
        panic!(
            "the quirk catalogue and the switch lists disagree.\n\n{}\n\n\
             docs/bugs.md §2 is the catalogue; l2_net::Quirk is the behavioural switch list and \
             l2_game::game::PRESENTATION the presentation one. A hand-kept copy of any of them \
             beside another drifts. If the change is intended, replace DISPOSITIONS in \
             crates/l2-testkit/tests/quirks_catalogue/main.rs with:\n\n\
             const DISPOSITIONS: &[(&str, Disposition)] = &[\n{}];\n",
            problems.join("\n"),
            lines
        );
    }
}

/// **A placeholder row is a catalogue entry**,
/// does not say what its switch is goes red on its own branch.
#[test]
fn a_placeholder_row_is_an_entry_so_an_unwired_one_fails_on_its_own_branch() {
    let tag = |slug: &str| format!("{}NEW-{slug}", 'B');
    let doc = format!(
        "# 1. Before the catalogue\n\n### {early} — not in §2\n\n\
         # 2. The catalogue — the original's bugs, reproduced\n\n\
         ### B1 — a numbered heading\n\n\
         ### {heading} — a placeholder heading\n\n\
         | **B10** | a numbered row | — | here |\n\
         | **{row}** | a placeholder row | — | here |\n\
         | ~~**{struck}**~~ | a retracted placeholder row | — | here |\n\n\
         # 3. The original's bugs we do not reproduce\n\n### {late} — not in §2 either\n",
        early = tag("early"),
        heading = tag("heading"),
        row = tag("row"),
        struck = tag("struck"),
        late = tag("late"),
    );
    assert_eq!(
        catalogue_of(&doc),
        vec!["B1".to_string(), tag("heading"), "B10".to_string(), tag("row"), tag("struck")],
        "the catalogue parser does not see placeholder entries, so a branch's new bug row is \
         invisible to every test in this file until the integrator numbers it"
    );

    for bad in ["NEW-", "NEW-trailing-", "NEW--lead", "NEW-two--hyphens", "NEW-sp ace"] {
        assert!(!is_entry_id(&format!("B{bad}")), "B{bad} is not a placeholder corrections.js recognises");
    }
    assert!(is_entry_id(&tag("herd-four")) && is_entry_id(&tag("group_87")));
}

