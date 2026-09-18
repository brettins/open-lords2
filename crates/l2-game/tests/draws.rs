//! What is worth set equality is much smaller and much sharper: **the English
//! captions we write in our own source.** In the original, every word a player
//! reads comes out of the player's own `L2.eng`; a string literal in a screen
//! module is therefore either
//!, so the inventory record

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/l2-game has two ancestors")
        .to_path_buf()
}

#[derive(Default)]
struct Record {
    screen: String,
    module: Option<String>,
    literals_ours: BTreeSet<String>,
    declared: bool,
}

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

fn caption_of(entry: &str) -> String {
    let end = entry.find(" \u{2014} ").unwrap_or(entry.len());
    entry[..end].trim().trim_matches('"').to_string()
}

const OUR_LETTERS: [&str; 5] = [
    "text::draw",
    "text::draw_centred",
    "text::draw_right",
    "widget::button",
    "widget::label",
];

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

fn literals_in(src: &str) -> BTreeSet<String> {
    let code = strip_comments(src);
    let mut out = BTreeSet::new();
    let mut from = 0usize;
    while from < code.len() {
        let Some(at) = OUR_LETTERS
            .iter()
            .filter_map(|c| code[from..].find(c).map(|i| (i + from, c.len())))
            .min_by_key(|&(i, _)| i)
        else {
            break;
        };
        let (open, len) = at;
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

fn module_source(dir: &Path, module: &str) -> Option<String> {
    let file = dir.join(module);
    if file.is_file() {
        return std::fs::read_to_string(&file).ok();
    }
    let sub = dir.join(module.trim_end_matches(".rs"));
    if !sub.is_dir() {
        return None;
    }
    fn walk(d: &Path, out: &mut String) {
        let mut entries: Vec<_> = std::fs::read_dir(d).unwrap().flatten().map(|e| e.path()).collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|e| e == "rs") {
                out.push_str(&std::fs::read_to_string(&p).unwrap());
                out.push('\n');
            }
        }
    }
    let mut out = String::new();
    walk(&sub, &mut out);
    Some(out)
}

#[test]
fn every_english_caption_we_draw_is_one_the_inventory_defends() {
    let root = repo_root();
    let recs = records(&root);

    let mut skipped: Vec<String> = Vec::new();
    let mut undefended: Vec<String> = Vec::new();
    let mut stale: Vec<String> = Vec::new();
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
        let Some(src) = module_source(&screens_dir(&root), module) else {
            panic!("{} names module {module}, which does not exist", screens_dir(&root).display())
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
        if module_source(&dir, &module).is_none() {
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
