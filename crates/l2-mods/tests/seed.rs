//! Seeding the base ruleset from the player's own game files.
//!
//! The fixtures here are synthetic — written by this test, shaped like the
//! real files. No game data is in this repository, and these tests run on a
//! bare checkout.

use l2_mods::seed::{
    battle_names, parse_troops_eng, slug, to_rules_toml, SeedError, COLUMNS, DIFFICULTIES, GROUPS,
    NORMAL_GROUP, ROWS, SIDES, TOKEN_COUNT,
};
use l2_mods::{Ruleset, Side, TroopRules};

/// A file shaped like `TROOPS*.ENG`: a comment header, then 35 `*`-marked
/// blocks of `1 + 5*2*11` numbers. Every value is `row*1000 + n`, so a
/// misalignment of even one token is visible.
fn synthetic_troops_eng() -> Vec<u8> {
    let mut out = String::from(
        "Data file, to enable troop number modeling for Lords2 skirmish mode.\n\
         Only alter the NUMBERS below and keep them in the current format\n\
         Note the 1997 and 42 in this header must be ignored by the parser.\n\n\
         Pe   Xb   Ma   Sw   Pi   Ar   Kn   Ca   To   Ra   Oi\n",
    );
    for row in 0..ROWS {
        // The real markers are `*Map one`, spelled out - deliberately free of
        // digits, because the engine's reader would otherwise eat them.
        out.push_str("*Map - attacker then defender, five difficulty groups\n");
        // Defensive advantage, deliberately over 10 on one row.
        out.push_str(&format!("{}\n", if row == 3 { 40 } else { row % 11 }));
        for g in 0..GROUPS {
            out.push_str("Very easy - or whatever this row is\n");
            for s in 0..SIDES {
                for c in 0..COLUMNS {
                    // Siege columns must stay <= 9, as in every shipped file.
                    let v = if c >= 7 { (row + g + s + c) % 10 } else { row * 100 + g * 10 + s + c };
                    out.push_str(&format!("{v} "));
                }
                out.push('\n');
            }
        }
    }
    out.into_bytes()
}

fn synthetic_battles_eng() -> Vec<u8> {
    let mut out = String::new();
    for row in 0..ROWS {
        out.push_str(&format!("Battle {}\r\n", row + 1));
        out.push_str(&format!("The Battle of Place {}\r\n", row + 1));
        out.push_str("A description that nothing here reads.\r\n\r\n");
    }
    out.into_bytes()
}

#[test]
fn the_token_arithmetic_closes_at_3885() {
    assert_eq!(TOKEN_COUNT, 3885);
    assert_eq!(ROWS * (1 + GROUPS * SIDES * COLUMNS), 3885);
}

#[test]
fn a_troops_file_parses_into_the_documented_shape() {
    let table = parse_troops_eng(&synthetic_troops_eng()).expect("parses");
    assert_eq!(table.counts[0][0][0][0], 0);
    assert_eq!(table.counts[9][2][1][5], 9 * 100 + 2 * 10 + 1 + 5);
    assert_eq!(table.counts[34][4][1][6], 34 * 100 + 4 * 10 + 1 + 6);
    // The advantage is clamped on read, exactly as the engine clamps it.
    assert_eq!(table.advantage[3], 10);
    assert_eq!(table.advantage[5], 5);
}

#[test]
fn a_file_that_does_not_close_is_rejected_rather_than_half_read() {
    let mut bytes = synthetic_troops_eng();
    bytes.extend_from_slice(b"\n99\n");
    match parse_troops_eng(&bytes) {
        Err(SeedError::TokenCount { found, expected }) => {
            assert_eq!((found, expected), (TOKEN_COUNT + 1, TOKEN_COUNT));
        }
        other => panic!("expected a token count error, got {other:?}"),
    }
    assert_eq!(parse_troops_eng(b"no marker here"), Err(SeedError::NoMarker));
}

#[test]
fn text_before_the_first_marker_is_ignored_even_when_it_contains_numbers() {
    // The real header contains "Lords2" and "zero to ten"; a parser that
    // started at byte 0 would be off by however many digits it found.
    let with_header = synthetic_troops_eng();
    let stripped: Vec<u8> = {
        let at = with_header.iter().position(|&b| b == b'*').unwrap();
        with_header[at..].to_vec()
    };
    let a = parse_troops_eng(&with_header).unwrap();
    let b = parse_troops_eng(&stripped).unwrap();
    assert_eq!(a.counts, b.counts);
    assert_eq!(a.advantage, b.advantage);
}

#[test]
fn battle_names_come_out_in_triples() {
    let names = battle_names(&synthetic_battles_eng());
    assert_eq!(names.len(), ROWS);
    assert_eq!(names[0], "Battle 1");
    assert_eq!(names[34], "Battle 35");
}

#[test]
fn slugs_are_readable_and_stable() {
    assert_eq!(slug("Three Bridges"), "three_bridges");
    assert_eq!(slug("Here Be Dragons!"), "here_be_dragons");
    assert_eq!(slug("  -- "), "unnamed");
    assert_eq!(slug("Siege14"), "siege14");
}

#[test]
fn the_generated_rules_reparse_and_mean_what_the_file_meant() {
    let table = parse_troops_eng(&synthetic_troops_eng()).unwrap();
    let names = battle_names(&synthetic_battles_eng());
    let text = to_rules_toml(&table, &names, "TROOPS2.ENG");

    let mut rs = Ruleset::new();
    rs.apply_str(&text, "base:rules/troops.toml").expect("generated rules parse");
    // Generating and reading back must not contest anything with itself.
    assert!(rs.log.overrides.is_empty());

    let rules = TroopRules::from_ruleset(&rs).expect("typed view");
    assert_eq!(rules.troops.len(), COLUMNS);
    assert_eq!(rules.battles.len(), ROWS);
    assert_eq!(rules.difficulties.len(), GROUPS);

    let b = rules.battle("battle_1").expect("first battle");
    assert_eq!(b.name, "Battle 1");
    // Row 0, Normal group, attacker, column 1 (crossbows).
    assert_eq!(b.attacker[1], table.counts[0][NORMAL_GROUP][0][1]);
    assert_eq!(b.defender[1], table.counts[0][NORMAL_GROUP][1][1]);
}

/// **The curve the binary hard-codes, as the ruleset carries it.**
///
/// `docs/formats/eng.md` §3.2: the engine derives every difficulty group from
/// Normal as `x * p / 100` with p = 116, 108, 100, 92, 84, for troop columns
/// 0–6 only. All five percentages are asserted here, over every column, from
/// the ruleset the seeder actually emits.
///
/// **Corrected.** This test used to inject `scale_percent = 50` through a mod
/// and then assert that the engine had applied 50 %, over a synthetic file:
///
/// ```text
/// rs.apply_str("[difficulty.very_hard]\nscale_percent = 50\n", ...);
/// assert_eq!(brutal[0], normal[0] * 50 / 100);
/// ```
///
/// which is a tautology — the test supplied the number it then checked for.
/// Under a name saying "the binary hard-coded", nothing about the binary was
/// touched, and a typo in `seed::DIFFICULTIES` would not have moved it. One
/// line at the end did check 116; the other four percentages were unguarded.
///
/// The override half is still worth having and is kept below, as a separate
/// assertion that says what it is.
#[test]
fn the_difficulty_curve_the_binary_hard_coded_is_now_a_rule() {
    let table = parse_troops_eng(&synthetic_troops_eng()).unwrap();
    let text = to_rules_toml(&table, &[], "TROOPS2.ENG");
    let mut rs = Ruleset::new();
    rs.apply_str(&text, "base:rules/troops.toml").unwrap();
    let rules = TroopRules::from_ruleset(&rs).unwrap();

    let mut checked = 0;
    for &(id, _, percent) in &DIFFICULTIES {
        assert_eq!(
            rules.difficulty(id).expect(id).scale_percent,
            percent,
            "{id} lost its transcribed percentage"
        );
        for b in &rules.battles {
            for side in [Side::Attacker, Side::Defender] {
                let normal = rules.army(b, "normal", side);
                let scaled = rules.army(b, id, side);
                for c in 0..COLUMNS {
                    let want =
                        if c < 7 { normal[c] * percent / 100 } else { normal[c] };
                    assert_eq!(scaled[c], want, "{} {id} side {side:?} column {c}", b.id);
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, DIFFICULTIES.len() * ROWS * 2 * COLUMNS);

    // The five percentages themselves, spelled out once so that a change to
    // seed::DIFFICULTIES has to be made twice on purpose.
    assert_eq!(
        DIFFICULTIES.map(|(_, _, p)| p),
        [116, 108, 100, 92, 84],
        "the curve docs/formats/eng.md §3.2 reads out of the engine"
    );
}

/// A mod may replace the curve, and only the columns the curve touches move.
#[test]
fn a_mod_can_replace_the_difficulty_curve_and_siege_columns_still_do_not_scale() {
    let table = parse_troops_eng(&synthetic_troops_eng()).unwrap();
    let text = to_rules_toml(&table, &[], "TROOPS2.ENG");
    let mut rs = Ruleset::new();
    rs.apply_str(&text, "base:rules/troops.toml").unwrap();
    rs.apply_str("[difficulty.very_hard]\nscale_percent = 50\n", "brutal:rules/curve.toml")
        .unwrap();

    let rules = TroopRules::from_ruleset(&rs).unwrap();
    let b = rules.battle("battle_5").unwrap();
    let normal = rules.army(b, "normal", Side::Attacker);
    let brutal = rules.army(b, "very_hard", Side::Attacker);

    // Non-siege columns halve, with the engine's truncating division.
    assert_eq!(brutal[0], normal[0] * 50 / 100);
    assert_eq!(brutal[6], normal[6] * 50 / 100);
    // Siege columns do not scale at any difficulty. That is the original's
    // behaviour, preserved.
    for c in 7..COLUMNS {
        assert_eq!(brutal[c], normal[c], "siege column {c} was scaled");
    }
    // The groups the mod did not name keep the transcribed curve.
    let easy = rules.army(b, "very_easy", Side::Attacker);
    assert_eq!(easy[0], normal[0] * 116 / 100);
}

#[test]
fn quoting_survives_a_battle_name_with_a_quote_in_it() {
    let table = parse_troops_eng(&synthetic_troops_eng()).unwrap();
    let names = vec![r#"The "Bridge""#.to_string()];
    let text = to_rules_toml(&table, &names, "TROOPS.ENG");
    let mut rs = Ruleset::new();
    rs.apply_str(&text, "base:rules/troops.toml").expect("still parses");
    assert_eq!(rs.string("battle.the_bridge.name").unwrap(), r#"The "Bridge""#);
}
