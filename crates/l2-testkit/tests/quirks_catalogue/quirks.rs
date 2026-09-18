#![allow(unused_imports)]
use super::*;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use Disposition::{Retracted, Switchable, Unswitchable, Unwired};
use Home::{Behavioural, Presentation};

fn presentation_rows(root: &Path) -> usize {
    presentation(root).0.len()
}

pub(crate) fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_path_buf()
}

fn read(root: &Path, rel: &str) -> String {
    let p = root.join(rel);
    if p.file_name().is_some_and(|f| f == "mod.rs") {
        let dir = p.parent().unwrap();
        let mut names: Vec<_> = std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|q| q.extension().is_some_and(|x| x == "rs"))
            .collect();
        names.sort();
        return names.iter().map(|q| std::fs::read_to_string(q).unwrap().replace("\r\n", "\n")).collect::<Vec<_>>().join("\n");
    }
    std::fs::read_to_string(&p)
        .unwrap_or_else(|e| panic!("{} is part of this check and could not be read: {e}", p.display()))
        .replace("\r\n", "\n")
}

fn catalogue(root: &Path) -> Vec<String> {
    catalogue_of(&read(root, "docs/bugs.md"))
}

fn catalogue_of(doc: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in doc.lines() {
        if line.starts_with("# 2. The catalogue") {
            inside = true;
            continue;
        }
        if inside && line.starts_with("# 3.") {
            break;
        }
        if !inside {
            continue;
        }
        let id = if let Some(rest) = line.strip_prefix("### ") {
            rest.split_whitespace().next().unwrap_or("")
        } else if let Some(rest) = line.strip_prefix("| **") {
            rest.split("**").next().unwrap_or("")
        } else if let Some(rest) = line.strip_prefix("| ~~**") {
            rest.split("**").next().unwrap_or("")
        } else {
            continue;
        };
        if is_entry_id(id) {
            out.push(id.to_string());
        }
    }
    out
}

fn is_entry_id(s: &str) -> bool {
    let Some(rest) = s.strip_prefix('B') else { return false };
    if let Some(slug) = rest.strip_prefix("NEW-") {
        return slug
            .split('-')
            .all(|run| !run.is_empty() && run.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
    }
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return false;
    }
    let tail = &rest[digits.len()..];
    tail.is_empty() || (tail.len() == 1 && tail.chars().all(|c| c.is_ascii_lowercase()))
}

fn quirk_variants(root: &Path) -> Vec<(String, String)> {
    let src = read(root, "crates/l2-net/src/quirks/mod.rs");
    let body = src
        .split("pub const fn entry(self) -> &'static str {")
        .nth(1)
        .expect("l2_net::Quirk::entry is what this check joins on; it has moved or been renamed");
    let body = body.split("\n    }").next().unwrap_or(body);
    let mut out = Vec::new();
    for arm in body.split("Quirk::").skip(1) {
        let variant: String =
            arm.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        let Some(rest) = arm.split_once("=> ") else { continue };
        let entry: String = rest.1.trim_start().trim_start_matches('"').chars().take_while(|c| *c != '"').collect();
        if !variant.is_empty() && is_entry_id(&entry) {
            out.push((variant, entry));
        }
    }
    out
}

fn sources(root: &Path) -> Vec<(String, String)> {
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
    let mut paths = Vec::new();
    walk(&root.join("crates"), &mut paths);
    paths
        .into_iter()
        .map(|p| {
            let rel = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace('\\', "/");
            let text = std::fs::read_to_string(&p).unwrap_or_default().replace("\r\n", "\n");
            (rel, text)
        })
        .collect()
}

fn presentation(root: &Path) -> (Vec<(String, String)>, Vec<String>) {
    let src = read(root, "crates/l2-game/src/game/mod.rs");

    let table = src
        .split("pub const PRESENTATION: &[(&str, &str)] = &[")
        .nth(1)
        .expect("l2_game::game::PRESENTATION is what this check joins on; it has moved");
    let table = table.split("];").next().unwrap_or("");
    let mut rows = Vec::new();
    for row in table.split('(').skip(1) {
        let quoted: Vec<&str> = row.split('"').skip(1).step_by(2).collect();
        if quoted.len() >= 2 {
            rows.push((quoted[0].to_string(), quoted[1].to_string()));
        }
    }

    let body = src
        .split("pub struct Quirks {")
        .nth(1)
        .expect("l2_game::game::Quirks is what this check joins on; it has moved");
    let body = body.split("\n}").next().unwrap_or("");
    let fields: Vec<String> = body
        .lines()
        .filter_map(|l| l.trim().strip_prefix("pub "))
        .filter_map(|l| l.split(':').next())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    (rows, fields)
}

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

