#![allow(unused_imports)]
use super::*;
use super::assignment_tests::*;
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

