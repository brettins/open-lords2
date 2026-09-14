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
//! green; `tests/options.rs` caught it.
//! `left-press-repeat`, `left-press-delayed` or `left-press-held`**: those are
//! answered by nothing but a `Kind` handed to `Press`
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

mod verification;
pub use verification::*;

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
/// existed, an arm could be marked `reproduced`, be present, and be
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

/// **Every `press::Kind` by its identifier**, read off the type
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
/// A hand-rolled walk: this test must not add a dependency
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
// **Addresses seen since the last `"status"` line**, held
    // attached at once. A record is created on its `"status"` line, so its
    // `"id"` line arrives while the *previous* record is still `out.last_mut()`
    // — and attaching there silently gave `tax-and-ration-arrows` the supplies
    // thumb's address and a kind it does not have.
    let mut pending: BTreeSet<u32> = BTreeSet::new();
    for line in text.lines() {
        // **Only the prose fields**, and the exclusions are each a case that
        // went wrong before the list existed:
        //
// * `id` — an id's address is sometimes a **table base**
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
// filter at the use site is what makes the net safe
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
/// the whole reason this is a method. A
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
/// Adding a value here should cost a decision, so it is a list and
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
// make them unrecordable.
    "draw",
    "frame",
    "timer",
    "none",
];

/// The gesture a `Widget_Test` / `Hotspot_Test` kind byte means.
///
/// `widget` says which of the two testers walks the table: they use **the same
/// 24-byte record and different kind numbers**, so the tester has to
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
