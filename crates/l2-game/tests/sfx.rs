//! **`docs/audio.json` against the tree, in both directions.**
//!
//! The sibling of `tests/arms.rs`, and it exists for the reason that one
//! records: *a stated rule does not hold on this project and a checked one
//! does*. `docs/audio-triggers.md` enumerated 134 sound triggers, marked our
//! side **by hand in prose**, and had nothing behind it — so the moment a call
//! site was added, removed or renamed the document was wrong and nothing went
//! red. It said we reproduced 24; the first time this check was run the real
//! number was different in both directions.
//!
//! # The three artefacts and what each one can see
//!
//! | | compares | runs where |
//! |---|---|---|
//! | `node tools/oracle/sounds.js --check` | `docs/audio.json` against the **decompilation** | wherever the corpus is |
//! | this file | `docs/audio.json` against the **markers in `crates/`** | everywhere |
//! | a person | the marker against what the code does | nowhere mechanical |
//!
//! The first is what stops the inventory inventing a site or losing one; the
//! second is what stops a claim outliving the code that kept it. Neither can do
//! the third, and saying so is the point of the table.
//!
//! # A marker may claim several ids
//!
//! `tests/arms.rs` allows exactly one id per marker, because an input arm is a
//! gesture and two gestures are two behaviours. **Sound is not shaped like
//! that**, and pretending otherwise would be the fiction
//! honesty: `Msg_DrawWindow` asks for the narrator at sixteen places, each
//! guarded by one value of one countdown, and [`l2_game::audio::voice_tick`] is
//! that whole ladder in one function. Fourteen markers on one call would be
//! fourteen claims about one line.
//!
//! So the marker is `// sfx: <id>[,<id>…]` and the rule the check actually
//! enforces is the one that matters: **no id is claimed twice**, and the set of
//! claimed ids equals the set the file calls `reproduced`.
//!
//! # What it deliberately does not check
//!
//! That the sound is the *right* one, or that it fires at the right moment.
//! Nothing mechanical can. `tests/audio_wiring.rs` and `tests/audio_install.rs`
//! are where the behaviour is asserted; this is the census.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/l2-game has two ancestors")
        .to_path_buf()
}

/// The ids one line claims, if it is a marker.
///
/// The same shape as `tests/arms.rs`'s: everything before `// sfx:` has to be
/// comment punctuation, so a mention of the convention inside prose is not a
/// marker. That rule is what lets this file and `docs/audio.json` both describe
/// the convention without either of them counting as a use of it.
fn marker_on(line: &str) -> Option<Vec<String>> {
    let t = line.trim();
    let i = t.find("// sfx:")?;
    if !t[..i].chars().all(|c| matches!(c, '/' | '!' | '*' | ' ' | '\t')) {
        return None;
    }
    let rest = t[i + "// sfx:".len()..].split_whitespace().next()?;
    let ids: Vec<String> = rest.split(',').filter(|s| !s.is_empty()).map(str::to_string).collect();
    (!ids.is_empty()).then_some(ids)
}

/// Every `// sfx:` claim in the workspace's Rust, by id, with the file it is in.
fn markers(root: &Path) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut stack = vec![root.join("crates")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let path = e.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n == "target") {
                    continue;
                }
                stack.push(path);
            } else if path.extension().is_some_and(|x| x == "rs") {
                let Ok(text) = std::fs::read_to_string(&path) else { continue };
                let rel =
                    path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
                for line in text.lines() {
                    for id in marker_on(line).unwrap_or_default() {
                        out.entry(id).or_default().push(rel.clone());
                    }
                }
            }
        }
    }
    out
}

/// One trigger site of `Lords2.exe`, and our verdict on it.
struct Site {
    id: String,
    addr: String,
    class: String,
    status: String,
    ours: Option<String>,
    note: Option<String>,
}

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

/// **The denominator, pinned.**
///
/// `docs/audio-triggers.md` makes a great deal of this number being *generated
///
/// decompilation, which is gitignored and absent on every machine that has not
/// run Ghidra. So the generated number cannot be asserted here, and what can be
/// is that the file still holds the number the generator last produced. The two
/// halves meet at `--check`.
const TRIGGER_SITES: usize = 143;

/// Every status the file defines, and what each one obliges its record to say.
const STATUSES: &[&str] = &["reproduced", "blocked", "dead", "missing"];

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

/// **The triggers that are dead in the shipped game, by name.**
///
/// This asserted the `dead` list **empty** — *"a category that quietly acquires
/// members stops being a finding and becomes a bucket; the first entry should
/// cost a decision"* — and that was right. These three are that decision, and
/// each one is on `docs/bugs.md`'s dead-code list with its evidence:
///
/// | id | why it cannot run | bugs.md |
/// |---|---|---|
/// | `FUN_0040d6ad#1`, `FUN_0040d7b8#1` | the two arrows of a slider widget whose hit-tester `FUN_0040D3F5` has **no caller**: zero rel32 calls, zero jumps and zero absolute references in the image, against 36 rel32 callers for `Widget_Test` as the control | `D39` |
/// | `Battle_PauseButton#1` | guarded on a word that alternates between 0 and −1; the only writer that could make it 1, `FUN_00434E68`, has no reference either | D38 |
/// | `FUN_004b39e8#1` | the degraded castle's three lines: the thunk that plays them has **no caller** — zero rel32 calls, zero jumps and zero dword references in the image, against one rel32 caller for its sibling `FUN_004B3714` as the control | `D40` |
///
/// **The third was already on the bug list and was filed `missing` here**, which
/// is the inventory contradicting the document it should agree with — and it is
/// what an assertion of emptiness does to a real finding: the one person who
/// could have filed it `dead` was told not to.
///
/// **A pinned set, not a relaxed check.** The fourth entry cost exactly what the
/// first did, and so will the fifth: say in `note` which guard can never hold —
/// or, as `FUN_004b39e8#1` does, that *nothing calls the function at all* — add
/// it to `docs/bugs.md`, and add its id here.
///
/// **The fourth is a different species from the first three and that is the
/// point of naming them individually.** The first three are code that runs and
/// cannot produce a sound; the fourth is a function nobody calls. Both are
/// `dead`, and only the second kind can be mistaken for *"we have not built the
/// caller yet"* — which is precisely what `docs/audio.json` said about it for
/// as long as the record existed.
const DEAD_IN_THE_SHIPPED_GAME: &[&str] =
    &["Battle_PauseButton#1", "FUN_0040d6ad#1", "FUN_0040d7b8#1", "FUN_004b39e8#1"];

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
