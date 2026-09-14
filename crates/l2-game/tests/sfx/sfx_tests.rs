#![allow(unused_imports)]
use super::*;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// `docs/audio.json`'s `sites` array.
///
/// A scanner
/// must not add a dependency to the build, and the file is generated with one
/// field per line by `tools/oracle/sounds.js --rebuild`. It **fails loudly**
///
/// matters for a file whose whole purpose is a count.
fn sites(root: &Path) -> Vec<Site> {
    let text = std::fs::read_to_string(root.join("docs/audio.json")).expect("docs/audio.json");
    let field = |line: &str, name: &str| -> Option<String> {
        let rest = line.trim().strip_prefix(&format!("\"{name}\":"))?;
        let rest = rest.trim().trim_end_matches(',').trim();
        (rest != "null").then(|| rest.trim_matches('"').to_string())
    };
    let mut out: Vec<Site> = Vec::new();
    for line in text.lines() {
        if let Some(v) = field(line, "id") {
            out.push(Site {
                id: v,
                addr: String::new(),
                class: String::new(),
                status: String::new(),
                ours: None,
                note: None,
            });
            continue;
        }
        let Some(last) = out.last_mut() else { continue };
        if let Some(v) = field(line, "addr") {
            last.addr = v;
        } else if let Some(v) = field(line, "class") {
            last.class = v;
        } else if let Some(v) = field(line, "status") {
            last.status = v;
        } else if let Some(v) = field(line, "ours") {
            last.ours = Some(v);
        } else if let Some(v) = field(line, "note") {
            last.note = Some(v);
        }
    }
    assert_eq!(
        out.len(),
        TRIGGER_SITES,
        "docs/audio.json parsed to {} sites and the denominator is {TRIGGER_SITES}. \
         Either the scanner lost the file, or the corpus really did change — in which case \
         run `node tools/oracle/sounds.js --check`, and if it agrees, move this constant \
         deliberately and say in the commit what the original gained or lost.",
        out.len(),
    );
    out
}

/// **The check.**
#[test]
fn every_reproduced_trigger_has_a_marker_and_every_marker_has_a_record() {
    let root = repo_root();
    let found = markers(&root);
    let recorded = sites(&root);

    let claimed: BTreeSet<String> =
        recorded.iter().filter(|s| s.status == "reproduced").map(|s| s.id.clone()).collect();
    let marked: BTreeSet<String> = found.keys().cloned().collect();

    let unkept: Vec<&String> = claimed.difference(&marked).collect();
    let unrecorded: Vec<&String> = marked.difference(&claimed).collect();

    assert!(
        unkept.is_empty(),
        "docs/audio.json calls these triggers `reproduced` and no `// sfx:` marker claims \
         them:\n  {}\n\
         Either the call site went away — in which case the record is now `missing` or \
         `blocked` and should say which — or somebody forgot the marker.",
        unkept.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n  "),
    );
    assert!(
        unrecorded.is_empty(),
        "these `// sfx:` markers are in the code and are not `reproduced` in \
         docs/audio.json:\n  {}\n\
         A sound we play and do not count is the exact failure the inventory exists to \
         prevent: the number goes stale downward and reads like progress not made.",
        unrecorded
            .iter()
            .map(|id| format!("{id}  ({})", found[*id].join(", ")))
            .collect::<Vec<_>>()
            .join("\n  "),
    );
}

/// One marker per trigger, so a record cannot silently mean two places.
#[test]
fn no_trigger_is_claimed_twice() {
    let root = repo_root();
    for (id, files) in markers(&root) {
        assert_eq!(
            files.len(),
            1,
            "{id} is claimed in {} places: {files:?}. Two call sites for one of the \
             original's is either a double sound or a copied marker, and both are worth \
             looking at.",
            files.len(),
        );
    }
}

/// **Every record says enough to act on**, which is the half of the inventory a
/// set comparison cannot reach.
///
/// The task this file was written for asked for one of three verdicts per
/// unreached trigger — *fired*, *cannot be fired yet and here is the mechanic*,
/// or *dead in the original and here is the evidence*. The first is checked
/// above. The other two are checked here, and the check is simply that the
/// record **says which and why**: a `blocked` with no note is the shape of an
/// answer without the answer in it, and that is what a hand-marked document
/// degrades into.
#[test]
fn every_record_carries_the_verdict_it_claims() {
    let root = repo_root();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for s in sites(&root) {
        assert!(
            STATUSES.contains(&s.status.as_str()),
            "{} has status `{}`, which is not one of {STATUSES:?}",
            s.id,
            s.status,
        );
        assert!(s.addr.starts_with("0x"), "{} has no address", s.id);
        if s.status == "reproduced" {
            let ours = s.ours.clone().unwrap_or_default();
            assert!(
                ours.starts_with("crates/"),
                "{} is reproduced and does not say where: `ours` is {ours:?}",
                s.id,
            );
        }
        if s.status == "blocked" || s.status == "dead" {
            let note = s.note.clone().unwrap_or_default();
            assert!(
                note.len() > 20,
                "{} is `{}` and its note is {note:?}. A `blocked` record must NAME the \
                 mechanic that is missing and a `dead` one must give the evidence; \
                 without that it is indistinguishable from `missing` and stops being a \
                 verdict at all.",
                s.id,
                s.status,
            );
        }
        *counts.entry(s.status.clone()).or_default() += 1;
    }
    // Three of the four must be populated. `dead` is asserted the other way —
    // see below.
    for s in ["reproduced", "blocked", "missing"] {
        assert!(counts.get(s).copied().unwrap_or(0) > 0, "no {s} triggers at all");
    }
}

#[test]
fn the_dead_triggers_are_the_ones_on_the_bug_list() {
    let root = repo_root();
    let dead: BTreeSet<String> =
        sites(&root).into_iter().filter(|s| s.status == "dead").map(|s| s.id).collect();
    let pinned: BTreeSet<String> =
        DEAD_IN_THE_SHIPPED_GAME.iter().map(|s| s.to_string()).collect();
    assert_eq!(
        dead, pinned,
        "docs/audio.json's `dead` triggers are not the pinned set. A NEW one is a finding \
         about `Lords2.exe` and not a status to live with: say in `note` which guard can \
         never hold and why, add it to `docs/bugs.md`'s dead-code list, and then add its id \
         to DEAD_IN_THE_SHIPPED_GAME deliberately. A pinned one that LEFT means somebody found \
         a path the evidence missed, and bugs.md is wrong too.",
    );
    let text = std::fs::read_to_string(root.join("docs/bugs.md")).expect("docs/bugs.md");
    for (id, entry) in [
        ("slider arrows", "D39"),
        ("Battle_PauseButton#1", "**D38**"),
        ("FUN_004b39e8#1", "**D40**"),
    ] {
        assert!(text.contains(entry), "{id} is dead here and {entry} is gone from docs/bugs.md");
    }
}

