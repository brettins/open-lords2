//! Merge semantics — the part a mod author has to be able to predict.

use l2_mods::Ruleset;

fn rules(docs: &[(&str, &str)]) -> Ruleset {
    let mut rs = Ruleset::new();
    for (source, text) in docs {
        rs.apply_str(text, source).unwrap_or_else(|e| panic!("{source}: {e}"));
    }
    rs
}

#[test]
fn a_mod_changes_one_number_and_keeps_the_rest() {
    let rs = rules(&[
        (
            "base:rules/troops.toml",
            "[battle.three_bridges.attacker]\ncrossbows = 20\nknights = 4\npeasants = 100\n",
        ),
        ("longbows:rules/tweak.toml", "[battle.three_bridges.attacker]\ncrossbows = 40\n"),
    ]);
    assert_eq!(rs.integer("battle.three_bridges.attacker.crossbows").unwrap(), 40);
    assert_eq!(rs.integer("battle.three_bridges.attacker.knights").unwrap(), 4);
    assert_eq!(rs.integer("battle.three_bridges.attacker.peasants").unwrap(), 100);
}

#[test]
fn an_override_names_both_documents_and_both_lines() {
    let rs = rules(&[
        ("base:rules/troops.toml", "[t]\nx = 1\n"),
        ("modA:rules/a.toml", "[t]\nx = 2\n"),
    ]);
    assert_eq!(rs.log.overrides.len(), 1);
    let o = &rs.log.overrides[0];
    assert_eq!(o.path, "t.x");
    assert_eq!(&*o.previous.source, "base:rules/troops.toml");
    assert_eq!(o.previous.line, 2);
    assert_eq!(&*o.current.source, "modA:rules/a.toml");
    assert_eq!(o.current.line, 2);
    assert!(!o.type_changed);
    // And the ruleset can be asked directly.
    assert_eq!(&*rs.origin("t.x").unwrap().source, "modA:rules/a.toml");
}

#[test]
fn adding_a_new_key_is_not_an_override() {
    let rs = rules(&[("base:a.toml", "[t]\nx = 1\n"), ("m:b.toml", "[t]\ny = 2\n")]);
    assert!(rs.log.overrides.is_empty());
    assert_eq!(rs.integer("t.x").unwrap(), 1);
    assert_eq!(rs.integer("t.y").unwrap(), 2);
}

#[test]
fn a_type_change_is_flagged_separately() {
    let rs = rules(&[("base:a.toml", "x = 1\n"), ("m:b.toml", "x = \"one\"\n")]);
    assert_eq!(rs.log.overrides.len(), 1);
    assert!(rs.log.overrides[0].type_changed);
}

#[test]
fn arrays_replace_whole_rather_than_merging_element_wise() {
    let rs = rules(&[("base:a.toml", "seasons = [1, 2, 3, 4]\n"), ("m:b.toml", "seasons = [9]\n")]);
    let v = rs.get("seasons").unwrap().value.as_array().unwrap();
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].value.as_integer(), Some(9));
}

#[test]
fn delete_removes_a_key_and_records_where_it_went() {
    let rs = rules(&[
        ("base:a.toml", "[troop.knight]\nattack = 40\n\n[troop.peasant]\nattack = 5\n"),
        ("m:b.toml", "[troop]\n\"$delete\" = [\"knight\"]\n"),
    ]);
    assert!(rs.get("troop.knight").is_none());
    assert_eq!(rs.integer("troop.peasant.attack").unwrap(), 5);
    assert_eq!(rs.log.deletions.len(), 1);
    assert_eq!(rs.log.deletions[0].path, "troop.knight");
    assert_eq!(&*rs.log.deletions[0].by.source, "m:b.toml");
}

#[test]
fn delete_then_redefine_in_the_same_document_works() {
    // The directive is applied before the rest of the table merges, so a mod
    // can replace a table wholesale instead of merging into it.
    let rs = rules(&[
        ("base:a.toml", "[troop.knight]\nattack = 40\ndefence = 30\n"),
        ("m:b.toml", "[troop]\n\"$delete\" = [\"knight\"]\n\n[troop.knight]\nattack = 1\n"),
    ]);
    assert_eq!(rs.integer("troop.knight.attack").unwrap(), 1);
    assert!(rs.get("troop.knight.defence").is_none());
}

#[test]
fn deleting_something_that_is_not_there_is_reported_not_ignored() {
    let rs = rules(&[("base:a.toml", "[troop.knight]\nattack = 1\n"), ("m:b.toml", "[troop]\n\"$delete\" = [\"knght\"]\n")]);
    assert_eq!(rs.log.dangling_deletes.len(), 1);
    assert_eq!(rs.log.dangling_deletes[0].path, "troop.knght");
}

#[test]
fn three_documents_fighting_over_one_path_is_surfaced() {
    let rs = rules(&[
        ("base:a.toml", "x = 1\n"),
        ("m1:b.toml", "x = 2\n"),
        ("m2:c.toml", "x = 3\n"),
    ]);
    assert_eq!(rs.integer("x").unwrap(), 3);
    let contested = rs.log.contested_paths();
    assert_eq!(contested, vec![("x", 3)]);
}

#[test]
fn merging_is_order_dependent_and_the_last_document_wins() {
    let a = ("base:a.toml", "x = 1\n");
    let b = ("m:b.toml", "x = 2\n");
    assert_eq!(rules(&[a, b]).integer("x").unwrap(), 2);
    assert_eq!(rules(&[b, a]).integer("x").unwrap(), 1);
}

#[test]
fn typed_accessors_report_the_offending_line() {
    let rs = rules(&[("m:b.toml", "[t]\n\nx = \"not a number\"\n")]);
    let e = rs.integer("t.x").unwrap_err().to_string();
    assert!(e.contains("m:b.toml:3"), "{e}");
    assert!(e.contains("should be a integer") || e.contains("integer"), "{e}");

    let missing = rs.integer("t.nope").unwrap_err().to_string();
    assert!(missing.contains("t.nope"), "{missing}");

    let range = rules(&[("m:b.toml", "x = 99\n")]).integer_in("x", 0, 10).unwrap_err().to_string();
    assert!(range.contains("outside 0..=10"), "{range}");
}
