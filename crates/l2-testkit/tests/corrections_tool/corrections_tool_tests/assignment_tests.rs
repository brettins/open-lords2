#![allow(unused_imports)]
use super::*;
use super::validation_tests::*;
use super::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[test]
fn a_duplicate_in_any_series_is_an_error() {
    let Some(tree) = Tree::new("dupe") else { return };
    tree.append("docs/bugs.md", "\n### B2 — the same number as the row above\n");
    let out = tree.run(&["--check"]);
    assert!(!out.status.success(), "--check passed two B2s");
    assert!(err(&out).contains("B2 (reproduced bug) is used 2 times"), "{}", err(&out));
}

#[test]
fn assign_numbers_a_correction_across_the_tree_and_relocks() {
    let Some(tree) = Tree::new("assign") else { return };
    let (one, two) = (tag('C', "slug"), tag('C', "slug-two"));
    tree.append(
        "docs/decisions.md",
        &format!("\n**{one} — A new correction.**\n\nSee {one}.\n\n**{two} — Another.**\n\nNot {one}, but {two}.\n"),
    );
    tree.write("crates/x/src/new.rs", format!("// docs/decisions.md {one}; and {two}.\n"));
    tree.write("docs/data.json", format!("{{\"note\": \"correction {one}\"}}\n"));
    tree.relock();

    let out = tree.run(&["--assign", &one]);
    assert!(out.status.success(), "--assign {one} failed:\n{}", err(&out));
    assert!(err(&out).contains(&format!("{one} -> C3")), "{}", err(&out));
    let log = tree.text("docs/decisions.md");
    assert!(log.contains("**C3 — A new correction.**") && log.contains("See C3.") && log.contains("Not C3, but "), "{log}");
    assert_eq!(tree.text("crates/x/src/new.rs"), format!("// docs/decisions.md C3; and {two}.\n"));
    assert_eq!(tree.text("docs/data.json"), "{\"note\": \"correction C3\"}\n");

    let out = tree.run(&["--assign", &two]);
    assert!(out.status.success(), "--assign {two} failed:\n{}", err(&out));
    assert!(tree.text("docs/decisions.md").contains("**C4 — Another.**"));

    let out = tree.run(&["--check"]);
    assert!(out.status.success(), "the tree does not pass --check after both assignments:\n{}", err(&out));
}

#[test]
fn assign_numbers_a_dead_code_entry_in_its_own_series() {
    let Some(tree) = Tree::new("assign-d") else { return };
    let t = tag('D', "slider-widget");
    tree.append("docs/bugs.md", &format!("| **{t}** | A slider widget nothing instantiates. | [V] |\n"));
    tree.write("crates/x/tests/sfx.rs", format!("const DEAD: &str = \"{t}\";\n"));
    tree.relock();

    let out = tree.run(&["--assign", &t]);
    assert!(out.status.success(), "--assign {t} failed:\n{}", err(&out));
    assert!(tree.text("docs/bugs.md").contains("| **D3** | A slider widget"), "{}", tree.text("docs/bugs.md"));
    assert_eq!(tree.text("crates/x/tests/sfx.rs"), "const DEAD: &str = \"D3\";\n");
    assert!(tree.run(&["--check"]).status.success());
}

#[test]
fn assign_changes_only_the_tag_bytes() {
    let Some(tree) = Tree::new("bytes") else { return };
    let t = tag('C', "bytes");
    tree.append("docs/decisions.md", &format!("\n**{t} — Read and write bytes.**\n"));
    let original = |t: &str| -> Vec<u8> {
        let mut b = vec![0xEF, 0xBB, 0xBF];
        b.extend_from_slice("line one — an em-dash\r\n".as_bytes());
        b.extend_from_slice(&[0xFF, 0x97, b'\r', b'\n']);
        b.extend_from_slice(format!("see {t} here\r\n").as_bytes());
        b
    };
    tree.write("notes/crlf.txt", original(&t));
    tree.relock();

    let out = tree.run(&["--assign", &t]);
    assert!(out.status.success(), "--assign {t} failed:\n{}", err(&out));
    assert_eq!(tree.read("notes/crlf.txt"), original("C3"), "--assign changed bytes other than the tag");
}

/// * a tag that is not in the tree, or is not a placeholder at all;
/// * a tag that is cited and defined nowhere, so
///   number from;
/// * **a malformed heading anywhere in the logs** — the case that matters. An
///   invisible `## C3` leaves the next free number at 3, so assigning would
///   create a second C3. The refusal is what stops the tool computing a number
///   from a log it cannot fully read.
///
/// Ablated: running `--assign` before the malformed-heading report assigns C3
/// beside the invisible one, and the snapshot comparison fails.
#[test]
fn assign_refuses_and_changes_nothing() {
    let Some(tree) = Tree::new("refuse") else { return };
    let t = tag('C', "real");
    tree.append("docs/decisions.md", &format!("\n**{t} — A real placeholder.**\n"));
    tree.append("crates/x/src/lib.rs", &format!("// {}\n", tag('C', "cited-only")));
    tree.relock();

    let refusals: [(Vec<String>, &str); 4] = [
        (vec!["--assign".into(), tag('C', "absent")], "is not in the tree"),
        (vec!["--assign".into(), "C3".into()], "is not a placeholder"),
        (vec!["--assign".into()], "needs the placeholder"),
        (vec!["--assign".into(), tag('C', "cited-only")], "no entry defines it"),
    ];
    for (args, why) in refusals {
        let before = tree.snapshot();
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = tree.run(&args);
        assert!(!out.status.success() && err(&out).contains(why), "{args:?} was not refused with \"{why}\":\n{}", err(&out));
        assert!(tree.snapshot() == before, "{args:?} was refused and still changed the tree");
    }

    tree.append("docs/decisions.md", "\n## C3 — invisible to the old tool\n");
    let before = tree.snapshot();
    let out = tree.run(&["--assign", &t]);
    assert!(!out.status.success(), "--assign numbered a placeholder beside a heading it cannot read:\n{}", err(&out));
    assert!(err(&out).contains("look like a numbered entry"), "{}", err(&out));
    assert!(tree.snapshot() == before, "--assign refused and still changed the tree");
}


