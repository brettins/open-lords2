//! **`tools/draws/screens.json` against the screens, in both directions.**
//!
//! `docs/draws.md` is the draw-call audit and this is its half of the checks
//! that live in Rust. The other half is `tools/draws/screendraws.js --check`
//! (the stored count against the decompiled binary) and
//! `tools/figures/figures.js --check` (every quoted figure against the stored
//! count). Those two need the corpus or the documents; this one needs neither,
//! so it is the half that runs in CI on every push.
//!
//! # What it checks, and why this pair rather than a `// draw:` marker
//!
//! The pilot recommended **against** set-equality markers on draw calls — some
//! 230 lines across seven screens against 26 input arms, most carrying no
//! decision at all — and nothing since has changed that. `docs/draws.md` §11.
//!
//! What is worth set equality is much smaller and much sharper: **the English
//! captions we write in our own source.** In the original, every word a player
//! reads comes out of the player's own `L2.eng`; a string literal in a screen
//! module is therefore either
//!
//! * an honest diagnostic of ours — *"NO MINIMAP"*, *"NOT SIMULATED"* — which
//!   says the engine is incomplete, which is true; or
//! * **an invention**: words on a screen the original never puts there.
//!
//! There is no way to tell those apart mechanically, so the inventory record
//! names the first kind in `literals_ours` and the check is *set equality*
//! against what the module actually draws. A new literal that nobody defended
//! fails; a defence for a literal somebody deleted fails too.
//!
//! **The two artefacts are not maintained by the same work**, which is the
//! property `docs/agents.md` says makes a duplicate check worth having: the
//! record is written by whoever read the painter out of the decompilation, and
//! the source by whoever wrote the screen, usually months apart.
//!
//! # What it deliberately does not check
//!
//! **That a literal we kept is the right thing to keep.** A one-line reason in
//! `literals_ours` is a claim by a person and this test believes it. What it
//! guarantees is only that the list cannot grow *silently* — which is exactly
//! the failure `crates/l2-view/src/text.rs`'s stale header caused, where whole
//! screens were written in our 5 x 7 debug font because a comment said the real
//! font was not decoded yet and nothing anywhere counted the consequence.
//!
//! **That a screen with no record is fine.** A screen the audit has not reached
//! is *skipped and counted*, and the count is printed. A skip that is silent is
//! a skip that becomes permanent.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    // `CARGO_MANIFEST_DIR` is `<root>/crates/l2-game`.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/l2-game has two ancestors")
        .to_path_buf()
}

/// One screen's record, reduced to what this file needs.
#[derive(Default)]
struct Record {
    screen: String,
    module: Option<String>,
    /// `literals_ours`, each entry reduced to the quoted caption it defends.
    ///
    /// An entry reads `"NOT SIMULATED - an honest diagnostic"`, so the caption
    /// is everything before the first ` - `. Written that way rather than as a
    /// nested object because the file is read by people at least as often as by
    /// this test.
    literals_ours: BTreeSet<String>,
    /// Whether the record declared the field at all. An absent list and an
    /// empty one mean different things: *nobody looked* against *there are
    /// none*, and conflating them is how a skip becomes permanent.
    declared: bool,
}

/// Parse `tools/draws/screens.json` without pulling in a JSON crate.
///
/// The file is written by `screendraws.js --write` with `JSON.stringify(_, 2)`,
/// so every scalar is on its own line and every array element is too. The
/// scanner asserts it found a plausible number of records rather than trusting
/// that, because a file reformatted onto one line would otherwise parse as
/// nothing at all and pass.
fn records(root: &Path) -> Vec<Record> {
    let path = root.join("tools/draws/screens.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));

    let scalar = |line: &str, name: &str| -> Option<String> {
        let rest = line.trim().strip_prefix(&format!("\"{name}\":"))?;
        Some(rest.trim().trim_end_matches(',').trim().trim_matches('"').to_string())
    };

    let mut out: Vec<Record> = Vec::new();
    let mut in_literals = false;
    for line in text.lines() {
        let t = line.trim();
        if in_literals {
            if t.starts_with(']') {
                in_literals = false;
            } else if let Some(rec) = out.last_mut() {
                let s = t.trim_end_matches(',').trim().trim_matches('"');
                if !s.is_empty() {
                    rec.literals_ours.insert(caption_of(s));
                }
            }
            continue;
        }
        if let Some(v) = scalar(t, "screen") {
            out.push(Record { screen: v, ..Default::default() });
        } else if let Some(v) = scalar(t, "module") {
            if let Some(rec) = out.last_mut() {
                rec.module = (!v.is_empty() && v != "null").then_some(v);
            }
        } else if t.starts_with("\"literals_ours\":") {
            if let Some(rec) = out.last_mut() {
                rec.declared = true;
            }
            // `"literals_ours": []` closes on the same line.
            in_literals = !t.contains(']');
        }
    }
    assert!(
        out.len() >= 6,
        "only {} records parsed from {} — the scanner lost the file",
        out.len(),
        path.display()
    );
    out
}

/// The caption an entry defends: everything before the first spaced **em
/// dash**.
///
/// The separator is the em dash and nothing else, deliberately. The first
/// version also split on `" - "` and truncated two real captions that contain a
/// hyphen — *"NO ARM_GRID.PL8 - RACKS ARE RECTANGLES"* is a caption, not a
/// caption plus a verdict. A separator that can occur inside the thing it
/// separates is not a separator, which is the same objection that killed the
/// self-describing citation scheme in `docs/agents.md`: **a disambiguation
/// scheme derived from the thing being disambiguated can inherit its
/// ambiguity.**
///
/// An entry with no em dash is all caption and no verdict. That is allowed by
/// the parser and caught by the eye: a record that defends nothing reads as
/// obviously unfinished.
fn caption_of(entry: &str) -> String {
    let end = entry.find(" \u{2014} ").unwrap_or(entry.len());
    entry[..end].trim().trim_matches('"').to_string()
}

/// The five call sites that put *our own letters* on the canvas, and the
/// literal caption each is given.
///
/// Pinned by name from `crates/l2-view/src/text.rs` and `crates/l2-game/src/
/// widget.rs` rather than matched by a pattern: a pattern over `draw` catches
/// `draw_scene`, `draw_misc` and `draw_system`, every one of which blits a
/// frame out of a `.pl8` the player owns and is therefore the *right* thing.
const OUR_LETTERS: [&str; 5] = [
    "text::draw",
    "text::draw_centred",
    "text::draw_right",
    "widget::button",
    "widget::label",
];

/// Everything outside a comment, with comments replaced by spaces so that byte
/// offsets and line breaks are preserved.
///
/// Comments must go before the scan rather than during it, because a call can
/// be split across lines by `rustfmt` and a per-line filter cannot see that.
/// This file's own module docs quote several of the captions it hunts for, so
/// getting this wrong makes the check match itself.
fn strip_comments(src: &str) -> String {
    let b: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let (mut i, mut in_str, mut in_line, mut in_block) = (0, false, false, 0usize);
    while i < b.len() {
        let c = b[i];
        let next = b.get(i + 1).copied().unwrap_or('\0');
        if in_line {
            if c == '\n' { in_line = false; out.push('\n'); } else { out.push(' '); }
            i += 1;
        } else if in_block > 0 {
            if c == '*' && next == '/' { in_block -= 1; out.push_str("  "); i += 2; }
            else if c == '/' && next == '*' { in_block += 1; out.push_str("  "); i += 2; }
            else { out.push(if c == '\n' { '\n' } else { ' ' }); i += 1; }
        } else if in_str {
            // Keep the string, escapes and all — the caption lives in here.
            out.push(c);
            if c == '\\' { if let Some(&e) = b.get(i + 1) { out.push(e); i += 2; continue; } }
            if c == '"' { in_str = false; }
            i += 1;
        } else if c == '/' && next == '/' {
            in_line = true;
        } else if c == '/' && next == '*' {
            in_block = 1; out.push_str("  "); i += 2;
        } else {
            if c == '"' { in_str = true; }
            out.push(c);
            i += 1;
        }
    }
    out
}

/// Every English caption a module hands to one of [`OUR_LETTERS`].
///
/// A `{}` placeholder is kept, because `"WILL TAKE {work} MEN"` is a sentence
/// we wrote whether or not one word of it is substituted.
///
/// **The known hole, stated so it does not get trusted past.** A caption that
/// never appears as a literal — `text::draw(canvas, x, y, &format!("{}", n))`
/// with the words in a variable built elsewhere, or a `&'static str` table
/// consumed through a binding — is invisible to this. It scans for a quoted run
/// between a draw call and its `;`, which catches a `rustfmt`-split call and
/// does not catch an indirected one. Two modules in the first audit had words
/// on screen that reach `text::draw` through a `format!` result and declared
/// `[]` here; both say so in their record's `notes`. **A silent skip becomes
/// permanent**, so those are written down rather than left to the scanner.
fn literals_in(src: &str) -> BTreeSet<String> {
    let code = strip_comments(src);
    let mut out = BTreeSet::new();
    let mut from = 0usize;
    while from < code.len() {
        // The next draw call, whichever of the five comes first.
        let Some(at) = OUR_LETTERS
            .iter()
            .filter_map(|c| code[from..].find(c).map(|i| (i + from, c.len())))
            .min_by_key(|&(i, _)| i)
        else {
            break;
        };
        let (open, len) = at;
        // The call's arguments end at its statement terminator. A caption after
        // that belongs to the next call, not this one.
        let rest = &code[open + len..];
        let end = rest.find(';').unwrap_or(rest.len());
        let args = &rest[..end];
        if let Some(a) = args.find('"') {
            if let Some(b) = args[a + 1..].find('"') {
                let s = &args[a + 1..a + 1 + b];
                if s.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
                    out.insert(s.to_string());
                }
            }
        }
        from = open + len;
    }
    out
}

fn screens_dir(root: &Path) -> PathBuf {
    root.join("crates/l2-game/src/screens")
}

#[test]
fn every_english_caption_we_draw_is_one_the_inventory_defends() {
    let root = repo_root();
    let recs = records(&root);

    let mut skipped: Vec<String> = Vec::new();
    let mut undefended: Vec<String> = Vec::new();
    let mut stale: Vec<String> = Vec::new();
    // A module can serve two screens (`0x35` and `0x36` are one painter and one
    // flag), so a caption only has to be defended by *one* of its records.
    let mut by_module: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut declared_for: BTreeSet<String> = BTreeSet::new();

    for rec in &recs {
        let Some(module) = rec.module.clone() else { continue };
        if rec.declared {
            declared_for.insert(module.clone());
            by_module.entry(module).or_default().extend(rec.literals_ours.iter().cloned());
        } else {
            skipped.push(format!("{} ({module}) declares no literals_ours", rec.screen));
        }
    }

    for module in &declared_for {
        let path = screens_dir(&root).join(module);
        let Ok(src) = std::fs::read_to_string(&path) else {
            panic!("{} names module {module}, which does not exist", path.display())
        };
        let drawn = literals_in(&src);
        let defended = by_module.get(module).cloned().unwrap_or_default();
        for s in drawn.difference(&defended) {
            undefended.push(format!("{module}: draws \"{s}\" and no record defends it"));
        }
        for s in defended.difference(&drawn) {
            stale.push(format!("{module}: a record defends \"{s}\" and nothing draws it"));
        }
    }

    // Printed, never asserted at a number: a threshold chosen from a passing
    // tree is a threshold that describes the tree. `docs/agents.md`, the
    // build-stamp test.
    println!(
        "draws: {} screens in the inventory, {} modules with a defended caption list, \
         {} without",
        recs.len(),
        declared_for.len(),
        skipped.len()
    );
    for s in &skipped {
        println!("  SKIP {s}");
    }

    assert!(
        undefended.is_empty() && stale.is_empty(),
        "the captions our screens draw and the ones tools/draws/screens.json defends \
         have come apart.\n\n\
         An English caption in a screen module is either an honest diagnostic of ours \
         or an invention — words on a screen the original never puts there. Either way \
         it belongs in that screen's `literals_ours` with a one-line reason, or it \
         belongs in `pen.eng(canvas, GROUP, INDEX, ..)` instead.\n\n\
         undefended:\n  {}\n\nstale:\n  {}",
        undefended.join("\n  "),
        stale.join("\n  "),
    );
}

#[test]
fn every_screen_module_the_inventory_names_exists() {
    let root = repo_root();
    let dir = screens_dir(&root);
    let mut bad = Vec::new();
    for rec in records(&root) {
        let Some(module) = rec.module else { continue };
        if !dir.join(&module).exists() {
            bad.push(format!("{} names {module}", rec.screen));
        }
    }
    assert!(
        bad.is_empty(),
        "tools/draws/screens.json names modules that are not there — a rename took the \
         inventory's pointer with it:\n  {}",
        bad.join("\n  ")
    );
}

/// **Minus one is singular**, which `Ui_DrawCount` says in three arms and we
/// said in two.
///
/// This is here rather than in `tests/text.rs` because it is a finding of the
/// draw-call audit and it is the audit's argument in miniature: the defect was
/// in a *primitive*, not in a screen, so no amount of reading screens would
/// have found it — and every screen that draws a signed quantity inherited it.
///
/// It needs no install and no canvas because `shell::count_noun` is a free
/// function, which is the shape that made it testable at all. Ablated:
/// restoring `value == 1` turns the second assertion red and nothing else.
#[test]
fn ui_drawcount_takes_the_singular_for_minus_one_as_well_as_one() {
    use l2_game::shell::count_noun;
    // `L2.eng` group 8 is singular at even indices and plural at odd, so the
    // noun index passed in *is* the singular and `+ 1` is the plural. Pinned
    // to 0/1, *"Crown."* / *"Crowns."*, which `shell.rs` asserts against the
    // player's own file.
    const CROWN: usize = 0;
    assert_eq!(count_noun(1, CROWN), CROWN, "one crown is singular");
    assert_eq!(count_noun(-1, CROWN), CROWN, "and so is minus one — 0x0041AB67's second arm");
    assert_eq!(count_noun(0, CROWN), CROWN + 1, "nought is plural, as in \"0 Crowns.\"");
    assert_eq!(count_noun(2, CROWN), CROWN + 1);
    assert_eq!(count_noun(-2, CROWN), CROWN + 1, "and minus two is plural again");
}

/// **The audit's coverage, printed rather than gated.**
///
/// `docs/draws.md` §6: seven screens is not enough to know what a normal ratio
/// is, and a gate set from seven samples is a gate set from the status quo. So
/// this prints which screen modules the inventory has not reached, and asserts
/// nothing about how many there are.
#[test]
fn which_screens_the_draw_audit_has_not_reached() {
    let root = repo_root();
    let covered: BTreeSet<String> =
        records(&root).into_iter().filter_map(|r| r.module).collect();
    let mut missing: Vec<String> = Vec::new();
    let Ok(entries) = std::fs::read_dir(screens_dir(&root)) else { return };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if !name.ends_with(".rs") || name == "mod.rs" {
            continue;
        }
        if !covered.contains(&name) {
            missing.push(name);
        }
    }
    missing.sort();
    println!(
        "draws: {} of {} screen modules are in the inventory; not yet enumerated: {}",
        covered.len(),
        covered.len() + missing.len(),
        if missing.is_empty() { "none".into() } else { missing.join(", ") }
    );
}
