//! **`docs/arms.json` against the tree, in both directions.**
//!
//! `CLAUDE.md` rule 5 says a feature must name the original function it
//! reproduces. `docs/decisions.md` C61 says stated rules do not hold on this
//! project and checked ones do — the numbering protocol was written after four
//! collisions and did not prevent the fifth. So this is the check, and it is the
//! only reason the file's shape is what it is.
//!
//! # The check
//!
//! The set of `id`s in `docs/arms.json` with status `reproduced` is **equal** to
//! the set of `// arm: <id>` markers in `crates/`. Both differences are named
//! when it fails, because they mean different things:
//!
//! * **a record with no marker** is a claim nobody kept — the file says we do
//!   something and no line of code says it does it;
//! * **a marker with no record** is an arm nobody wrote down, which is the exact
//!   failure C61 measured: behaviour that exists and is not in the inventory.
//!
//! # Two forms of marker, and why one of them is refused for three words
//!
//! A marker is a comment, `// arm: <id> <gesture>`, **or** a declaration,
//! `crate::arm!("<id>", <Kind>)` — which *is* the `press::Kind` it names and
//! nothing else. The declaration's gesture is not written anywhere: it is
//! [`Kind::gesture`] of the identifier, so the kind a widget is answered with
//! and the word its marker claims are one token.
//!
//! That form exists because the comment form was checked against the record
//! and never against the code beside it. Every options row was once declared
//! `Kind::Press` under a `left-press-delayed` comment, and this file stayed
//! green; `tests/options.rs` caught it. **So a comment may not claim
//! `left-press-repeat`, `left-press-delayed` or `left-press-held`**: those are
//! answered by nothing but a `Kind` handed to `Press`, so a claim of one has to
//! be that `Kind`. A plain press or a release can still be a comment, because
//! hand-rolled hit tests answer those.
//!
//! # What it deliberately does not check
//!
//! That the *implementation* is right. Nothing mechanical can. What it buys is
//! that the inventory cannot rot silently, which is what happened to
//! `input.rs`'s "the only reader of `g_mouseLeftDoubleClick`" — a claim that was
//! true when it was written, false by the time anybody looked, and checked by
//! nothing.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use l2_game::press::Kind;

fn repo_root() -> PathBuf {
    // `CARGO_MANIFEST_DIR` is `<root>/crates/l2-game`.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/l2-game has two ancestors")
        .to_path_buf()
}

/// The marker on one line, if it holds one.
///
/// A marker is `// arm: <id> <gesture>` **on its own**, and it is allowed to be
/// nested inside a doc comment — `/// // arm: 0x…` — because the natural place
/// for it is the last line of the doc comment that explains the arm. So
/// everything before it on the line has to be comment punctuation and nothing
/// else, which is what stops a mention of the convention inside prose from
/// counting as one.
///
/// **The gesture is the second word and it is not decoration.** Until it
/// existed, an arm could be marked `reproduced`, be genuinely present, and be
/// answered with the wrong *kind* of gesture in every case — a press where the
/// original waits for the release, a single fire where it auto-repeats — and
/// nothing anywhere could tell. See `docs/input.md`.
fn marker_on(line: &str) -> Option<Marker> {
    let t = line.trim();
    let i = t.find("// arm:")?;
    if !t[..i].chars().all(|c| matches!(c, '/' | '!' | '*' | ' ' | '\t')) {
        return None;
    }
    let mut words = t[i + "// arm:".len()..].split_whitespace();
    let id = words.next()?;
    if id.is_empty() {
        return None;
    }
    Some(Marker { id: id.to_string(), gesture: words.next().unwrap_or_default().to_string() })
}

/// One `// arm:` marker: what it claims to implement, and with what gesture.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Marker {
    id: String,
    gesture: String,
}

/// How a marker is written.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum How {
    /// `// arm: <id> <gesture>` — the gesture is a word somebody typed.
    Comment,
    /// `arm!("<id>", <Kind>)` — the gesture is the `Kind`'s own.
    Declared,
}

/// One marker, where it is, and how it is written.
struct Site {
    marker: Marker,
    /// `path:line`, relative to the repository.
    at: String,
    how: How,
}

/// **Every `press::Kind` by its identifier**, read off the type rather than
/// listed here.
///
/// `Kind::from_record` is how the original's kind bytes become kinds, and every
/// kind is one of those bytes, so walking both testers' 256 values finds all
/// five. A list in this file would be a second copy of the enum that nobody
/// would remember to extend.
fn kinds_by_name() -> BTreeMap<String, Kind> {
    let mut out = BTreeMap::new();
    for widget in [false, true] {
        for byte in 0..=u8::MAX {
            if let Some(k) = Kind::from_record(widget, byte) {
                out.insert(format!("{k:?}"), k);
            }
        }
    }
    assert_eq!(out.len(), 5, "the five gesture kinds, by name: {:?}", out.keys());
    out
}

/// The macro's name and its opening parenthesis, built so that this file's own
/// source does not contain the sequence it scans for.
const DECLARATION: &str = concat!("arm", "!(");

/// Every `arm!("<id>", <Kind>)` in one file, with its line number.
///
/// **A declaration this cannot read is a failure, not a skip** — a marker the
/// check cannot hold to its record is exactly the marker that drifts.
/// Occurrences on a comment line are prose or a doc example and are not
/// declarations, which is the same rule [`marker_on`] applies the other way
/// round.
fn declarations_in(text: &str, file: &str, kinds: &BTreeMap<String, Kind>) -> Vec<(usize, Marker)> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(off) = text[from..].find(DECLARATION) {
        let at = from + off;
        from = at + DECLARATION.len();
        // `farm!(` is not this macro.
        if text[..at].chars().next_back().is_some_and(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }
        let line_start = text[..at].rfind('\n').map_or(0, |i| i + 1);
        if text[line_start..at].contains("//") {
            continue;
        }
        let line = text[..at].matches('\n').count() + 1;
        let rest = &text[from..];
        let args = &rest[..rest.find(')').unwrap_or_else(|| {
            panic!("{file}:{line}: an arm! declaration with no closing parenthesis")
        })];
        let parsed = args.split_once(',').and_then(|(id, kind)| {
            let id = id.trim().strip_prefix('"')?.strip_suffix('"')?;
            Some((id.to_string(), kind.trim().trim_end_matches(',').trim().to_string()))
        });
        let Some((id, name)) = parsed else {
            panic!(
                "{file}:{line}: arm! declaration `{}` is not `(\"<id>\", <Kind>)`, and a marker \
                 this check cannot read is one it cannot hold to its record",
                args.trim(),
            );
        };
        let Some(kind) = kinds.get(&name) else {
            panic!("{file}:{line}: `{name}` is not one of press::Kind's {:?}", kinds.keys());
        };
        out.push((line, Marker { id, gesture: kind.gesture().to_string() }));
    }
    out
}

/// Every marker in the workspace's Rust, of both forms, with where it is.
///
/// A hand-rolled walk rather than a crate: this test must not add a dependency
/// to build, and the tree is a few hundred files.
fn sites(root: &Path) -> Vec<Site> {
    let kinds = kinds_by_name();
    let mut out = Vec::new();
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
                for (n, line) in text.lines().enumerate() {
                    let Some(marker) = marker_on(line) else { continue };
                    out.push(Site { marker, at: format!("{rel}:{}", n + 1), how: How::Comment });
                }
                for (n, marker) in declarations_in(&text, &rel, &kinds) {
                    out.push(Site { marker, at: format!("{rel}:{n}"), how: How::Declared });
                }
            }
        }
    }
    out
}

/// Every marker, of either form, with the places it is written.
fn markers(root: &Path) -> BTreeMap<Marker, Vec<String>> {
    let mut out: BTreeMap<Marker, Vec<String>> = BTreeMap::new();
    for s in sites(root) {
        out.entry(s.marker).or_default().push(s.at);
    }
    out
}

/// The `arms` array's `(id, status)` pairs, without pulling in a JSON crate.
///
/// The file is written by hand and read by a person; a three-line scanner over
/// `"id"` and `"status"` lines is enough and cannot pull a dependency into the
/// build. It **fails loudly** if the two do not alternate, which is what would
/// happen if somebody reformatted the file onto one line.
fn records(root: &Path) -> Vec<Record> {
    let text = std::fs::read_to_string(root.join("docs/arms.json")).expect("docs/arms.json");
    let field = |line: &str, name: &str| -> Option<String> {
        let key = format!("\"{name}\":");
        let rest = line.trim().strip_prefix(&key)?;
        let rest = rest.trim().trim_end_matches(',').trim();
        Some(rest.trim_matches('"').to_string())
    };
    let mut out: Vec<Record> = Vec::new();
    let mut id: Option<String> = None;
    // **Addresses seen since the last `"status"` line**, held rather than
    // attached at once. A record is created on its `"status"` line, so its
    // `"id"` line arrives while the *previous* record is still `out.last_mut()`
    // — and attaching there silently gave `tax-and-ration-arrows` the supplies
    // thumb's address and a kind it does not have.
    let mut pending: BTreeSet<u32> = BTreeSet::new();
    for line in text.lines() {
        // **Only the prose fields**, and the exclusions are each a case that
        // went wrong before the list existed:
        //
        // * `id` — an id's address is sometimes a **table base** rather than a
        //   record. `0x004DD538/supplies-sheep-row` is `g_sendSuppliesWidgets`,
        //   whose record 0 is a kind-5 thumb and whose sixth and seventh
        //   records are the kind-4 pair the arm is about. Reading the id as a
        //   record said kind 5 about a spinner.
        // * `addr` — the `addr`-keyed half of this check already reads it, as a
        //   handler.
        // * everything outside a record — the `groups` prose at the top of the
        //   file names `g_splitWidgets`, and it would have landed on whichever
        //   record happened to be first.
        if field(line, "id").is_some() {
            pending.clear();
        }
        if PROSE_FIELDS.iter().any(|f| field(line, f).is_some()) {
            for a in addresses_in(line) {
                pending.insert(a);
            }
        }
        if let Some(v) = field(line, "id") {
            assert!(id.is_none(), "two \"id\" lines with no \"status\" between them: {v}");
            id = Some(v);
        } else if let Some(v) = field(line, "status") {
            let key = id.take().expect("a \"status\" line with no \"id\" above it");
            out.push(Record {
                id: key,
                status: v,
                gesture: String::new(),
                addr: None,
                ours: None,
                removed: false,
                prose: std::mem::take(&mut pending),
            });
        } else if let Some(v) = field(line, "gesture") {
            if let Some(last) = out.last_mut() {
                last.gesture = v;
            }
        } else if let Some(v) = field(line, "addr") {
            if let Some(last) = out.last_mut() {
                last.addr = u32::from_str_radix(v.trim_start_matches("0x"), 16).ok();
            }
        } else if let Some(v) = field(line, "ours") {
            if let Some(last) = out.last_mut() {
                last.ours = Some(v);
            }
        } else if let Some(v) = field(line, "removed") {
            if let Some(last) = out.last_mut() {
                last.removed = v == "true";
            }
        }
        // Everything after the `"status"` line belongs to the record it made.
        // Deliberately not restricted to `what` and `note`: a table named in a
        // `merged` field is still a table this record is about, and the region
        // filter at the use site is what makes the net safe rather than the
        // field name.
        // `id` is `Some` exactly between a record's `"id"` line and its
        // `"status"` line, which is the window in which the *previous* record
        // is still `out.last_mut()`. Appending there is how
        // `tax-and-ration-arrows` picked up the supplies thumb's address out of
        // the next record's id and acquired a kind it does not have.
        if id.is_none() {
            if let Some(last) = out.last_mut() {
                last.prose.append(&mut pending);
            }
        }
    }
    assert!(id.is_none(), "the last record has an \"id\" and no \"status\"");
    assert!(out.len() > 20, "only {} records parsed — the scanner lost the file", out.len());
    out
}

/// **The fields whose text is prose about the original**, and therefore the
/// only ones the address scan reads. See `records` for why each of the others
/// is excluded.
const PROSE_FIELDS: &[&str] = &["what", "note", "why", "merged"];

/// Every `0x00xxxxxx` in one line of text, however it is spelled.
///
/// `DAT_004DD790`, `0x004DD790` and `&DAT_004DD790` are the three forms this
/// file's prose uses, and they differ only in what precedes the eight hex
/// digits — so the scan is for the digits and the prefix is ignored.
fn addresses_in(line: &str) -> Vec<u32> {
    let b: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    for i in 0..b.len() {
        if b[i] != '0' || i + 8 > b.len() || b[i + 1] != '0' {
            continue;
        }
        // A run of exactly eight hex digits starting `00`, not part of a longer
        // one.
        if !b[i..i + 8].iter().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        if b.get(i + 8).is_some_and(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        if let Ok(v) = u32::from_str_radix(&b[i..i + 8].iter().collect::<String>(), 16) {
            out.push(v);
        }
    }
    out
}

/// One record of `docs/arms.json`.
struct Record {
    id: String,
    status: String,
    /// **What kind of gesture this is**, from the closed vocabulary in
    /// [`GESTURES`]. Not *which* control — that is `what` — but how the
    /// original decides the control has been used at all.
    gesture: String,
    /// The original function, when the record names one. Inventions do not.
    addr: Option<u32>,
    ours: Option<String>,
    /// `invention` only: whether the code has since been taken out.
    removed: bool,
    /// **Every address the record's prose names**, which is how the exe-gated
    /// check reaches the four arms whose `addr` is a dispatcher.
    ///
    /// `0x004BA9C8` is `Screen_HandleInput`, 3,832 bytes and nobody's handler,
    /// so a record filed under it has no kind byte to read — and then names its
    /// widget table in its own `what`: *"g_taxWidgets (0x004DD790) and
    /// g_rationWidgets (0x004DD7C0)"*. That address is the thing the check
    /// wanted, written months before the gesture field existed and by somebody
    /// not thinking about kinds, which is the property `docs/agents.md` says a
    /// second artefact needs and usually does not have.
    prose: BTreeSet<u32>,
}

impl Record {
    /// **Is this arm in the tree right now**, and therefore required to carry a
    /// marker?
    ///
    /// Not the same question as *"do we implement it"*, and the difference is
    /// the whole reason this is a method rather than a set membership test. A
    /// **removed** invention is something we implemented and took out: it stays
    /// in the file, because the count of inventions is half the 1:1
    /// measurement and deleting the record would delete the evidence, but there
    /// is no code left to mark.
    ///
    /// The original rule was `status == "reproduced"`, which is right today and
    /// only by accident: every invention on file happens to be removed. An
    /// invention we decided to KEEP would need a marker and would have slipped
    /// through, which is the hole this closes.
    fn in_the_tree(&self) -> bool {
        match self.status.as_str() {
            "reproduced" | "dead-reproduced" => true,
            "invention" => !self.removed,
            _ => false,
        }
    }
}

/// Every status the file defines. The cross-product of *what the original has*
/// and *what we have*, minus the cell that is not a record.
const STATUSES: &[&str] = &["reproduced", "missing", "dead", "invention", "dead-reproduced"];

/// **Every gesture kind the file may name.**
///
/// Closed on purpose. `gesture` was an open field and it had drifted into three
/// different things at once: a kind (`left-press`), a *position in a table*
/// (`button-4`, fourteen records), and "not a gesture at all" (`draw`,
/// `timer`). A field that means three things cannot be the thing a check reads,
/// and reading it is the entire point — see `docs/input.md`.
///
/// The five mouse kinds are the original's own, not a taxonomy of ours. The
/// interface has exactly two hit-testers and each reads a **kind byte** at
/// `+0x0F` of the 24-byte record it is walking:
///
/// | value | tester | what it does |
/// |---|---|---|
/// | `left-press` | `Hotspot_Test` kind 1 | fires on the down edge |
/// | `left-press-held` | `Hotspot_Test` kind 2 | down edge, then every 320 ms while held |
/// | `left-release` | `Hotspot_Test` kind 3, and `Ui_OkButtonClicked` | fires on the **up** edge |
/// | `left-press-repeat` | `Widget_Test` kind 4 | down edge, pressed frame, **accelerating** repeat |
/// | `left-press-delayed` | `Widget_Test` kind 5 | down edge shows the pressed frame; the handler runs **20 frames later** |
///
/// Adding a value here should cost a decision, which is why it is a list and
/// not a regex.
const GESTURES: &[&str] = &[
    // The mouse, by kind.
    "left-press",
    "left-press-held",
    "left-press-repeat",
    "left-press-delayed",
    "left-release",
    "right-release",
    "double-click",
    "drag",
    "hover",
    "hover-at-edge",
    "pointer",
    // The keyboard. `key` is `WM_KEYDOWN`, `type` is `WM_CHAR`, and the
    // original dispatches them from two different messages.
    "key",
    "type",
    // Not gestures. These are here because the arm they name is real input
    // behaviour that lives outside the input ladder — `docs/arms.json`'s own
    // note on the five places behaviour hides — and leaving them out would
    // make them unrecordable rather than making them not exist.
    "draw",
    "frame",
    "timer",
    "none",
];

/// The gesture a `Widget_Test` / `Hotspot_Test` kind byte means.
///
/// `widget` says which of the two testers walks the table: they use **the same
/// 24-byte record and different kind numbers**, which is why the tester has to
/// be part of the question. `Hotspot_Test`'s 3 is a release; `Widget_Test` has
/// no 3 that fires at all.
fn gesture_of_kind(widget: bool, kind: u8) -> Option<&'static str> {
    Some(match (widget, kind) {
        (false, 1) => "left-press",
        (false, 2) => "left-press-held",
        (false, 3) => "left-release",
        (true, 4) => "left-press-repeat",
        (true, 5) => "left-press-delayed",
        _ => return None,
    })
}

/// **The check.**
#[test]
fn every_reproduced_arm_has_a_marker_and_every_marker_has_a_record() {
    let root = repo_root();
    let found = markers(&root);
    let recorded = records(&root);

    let claimed: BTreeSet<Marker> = recorded
        .iter()
        .filter(|r| r.in_the_tree())
        .map(|r| Marker { id: r.id.clone(), gesture: r.gesture.clone() })
        .collect();
    let marked: BTreeSet<Marker> = found.keys().cloned().collect();

    let unkept: Vec<&Marker> = claimed.difference(&marked).collect();
    let unrecorded: Vec<&Marker> = marked.difference(&claimed).collect();

    // The pair is the unit, so a marker whose *gesture* is wrong appears in
    // both lists. Say so rather than making a reader notice: the whole reason
    // the gesture is on the marker is that answering an arm with the wrong kind
    // used to be invisible, and reporting it as two unrelated failures would
    // put it back.
    let by_id: BTreeMap<&str, &Marker> = marked.iter().map(|m| (m.id.as_str(), m)).collect();
    let mut mismatched: Vec<String> = Vec::new();
    for c in &claimed {
        if let Some(m) = by_id.get(c.id.as_str()) {
            if m.gesture != c.gesture {
                mismatched.push(format!(
                    "{}\n      docs/arms.json says the original's gesture is `{}`\n      \
                     the marker in {} says we answer `{}`",
                    c.id,
                    c.gesture,
                    found[*m].join(", "),
                    if m.gesture.is_empty() { "<no gesture on the marker>" } else { &m.gesture },
                ));
            }
        }
    }
    assert!(
        mismatched.is_empty(),
        "these arms are implemented with the WRONG KIND of gesture, or the marker and the \
         record have drifted apart:\n  {}\n\
         An arm answered with the wrong kind is not reproduced. `docs/input.md` says what \
         each kind is; fix the handler, or the record if the record is what is wrong.",
        mismatched.join("\n  "),
    );

    let describe = |m: &Marker| format!("{} {}", m.id, m.gesture);
    assert!(
        unkept.is_empty(),
        "docs/arms.json claims these are implemented and no `// arm:` marker says so:\n  {}",
        unkept.iter().map(|m| describe(m)).collect::<Vec<_>>().join("\n  "),
    );
    assert!(
        unrecorded.is_empty(),
        "these `// arm:` markers are in the code and not in docs/arms.json:\n  {}",
        unrecorded
            .iter()
            .map(|m| format!("{}  ({})", describe(m), found[*m].join(", ")))
            .collect::<Vec<_>>()
            .join("\n  "),
    );
}

/// **Every gesture is one the file defines, and every marker carries one.**
///
/// The second half is what stops the pair check above from being satisfied by
/// leaving the gesture off both sides.
#[test]
fn every_arm_names_a_gesture_from_the_closed_vocabulary() {
    let root = repo_root();
    for r in records(&root) {
        assert!(
            GESTURES.contains(&r.gesture.as_str()),
            "{} has gesture `{}`, which is not one of {GESTURES:?}.\n\
             A new kind is a claim about how the original decides a control was used. \
             Read `docs/input.md` first; if it really is new, add it there and here \
             together.",
            r.id,
            r.gesture,
        );
    }
    for (m, files) in markers(&root) {
        assert!(
            GESTURES.contains(&m.gesture.as_str()),
            "the marker for {} in {} carries `{}`, which is not a gesture. \
             The marker is `// arm: <id> <gesture>`.",
            m.id,
            files.join(", "),
            m.gesture,
        );
    }
}

/// **A kind only `Press` answers is declared by that kind, never claimed by a
/// comment.**
///
/// The three are `Widget_Test`'s kinds 4 and 5 and `Hotspot_Test`'s kind 2.
/// Nothing in this engine answers them but a `press::Kind` handed to
/// `press::Press`, so a marker claiming one is a claim about a `Kind` value —
/// and the only marker that cannot disagree with that value is the value. A
/// comment beside a table could say `left-press-delayed` over a row declared
/// `Kind::Press`, and did, and the set check above was satisfied by the comment.
///
/// **Ablations, run:** declare `opt-music`'s row `Press` in its `arm!` and the
/// set check above goes red, naming the arm and both words; replace the `arm!`
/// with a bare `Kind::Press` and put the old comment back above it, and this
/// one goes red naming the file and line.
#[test]
fn a_press_only_gesture_is_declared_by_its_kind_and_never_by_a_comment() {
    let root = repo_root();
    let press_only: BTreeSet<&'static str> = kinds_by_name()
        .into_values()
        .filter(|k| !matches!(k, Kind::Press | Kind::Release))
        .map(Kind::gesture)
        .collect();
    assert_eq!(press_only.len(), 3, "repeat, delayed and held: {press_only:?}");

    let all = sites(&root);
    let typed: Vec<String> = all
        .iter()
        .filter(|s| s.how == How::Comment && press_only.contains(s.marker.gesture.as_str()))
        .map(|s| format!("{}  // arm: {} {}", s.at, s.marker.id, s.marker.gesture))
        .collect();
    assert!(
        typed.is_empty(),
        "these markers claim a kind only press::Press answers, in a comment:\n  {}\n\
         A comment can say `left-press-delayed` over a widget declared `Kind::Press`, and \
         nothing would notice. Delete the comment and write the widget's kind as an \
         arm! declaration — the id and `Delayed` as its two arguments — so the marker and \
         the kind are one token.",
        typed.join("\n  "),
    );

    // **And the declared half is not empty**, or the scanner went blind and the
    // assertion above is vacuous. Thirty-four when this was written: every
    // widget table in `screens/`, and the auto-repeat in `press.rs`.
    let declared = all.iter().filter(|s| s.how == How::Declared).count();
    assert!(declared >= 34, "only {declared} arm! declarations found; it was 34");
}

/// One marker per arm, so a record cannot silently mean two places.
#[test]
fn no_arm_is_marked_twice() {
    let root = repo_root();
    // By `id`, not by the whole marker: two markers for one arm that disagree
    // about the gesture are still two markers for one arm, and folding them
    // into separate keys would hide exactly that case.
    let mut places: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (m, files) in markers(&root) {
        places.entry(m.id).or_default().extend(files);
    }
    for (id, files) in places {
        assert_eq!(files.len(), 1, "{id} is marked in {} places: {files:?}", files.len());
    }
}

/// Every id starts with something a `grep` for the marker convention finds, and
/// every status is one the file defines.
#[test]
fn the_inventory_uses_the_defined_statuses_and_the_marker_shape() {
    let root = repo_root();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for r in records(&root) {
        let (id, status) = (&r.id, &r.status);
        assert!(
            id.starts_with("0x") || id.starts_with("ours/"),
            "{id} is neither an address nor an invention",
        );
        assert!(
            STATUSES.contains(&status.as_str()),
            "{id} has status {status}, which is not one of {STATUSES:?}",
        );
        if r.in_the_tree() {
            let ours = r.ours.clone().unwrap_or_default();
            assert!(ours.starts_with("crates/"), "{id} does not say where it lives");
        }
        *counts.entry(status.clone()).or_default() += 1;
    }
    // Four of the five must be populated — they are the four things this file
    // exists to count, and an empty one means the enumeration stopped early.
    //
    // `dead-reproduced` is the exception and is expected to be EMPTY: it names
    // work we should not have done. It is asserted the other way round, because
    // a category that quietly acquires members is exactly how "we built an arm
    // no player can reach" would stop being a finding and become a bucket.
    for s in ["reproduced", "missing", "dead", "invention"] {
        assert!(counts.get(s).copied().unwrap_or(0) > 0, "no {s} arms at all");
    }
    let built_dead = counts.get("dead-reproduced").copied().unwrap_or(0);
    assert_eq!(
        built_dead, 0,
        "{built_dead} arm(s) are marked dead-reproduced: we have built input the shipped \
         game cannot reach. That is a finding, not a status to live with — read each one \
         and delete the code, or prove the screen is reachable after all and re-file it. \
         If it is genuinely worth keeping, change this assertion deliberately and say why."
    );
}

/// **Every `group` an arm names is declared in `groups`.**
///
/// This exists because a merge ate five declarations without failing anything.
/// The keyed JSON driver merges the `arms` **array** by `id` — it does that
/// correctly, and it says so — and then takes one side's `groups` object and
/// `_note` wholesale. On the first concurrent rebase of this file that dropped
/// five group declarations and twenty-one lines of prose, in the file whose
/// entire purpose is counting, and every test above stayed green because every
/// one of them reads the `arms` array and nothing else.
///
/// A `group` is *"the unit somebody can claim complete, with an owner"*. An arm
/// filed under a group that does not exist has no owner and no completeness
/// flag, so it is exactly the arm that stops being counted.
#[test]
fn every_group_an_arm_names_is_declared() {
    let root = repo_root();
    let text = std::fs::read_to_string(root.join("docs/arms.json")).expect("docs/arms.json");

    // The `groups` object's keys are the two-space-indented `"name": {` lines
    // inside it, and the `arms` array's are six-space-indented `"group": "x"`.
    // A scan rather than a parser, for the reason `records` gives.
    let mut declared: BTreeSet<String> = BTreeSet::new();
    let mut in_groups = false;
    for line in text.lines() {
        if line.starts_with("  \"groups\": {") {
            in_groups = true;
            continue;
        }
        if in_groups {
            if line == "  }," || line == "  }" {
                in_groups = false;
                continue;
            }
            if let Some(rest) = line.strip_prefix("    \"") {
                if let Some((name, tail)) = rest.split_once("\":") {
                    if tail.trim_start().starts_with('{') {
                        declared.insert(name.to_string());
                    }
                }
            }
        }
    }
    assert!(!declared.is_empty(), "no groups parsed — the scanner lost the file");

    let mut used: BTreeSet<String> = BTreeSet::new();
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("\"group\": \"") {
            if let Some((value, _)) = rest.split_once('"') {
                used.insert(value.to_string());
            }
        }
    }
    assert!(used.len() > 1, "only {} groups used — the scanner lost the file", used.len());

    let undeclared: Vec<&String> = used.difference(&declared).collect();
    assert!(
        undeclared.is_empty(),
        "these arms name a group that docs/arms.json does not declare:\n  {}\n\
         A declaration carries the owner and the `complete` flag, so an arm without one is \
         an arm nobody can claim or finish. If a merge dropped them, put them back; the \
         keyed driver does not guard anything outside the `arms` array.",
        undeclared.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n  "),
    );

    // And the other way, which is the cheaper half of the same mistake: a group
    // nobody files under is a group whose arms went somewhere else.
    let unused: Vec<&String> = declared.difference(&used).collect();
    assert!(
        unused.is_empty(),
        "these groups are declared and no arm is filed under them:\n  {}",
        unused.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n  "),
    );
}

/// **The gesture, checked against the player's own `Lords2.exe`.**
///
/// This is the half that cannot be typed into agreement. Everything above
/// compares two things a person maintains; this one derives the answer from the
/// game, so a record that says `left-release` about a control the original
/// fires on the press goes red however carefully the record was written.
///
/// **How.** The interface is data. `Widget_Test` (`0x0040DA1E`) and
/// `Hotspot_Test` (`0x0040E3EE`) both walk arrays of 24-byte records with a
/// handler pointer at `+0x08` and a **kind byte at `+0x0F`**, and the kind byte
/// is the whole of what decides press, release, hold or repeat. So: scan the
/// two regions of `.data` the interface's tables live in, build handler address
/// -> kind, and cross it with `docs/arms.json`'s `addr`.
///
/// **What it does not cover, said in the same sentence as the number.** Only
/// records whose `addr` is a *table handler* — 39 of 211 today. An arm
/// dispatched from `Screen_FrameInput`'s own ladder reads
/// `g_mouseLeftPressed` / `g_mouseRightReleased` inline and has no kind byte to
/// read, so this check is silent about it. It is silent, not green: the
/// coverage is asserted below so that it cannot quietly fall to zero.
#[test]
fn the_gesture_of_every_table_handler_is_the_exes_own_kind_byte() {
    let exe = l2_testkit::executable!();

    // The two regions, and how they were established: the first is the lowest
    // hotspot table named by any `Hotspot_Test` call site in the decompilation
    // (`0x004DC4D0`, the six `FUN_00438B02` records), the last widget table is
    // the save/load scroll pair. The tables are laid out contiguously at a
    // 24-byte stride from the first, which is what makes a scan possible at
    // all — and what makes the two spot checks below necessary, because a
    // wrong base would decode plausible-looking rubbish rather than nothing.
    const HOTSPOTS: (u32, u32) = (0x004D_C4D0, 0x004D_D310);
    const WIDGETS: (u32, u32) = (0x004D_D310, 0x004D_E400);

    let mut kinds: BTreeMap<u32, BTreeSet<&'static str>> = BTreeMap::new();
    // **And the same thing keyed by the RECORD's address**, which is what lets
    // the prose check below reach the four arms whose `addr` is a dispatcher.
    // See `docs/input.md` §7a.
    let mut at_record: BTreeMap<u32, &'static str> = BTreeMap::new();
    // And which handler each record calls, so that a handler reachable at two
    // kinds can be settled by the one record an arm's own prose names.
    let mut handler_at: BTreeMap<u32, u32> = BTreeMap::new();
    for (widget, (lo, hi)) in [(false, HOTSPOTS), (true, WIDGETS)] {
        let mut va = lo;
        while va < hi {
            let t = l2_testkit::pe::Table::at(&exe, va);
            let handler = (0..4).fold(0u32, |a, i| a | (t.u8_at(8 + i) as u32) << (8 * i));
            let kind = t.u8_at(0x0F);
            if (0x0040_1000..0x004D_0000).contains(&handler) {
                if let Some(g) = gesture_of_kind(widget, kind) {
                    kinds.entry(handler).or_default().insert(g);
                    at_record.insert(va, g);
                    handler_at.insert(va, handler);
                }
            }
            va += 24;
        }
    }

    // **Two spot checks on the scan itself, before believing a word of it.**
    // A wrong base or stride decodes garbage that still parses, so pin one
    // record in each region against a handler this project has already named
    // from a call site. `Turn_End` is the End Turn strip; `Ui_ConfirmClicked`
    // is the yes/no box's pair of gauntlets.
    assert_eq!(
        kinds.get(&0x0043_AC23).map(|s| s.iter().copied().collect::<Vec<_>>()),
        Some(vec!["left-press"]),
        "the hotspot scan did not find Turn_End (0x0043AC23) at kind 1 — the base or the \
         stride is wrong and every verdict below it is rubbish",
    );
    assert_eq!(
        kinds.get(&0x0043_4E1F).map(|s| s.iter().copied().collect::<Vec<_>>()),
        Some(vec!["left-press-delayed"]),
        "the widget scan did not find Ui_ConfirmClicked (0x00434E1F) at kind 5",
    );

    let mut checked = 0usize;
    let mut wrong: Vec<String> = Vec::new();
    // A handler reachable from two tables with two different kinds cannot be
    // classified from its address alone. There is one, and it is named rather
    // than skipped silently.
    let mut ambiguous: BTreeSet<String> = BTreeSet::new();

    // **The prose half**, which is half the blind spot closed. See
    // `docs/input.md` §7a for why it is record bases only and why an ambiguity
    // is reported rather than resolved.
    let mut by_prose = 0usize;
    for r in records(&repo_root()) {
        let named: BTreeSet<&'static str> =
            r.prose.iter().filter_map(|a| at_record.get(a).copied()).collect();
        if named.is_empty() {
            continue;
        }
        if named.len() > 1 {
            ambiguous.insert(format!(
                "{} names tables of kinds {:?} in its prose",
                r.id,
                named.iter().copied().collect::<Vec<_>>(),
            ));
            continue;
        }
        by_prose += 1;
        let want = *named.iter().next().unwrap();
        if r.gesture != want {
            wrong.push(format!(
                "{}\n      docs/arms.json says `{}`\n      the kind byte at +0x0F of the \
                 widget record its own prose names says `{want}`",
                r.id, r.gesture,
            ));
        }
    }

    for r in records(&repo_root()) {
        let Some(addr) = r.addr else { continue };
        let Some(found) = kinds.get(&addr) else { continue };
        // **A handler in two tables at two kinds is settled by the record the
        // arm names**, when it names one and that record calls this handler.
        // `Opt_ToggleSpeech` and `Opt_ToggleAnimations` are each kind 5 in
        // their options table and kind 4 in the orphaned table at 0x004DDE08
        // that no call site tests; their records name the kind-5 record base,
        // and nothing about the address alone could have said which was meant.
        let named: BTreeSet<&'static str> = r
            .prose
            .iter()
            .filter(|va| handler_at.get(va) == Some(&addr))
            .filter_map(|va| at_record.get(va).copied())
            .collect();
        let found: BTreeSet<&'static str> =
            if found.len() > 1 && named.len() == 1 { named } else { found.clone() };
        if found.len() > 1 {
            ambiguous.insert(format!(
                "{addr:#010X} is in tables of kinds {:?}",
                found.iter().copied().collect::<Vec<_>>(),
            ));
            continue;
        }
        checked += 1;
        let want = *found.iter().next().unwrap();
        if r.gesture != want {
            wrong.push(format!(
                "{}\n      docs/arms.json says `{}`\n      the kind byte at +0x0F of \
                 {addr:#010X}'s widget record says `{want}`",
                r.id, r.gesture,
            ));
        }
    }

    assert!(
        wrong.is_empty(),
        "these arms are filed under a gesture the game's own widget tables contradict:\n  {}\n\
         The exe is right. `docs/input.md` has the kind vocabulary.",
        wrong.join("\n  "),
    );

    // **The coverage, asserted, because the interesting failure is outside the
    // set the check is exhaustive over.** If this number falls, the check went
    // quiet rather than green, and a quiet check reads exactly like a passing
    // one.
    // 37 until the options panels' twelve rows and the orphaned table beside
    // them were recorded; two of those twelve are only classifiable at all
    // because the resolution above reads the record base their prose names.
    assert!(
        checked >= 54,
        "only {checked} arms have a `addr` this check can classify; it was 54. \
         Either records lost their `addr`, or the table regions moved.",
    );
    // The prose half's coverage, asserted for the same reason: it reaches the
    // four arms whose `addr` is `Screen_HandleInput`, and if it fell silent it
    // would read exactly like passing. `docs/input.md` §7a.
    assert!(
        by_prose >= 31,
        "only {by_prose} arms name a widget record in their prose; it was 31. \
         A record that stops naming its table stops being classifiable.",
    );
    // **One handler is reachable at two different kinds**, and it is named
    // rather than skipped, because a growing list of things a check declines to
    // judge is itself the signal. `FUN_00432B05` is the multiplayer setup
    // page's four-row table (`0x004DCB48`): three of its records are kind 1 and
    // **the third is kind 3** — one row of one table waits for the release
    // while its neighbours fire on the press. Nothing about the address says
    // which row an arm means, so both of its arms go unjudged here.
    assert_eq!(
        ambiguous.iter().cloned().collect::<Vec<_>>(),
        vec!["0x00432B05 is in tables of kinds [\"left-press\", \"left-release\"]".to_string()],
        "the set of handlers reachable at two different kinds changed",
    );
}
