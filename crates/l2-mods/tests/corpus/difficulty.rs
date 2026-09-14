#![allow(unused_imports)]
use super::*;
use super::indexing::*;
use super::seeding::*;
use common::TempDir;
use l2_mods::seed::{battle_names, parse_troops_eng, to_rules_toml, ROWS};
use l2_mods::{Platform, Ruleset, Side, TroopRules, Vfs};
use std::env;

/// **The four non-Normal difficulty groups in every shipped `TROOPS*.ENG` are
/// dead data, and they are not our curve.**
///
/// `docs/formats/eng.md` §3.2 says the game overwrites groups 0, 1, 3 and 4
/// with group 2 ("Normal") immediately after parsing and re-derives them as
/// `Normal * p / 100` for troop columns 0–6, leaving the four siege columns
/// alone. `l2_mods::seed::DIFFICULTIES` transcribes those percentages as 116,
/// 108, 100, 92 and 84.
///
/// This was written as an attempt to make the shipped files the *oracle* for
/// those five numbers, on the reasoning that `TROOPS.ENG` — the oldest of the
/// three, from before the layout change its own header announces — still has
/// all five groups filled in, so its rows would be the same arithmetic done by
/// hand. **They are not**, and the attempt is more useful for having failed:
///
/// * 299 of the 400 entries in each non-Normal group of `TROOPS.ENG` are
/// **zero**, so it is not a filled-in table either;
/// * of the ones that are not, the ratios to Normal are all over the place —
///   1.20, 1.222, 1.225, 1.233, 1.25, 1.266, 1.30, 1.33, 1.40, 1.50, 1.60, 2.00
///   in group 0 alone. They were authored by hand, per battle, and 116 % is
///   nowhere in them.
///
/// So the file corroborates §3.2's conclusion from the other side — these rows
/// are legacy content the engine discards — and it is **not** a source for the
/// curve. Anybody who "fixes" `DIFFICULTIES` to match the shipped data would be
/// undoing a correct reading; this test is here to say so before they try.
///
/// The curve's provenance therefore remains the decompilation alone. That is
/// recorded as an open item: see `docs/audit.md`.
#[test]
fn the_shipped_difficulty_rows_are_dead_data_and_not_the_engines_curve() {
    use l2_mods::seed::{COLUMNS, DIFFICULTIES, ROWS, SIDES};

    let dir = skip_without_install!();
    let mut vfs = Vfs::new();
    vfs.push_layer("base", &dir).unwrap();

    const NORMAL: usize = 2;
    let mut files = 0;
    for file in ["TROOPS.ENG", "TROOPS2.ENG", "TROOPS3.ENG"] {
        let Ok(bytes) = vfs.read(file) else { continue };
        let t = parse_troops_eng(&bytes).expect("parses");
        files += 1;

        let mut populated = 0usize;
        let mut agrees_with_our_curve = 0usize;
        let mut total = 0usize;
        let mut rows = std::collections::BTreeSet::new();
        for row in 0..ROWS {
            for side in 0..SIDES {
                let normal = t.counts[row][NORMAL][side];
                for (group, &(_, _, percent)) in DIFFICULTIES.iter().enumerate() {
                    if group == NORMAL {
                        continue;
                    }
                    for col in 0..COLUMNS {
                        let stored = t.counts[row][group][side][col];
                        total += 1;
                        if stored != 0 {
                            populated += 1;
                            rows.insert(row);
                        }
                        let derived =
                            if col < 7 { normal[col] * percent / 100 } else { normal[col] };
                        if stored == derived && stored != 0 {
                            agrees_with_our_curve += 1;
                        }
                    }
                }
            }
        }
        println!(
            "{file}: {populated}/{total} non-Normal entries populated in rows {rows:?}, \
             {agrees_with_our_curve} of them equal to Normal x our percentage"
        );
        // Most of the table is empty in every shipped file, which is what
        // "the engine derives it" looks like from the data side.
        assert!(
            populated * 4 < total,
            "{file}: {populated} of {total} non-Normal entries are populated, which is too \
             many for a table the engine overwrites"
        );
        // And what is populated is not our arithmetic. If this ever became
        // true the file would be a real oracle and this test should be
        // rewritten as one - which is a good failure to have.
        assert!(
            agrees_with_our_curve * 2 < populated.max(1),
            "{file}: the shipped rows now match l2_mods::seed::DIFFICULTIES; if that is real, \
             this file is an oracle for the curve and should be used as one"
        );
    }
    assert!(files > 0, "no TROOPS*.ENG in this install");
    println!("{files} shipped troops files checked");
}
