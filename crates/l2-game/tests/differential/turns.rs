#![allow(unused_imports)]
use super::*;

use super::compare::{clock, run, Row};

#[test]
fn the_pairs_are_one_end_turn_apart() {
    if l2_testkit::fixtures_dir().is_none() {
        l2_testkit::skip!("no fixture directory");
    }
    let mut lines = Vec::new();
    for pair in PAIRS {
        let (Ok(b), Ok(a)) =
            (l2_testkit::fixture_save(pair.before), l2_testkit::fixture_save(pair.after))
        else {
            l2_testkit::skip!("{} or {} is not in the fixture directory", pair.before, pair.after);
        };
        let (bt, bs, by) = clock(&b);
        let (at, as_, ay) = clock(&a);
        lines.push(format!(
            "  {:<14} {:<20} turn {bt:>2} season {bs} {by}  ->  {:<20} turn {at:>2} season {as_} {ay}",
            pair.label, pair.before, pair.after
        ));
        assert_eq!(
            at - bt,
            1,
            "{} -> {} is {} turns apart, not one; it is not an End Turn pair and must not be \
             used as one",
            pair.before,
            pair.after,
            at - bt
        );
        assert_eq!(
            (bs % 4) + 1,
            as_,
            "{} -> {}: the season did not step by one",
            pair.before,
            pair.after
        );
        assert_eq!(
            ay - by,
            i32::from(as_ == 4),
            "{} -> {}: the year moved other than into Winter",
            pair.before,
            pair.after
        );
    }
    eprintln!("\nthe one-End-Turn pairs:\n{}\n", lines.join("\n"));
}

#[test]
fn the_named_before_and_after_saves_are_the_same_turn() {
    if l2_testkit::fixtures_dir().is_none() {
        l2_testkit::skip!("no fixture directory");
    }
    for triple in SAME_TURN_TRIPLES {
        let mut seen = Vec::new();
        for name in triple {
            let Ok(s) = l2_testkit::fixture_save(name) else {
                l2_testkit::skip!("{name} is not in the fixture directory");
            };
            seen.push((*name, clock(&s)));
        }
        let first = seen[0].1;
        for (name, c) in &seen {
            assert_eq!(
                *c, first,
                "{name} reads turn/season/year {c:?} where {} reads {first:?} - if these three \
                 have stopped being one moment caught three times, PAIRS and \
                 crates/l2-game/tests/seam/main.rs both want re-reading",
                seen[0].0
            );
        }
        eprintln!(
            "{:?} are all turn {} season {} year {} - one turn, not a before/after pair",
            triple, first.0, first.1, first.2
        );
    }
}

#[test]
fn one_end_turn_of_ours_against_one_end_turn_of_the_originals() {
    if l2_testkit::fixtures_dir().is_none() {
        l2_testkit::skip!("no fixture directory");
    }

    let mut reports = Vec::new();
    for pair in PAIRS {
        match run(pair) {
            Ok(r) => reports.push((pair.label, r)),
            Err(why) => l2_testkit::skip!("{why}"),
        }
    }

    let mut compared_total = 0usize;
    let mut agree_total = 0usize;
    let mut moved_total = 0usize;
    let mut moved_agree_total = 0usize;
    let mut by_kind: std::collections::BTreeMap<&str, (usize, usize)> = Default::default();
    let mut found: std::collections::BTreeMap<(&str, &str), usize> = Default::default();

    for (label, r) in &reports {
        compared_total += r.compared;
        agree_total += r.agree;
        moved_total += r.moved;
        moved_agree_total += r.moved_agree;
        eprintln!(
            "\n=== {label}: {} compared, {} agree; of the {} the original's turn MOVED, {} agree",
            r.compared, r.agree, r.moved, r.moved_agree
        );
        let mut rows: Vec<&Row> = r.diverged.iter().collect();
        rows.sort_by_key(|row| (row.kind, -row.delta().abs(), row.path, row.record.clone()));
        for row in rows {
            let e = by_kind.entry(row.kind.tag()).or_default();
            e.0 += 1;
            if row.moved() {
                e.1 += 1;
            }
            *found.entry((row.pair.as_str(), row.path)).or_default() += 1;
            eprintln!(
                "  {:<12} {:<26} {:<14} was {:>8}   original {:>8}   ours {:>8}   delta {:>+9}{}",
                row.kind.tag(),
                row.path,
                row.record,
                row.was,
                row.theirs,
                row.ours,
                row.delta(),
                if row.moved() { "  [moved]" } else { "" }
            );
        }
    }

    eprintln!("\n--- divergences by kind (total, of which the original moved) ---");
    for (kind, (n, m)) in &by_kind {
        eprintln!("  {n:>4}  {kind:<13} ({m} moved)");
    }
    let pct = |a: usize, b: usize| if b == 0 { 0 } else { a * 100 / b };
    eprintln!(
        "\n{compared_total} fields compared across {} pairs, {agree_total} agree ({}%)\n\
         of those, {moved_total} are fields the original's own End Turn MOVED: {moved_agree_total} agree ({}%)\n\
         -- the second line is the one that means anything; an inert field agrees for free.\n",
        reports.len(),
        pct(agree_total, compared_total),
        pct(moved_agree_total, moved_total),
    );

    let expected: std::collections::BTreeMap<(&str, &str), usize> =
        BASELINE.iter().map(|&(p, f, n)| ((p, f), n)).collect();
    if found != expected {
        let mut block = String::new();
        for ((pair, path), n) in &found {
            block.push_str(&format!("    (\"{pair}\", \"{path}\", {n}),\n"));
        }
        panic!(
            "the divergence baseline has moved.\n\n\
             This is a measuring instrument. Do NOT weaken a comparison or exclude a field to \
             make it green: if the change improved agreement, say so by moving the numbers up. \
             Replace BASELINE in crates/l2-game/tests/differential/main.rs with:\n\n\
             const BASELINE: &[(&str, &str, usize)] = &[\n{block}];\n\n\
             and set COMPARED_TOTAL = {compared_total}, AGREE_TOTAL = {agree_total}, \
             MOVED_TOTAL = {moved_total}, MOVED_AGREE_TOTAL = {moved_agree_total}."
        );
    }
    assert_eq!(compared_total, COMPARED_TOTAL, "the number of fields compared has moved");
    assert_eq!(agree_total, AGREE_TOTAL, "the number of fields that agree has moved");
    assert_eq!(moved_total, MOVED_TOTAL, "the number of fields the original's turn moved has moved");
    assert_eq!(
        moved_agree_total, MOVED_AGREE_TOTAL,
        "agreement on the fields the original's turn moved has changed - this is the number that \
         means something, so say which way it went and why"
    );
}

