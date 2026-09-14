#![allow(unused_imports)]
use super::*;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// **The base tree passes**, so every failure below is the line it adds.
///
/// Also pins the two D-series as two series: `D1` in `decisions.md` and `D1` in
/// `bugs.md` are not a duplicate, and the report gives each its own next number.
///
/// Ablated: keying duplicates by id alone, without the log, fails this — and
/// every other test in the file, because every tree here carries D1 in both
/// logs. Taking the next free number by letter alone fails
/// `assign_numbers_a_dead_code_entry_in_its_own_series` instead.
#[test]
fn the_base_tree_passes_and_its_two_d_series_are_two_series() {
    let Some(tree) = Tree::new("base") else { return };
    let out = tree.run(&["--check"]);
    assert!(out.status.success(), "the base tree fails --check:\n{}", err(&out));
    assert!(err(&out).contains("2 corrections (C1..C2)"), "{}", err(&out));

    let report = err(&tree.run(&[]));
    for next in ["C3", "D5 (decision", "B4 (reproduced bug", "N2 (bug not reproduced", "S2 (surprising", "D3 (dead code"] {
        assert!(report.contains(next), "the report does not give `{next}` as a next free id:\n{report}");
    }
}

/// **A Markdown heading is an error naming its line, not a skip.**
///
/// Two branches wrote their correction as a `##` heading over a placeholder, and
/// the tool recognised only `**C<n> — title**`, so the integrator rewrote both by
/// hand. The numbered shape is the dangerous one: nothing cites a new correction
/// yet, so an invisible `## C3` passed every rule, and the next free number was
/// still 3.
///
/// Ablated: without the `SUSPECT` scan in `readLogs`, `## C3` passes `--check`
/// and the placeholder shapes fail only as "unassigned", never naming a heading.
#[test]
fn a_markdown_heading_is_an_error_naming_its_line() {
    for line in [
        "## C3 — A correction written as a heading".to_string(),
        format!("## {}", tag('C', "slug")),
        format!("### **{} — the bold inside a heading**", tag('C', "arms-county")),
    ] {
        let Some(tree) = Tree::new("markdown-heading") else { return };
        tree.append("docs/decisions.md", &format!("\n{line}\n\nIts prose.\n"));
        assert_malformed(&tree, "docs/decisions.md", &line, "Markdown heading");
    }
    // And in bugs.md, where headings are the form: the wrong level is still wrong.
    let Some(tree) = Tree::new("markdown-level") else { return };
    tree.append("docs/bugs.md", "\n## B4 — a bug at the wrong level\n");
    assert_malformed(&tree, "docs/bugs.md", "## B4", "level-2 heading");
}

/// **A missing em-dash is an error naming its line.**
///
/// Ablated: without the `SUSPECT` scan, every one of these passes `--check`.
#[test]
fn a_heading_without_its_em_dash_is_an_error_naming_its_line() {
    let cases = [
        ("docs/decisions.md", "**C3 - a hyphen**", "hyphen"),
        ("docs/decisions.md", "**C3 – an en-dash**", "en-dash"),
        ("docs/decisions.md", "**C3: a colon**", "colon"),
        ("docs/decisions.md", "**C3—no spaces**", "spacing"),
        ("docs/decisions.md", "**C3**", "nothing after the id"),
        ("docs/bugs.md", "### B4 - a hyphen", "hyphen"),
        ("docs/bugs.md", "| **B4**| a cell closed tight |", "nothing after the id"),
    ];
    for (file, line, why) in cases {
        let Some(tree) = Tree::new("dash") else { return };
        tree.append(file, &format!("\n{line}\n"));
        assert_malformed(&tree, file, line, why);
    }
    // A placeholder with no dash is the same error.
    let Some(tree) = Tree::new("dash-placeholder") else { return };
    let line = format!("### {}: a colon", tag('B', "colon"));
    tree.append("docs/bugs.md", &format!("\n{line}\n"));
    assert_malformed(&tree, "docs/bugs.md", &line, "colon");
}

/// **A double-encoded em-dash is an error, and it says so.**
///
/// C146's heading carried these exact bytes. The old tool parsed nothing on
/// that line, so the correction did not exist; it was noticed only because three
/// citations then dangled. A new correction has no citations, so it would not
/// have been noticed at all.
///
/// Ablated: without the `SUSPECT` scan, this passes `--check`.
#[test]
fn a_double_encoded_em_dash_is_an_error_that_names_the_encoding() {
    let Some(tree) = Tree::new("mojibake") else { return };
    let mut bytes = tree.read("docs/decisions.md");
    bytes.extend_from_slice(b"\n**C3 ");
    bytes.extend_from_slice(&MANGLED_EM_DASH);
    bytes.extend_from_slice(b" The differential's first finding.**\n");
    tree.write("docs/decisions.md", bytes);
    assert_malformed(&tree, "docs/decisions.md", "The differential's first finding", "double-encoded");
}

/// **Prose that opens with an id is not a heading.**
///
/// Both logs do it, and the real ones carry every shape below. A check that
/// cried wolf at them would be switched off within a day.
///
/// Ablated: treating every bold line that opens with an id as an attempted
/// entry fails this on `**C1's failure mode…`.
#[test]
fn prose_that_opens_with_an_id_is_not_a_heading() {
    let Some(tree) = Tree::new("prose") else { return };
    tree.append(
        "docs/decisions.md",
        "\n**C1's failure mode is not rare.**\n\n**C2 was right and it was half the fix.**\n",
    );
    tree.append(
        "docs/bugs.md",
        "\n**B1** the harvest weather, **B2**/**B3** the ale.\n\n\
         **D1, the score gold bracket, and only D1.**\n\n\
         | | ruleset? |\n|---|---|\n| **D1** gold bracket | yes |\n| **B1** harvest weather | no |\n\n\
         | **D1 →** | a pointer to §5, not an entry |\n",
    );
    tree.relock();
    let out = tree.run(&["--check"]);
    assert!(out.status.success(), "--check flagged prose as a heading:\n{}", err(&out));
}

/// **Every placeholder family is reported**, and so is a letter no log numbers.
///
/// The tool once matched `[CB]NEW-` and nothing else, so a `D` placeholder for a
/// dead-code entry went unreported and was numbered by hand.
///
/// Ablated: narrowing `PLACEHOLDER` back to `[CB]` fails this, and fails the
/// dead-code assignment too, whose `D` tag's uses are then never found to
/// rewrite. Entries the logs define are still listed under that ablation —
/// they come from the log parse, not the scan — so the stray `X` tag is the
/// assertion that carries it.
#[test]
fn every_placeholder_family_is_reported_with_the_number_it_would_take() {
    let Some(tree) = Tree::new("families") else { return };
    tree.append(
        "docs/decisions.md",
        &format!("\n**{} — a decision.**\n\n**{} — a correction.**\n", tag('D', "dec"), tag('C', "corr")),
    );
    tree.append(
        "docs/bugs.md",
        &format!(
            "\n### {} — a bug\n\n| **{}** | not reproduced |\n\n### {} — surprising\n\n| **{}** | dead |\n",
            tag('B', "bug"),
            tag('N', "notrep"),
            tag('S', "surprise"),
            tag('D', "dead"),
        ),
    );
    tree.append(
        "crates/x/src/lib.rs",
        &format!("// {} and {}\n", tag('X', "stray"), tag('B', "orphan")),
    );
    tree.relock();

    let out = tree.run(&["--check"]);
    let e = err(&out);
    assert!(!out.status.success(), "--check passed a tree full of placeholders");
    for (t, becomes) in [
        (tag('D', "dec"), "D5"),
        (tag('C', "corr"), "C3"),
        (tag('B', "bug"), "B4"),
        (tag('N', "notrep"), "N2"),
        (tag('S', "surprise"), "S2"),
        (tag('D', "dead"), "D3"),
    ] {
        let row = e.lines().skip_while(|l| !l.contains(&format!("{t}  "))).nth(1).unwrap_or("");
        assert!(
            row.contains(&format!("--assign {t}")) && row.contains(&format!("take {becomes})")),
            "{t} is not reported with the command that assigns it and the number {becomes}:\n{e}"
        );
    }
    assert!(e.contains(&tag('X', "stray")) && e.contains("X is not a series"), "{e}");
    assert!(e.contains(&tag('B', "orphan")) && e.contains("no entry defines it"), "{e}");
    assert!(e.contains("Every other rule passed"), "{e}");
}

/// **A duplicate in any series is an error**, not only in the corrections.
///
/// `docs/bugs.md` carried two `B69`s on `main` — a table row and a heading,
/// fifty minutes apart — and the quirks catalogue, the only reader of that file
/// as data, held one disposition for both and passed. The first run of this
/// rule found it.
///
/// Ablated: restricting the duplicate check to the C-series passes this.
#[test]
fn a_duplicate_in_any_series_is_an_error() {
    let Some(tree) = Tree::new("dupe") else { return };
    tree.append("docs/bugs.md", "\n### B2 — the same number as the row above\n");
    let out = tree.run(&["--check"]);
    assert!(!out.status.success(), "--check passed two B2s");
    assert!(err(&out).contains("B2 (reproduced bug) is used 2 times"), "{}", err(&out));
}

/// **`--assign` numbers a correction everywhere and relocks.**
///
/// The tag is replaced in the heading, the log's prose, a source comment and a
/// JSON string; a longer tag that begins with it is left alone and then assigned
/// in its own turn; and the tree passes `--check` afterwards, which it cannot do
/// unless the lock was rewritten, because two citations were created.
///
/// Ablated: skipping the relock fails the final `--check` with a stale lock;
/// dropping the hyphen clause from `replaceBytes`' boundary replaces the prefix
/// of the longer tag, and the scan-versus-bytes count refuses the write.
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

/// **A dead-code placeholder is numbered in its own log's series, not by its
/// letter.** `decisions.md` runs D1..D4 and `bugs.md`'s dead code D1..D2, so the
/// right answer is D3 and the letter-only answer is D5.
///
/// Ablated: taking the series from the letter alone assigns D5.
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

/// **`--assign` reads and writes bytes.**
///
/// The integrator's `perl -pi` had no encoding layer, and that is the mechanism
/// that double-encodes UTF-8. So a file carrying a BOM, CRLF endings, an
/// em-dash, and two bytes that are not UTF-8 at all must come back identical
/// except for the tag.
///
/// Ablated: reading and writing the file as a UTF-8 string turns `0xFF` and
/// `0x97` into `ef bf bd` and fails the byte comparison.
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

/// **`--assign` refuses, and changes nothing, when it cannot be right.**
///
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

