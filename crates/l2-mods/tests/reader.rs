//! The rule-document reader.

use l2_mods::value::Value;
use l2_mods::reader::parse;

fn doc(text: &str) -> Value {
    parse(text, "test.toml").expect("parses").value
}

#[test]
fn scalars_of_every_kind() {
    let v = doc(
        r#"
        name = "Longbows"
        count = 40
        ratio = 1.5
        negative = -12
        big = 1_000_000
        hexy = 0xFF
        octal = 0o17
        binary = 0b1010
        on = true
        off = false
        raw = 'C:\games\no escapes'
        "#,
    );
    assert_eq!(v.get("name").unwrap().value, Value::String("Longbows".into()));
    assert_eq!(v.get("count").unwrap().value.as_integer(), Some(40));
    assert_eq!(v.get("ratio").unwrap().value.as_float(), Some(1.5));
    assert_eq!(v.get("negative").unwrap().value.as_integer(), Some(-12));
    assert_eq!(v.get("big").unwrap().value.as_integer(), Some(1_000_000));
    assert_eq!(v.get("hexy").unwrap().value.as_integer(), Some(255));
    assert_eq!(v.get("octal").unwrap().value.as_integer(), Some(15));
    assert_eq!(v.get("binary").unwrap().value.as_integer(), Some(10));
    assert_eq!(v.get("on").unwrap().value.as_bool(), Some(true));
    assert_eq!(v.get("off").unwrap().value.as_bool(), Some(false));
    assert_eq!(v.get("raw").unwrap().value.as_str(), Some(r"C:\games\no escapes"));
}

#[test]
fn tables_dotted_keys_and_comments() {
    let v = doc(
        r#"
        # a comment
        [battle.three_bridges]      # trailing comment
        name = "Three Bridges"
        attacker.crossbows = 20
        attacker.knights = 4

        [battle.three_bridges.defender]
        crossbows = 12
        "#,
    );
    assert_eq!(v.get("battle.three_bridges.name").unwrap().value.as_str(), Some("Three Bridges"));
    assert_eq!(v.get("battle.three_bridges.attacker.crossbows").unwrap().value.as_integer(), Some(20));
    assert_eq!(v.get("battle.three_bridges.attacker.knights").unwrap().value.as_integer(), Some(4));
    assert_eq!(v.get("battle.three_bridges.defender.crossbows").unwrap().value.as_integer(), Some(12));
}

#[test]
fn arrays_inline_tables_and_arrays_of_tables() {
    let v = doc(
        r#"
        requires = ["core >= 1.2", "ui"]
        nested = [[1, 2], [3]]
        point = { x = 1, y = 2 }
        trailing = [
            1,
            2,
        ]

        [[wave]]
        turn = 1
        troops = 10

        [[wave]]
        turn = 5
        troops = 40
        "#,
    );
    assert_eq!(v.get("requires").unwrap().value.as_array().unwrap().len(), 2);
    assert_eq!(v.get("requires.0").unwrap().value.as_str(), Some("core >= 1.2"));
    assert_eq!(v.get("nested.0.1").unwrap().value.as_integer(), Some(2));
    assert_eq!(v.get("point.y").unwrap().value.as_integer(), Some(2));
    assert_eq!(v.get("trailing").unwrap().value.as_array().unwrap().len(), 2);
    assert_eq!(v.get("wave").unwrap().value.as_array().unwrap().len(), 2);
    assert_eq!(v.get("wave.1.troops").unwrap().value.as_integer(), Some(40));
}

#[test]
fn strings_escape_and_span_lines() {
    let v = doc(
        "quoted = \"tab\\there\\nnew\"\n\
         uni = \"\\u00E9\"\n\
         many = \"\"\"\nfirst\nsecond\"\"\"\n\
         joined = \"\"\"a \\\n   b\"\"\"\n\
         literal = '''\nas \\written\n'''\n",
    );
    assert_eq!(v.get("quoted").unwrap().value.as_str(), Some("tab\there\nnew"));
    assert_eq!(v.get("uni").unwrap().value.as_str(), Some("é"));
    assert_eq!(v.get("many").unwrap().value.as_str(), Some("first\nsecond"));
    assert_eq!(v.get("joined").unwrap().value.as_str(), Some("a b"));
    assert_eq!(v.get("literal").unwrap().value.as_str(), Some("as \\written\n"));
}

#[test]
fn quoted_keys_carry_characters_bare_keys_cannot() {
    let v = doc(
        r#"
        "$delete" = ["knight"]
        [troop]
        "long bow" = 3
        "#,
    );
    assert_eq!(v.get("$delete").unwrap().value.as_array().unwrap().len(), 1);
    assert_eq!(v.get("troop").unwrap().value.as_table().unwrap().get("long bow").unwrap().value.as_integer(), Some(3));
}

#[test]
fn every_value_remembers_where_it_came_from() {
    let v = doc("a = 1\n\n[t]\nb = 2\n");
    let a = v.get("a").unwrap();
    assert_eq!((a.origin.line, a.origin.col), (1, 5));
    let b = v.get("t.b").unwrap();
    assert_eq!(b.origin.line, 4);
    assert_eq!(&*b.origin.source, "test.toml");
}

#[test]
fn a_key_set_twice_in_one_document_is_an_error() {
    let e = parse("a = 1\na = 2\n", "dup.toml").unwrap_err();
    assert!(e.message.contains("set twice"), "{}", e.message);
    assert_eq!(e.origin.line, 2);
}

#[test]
fn a_table_defined_twice_is_an_error() {
    let e = parse("[t]\na = 1\n\n[t]\nb = 2\n", "dup.toml").unwrap_err();
    assert!(e.message.contains("defined twice"), "{}", e.message);
}

#[test]
fn errors_point_at_the_line_and_say_what_was_expected() {
    let cases: &[(&str, u32, &str)] = &[
        ("a = 1\nb 2\n", 2, "expected '='"),
        ("a = \"unterminated\n", 1, "unterminated string"),
        ("a = [1, 2\n", 2, "unterminated array"),
        ("a = 1 b = 2\n", 1, "expected end of line"),
        ("[t\n", 1, "expected ']'"),
        ("a = 1979-05-27\n", 1, "dates and times"),
        ("a = 07:32:00\n", 1, "dates and times"),
        ("a = \"x\\q\"\n", 1, "unknown escape"),
    ];
    for (text, line, needle) in cases {
        let e = parse(text, "bad.toml").unwrap_err();
        assert!(
            e.message.contains(needle),
            "for {text:?}: expected {needle:?}, got {:?}",
            e.message
        );
        assert_eq!(e.origin.line, *line, "for {text:?}");
    }
}

#[test]
fn a_scalar_cannot_be_reopened_as_a_table() {
    let e = parse("a = 1\n[a.b]\nc = 2\n", "bad.toml").unwrap_err();
    assert!(e.message.contains("not a table"), "{}", e.message);
}

#[test]
fn empty_and_comment_only_documents_parse_to_an_empty_table() {
    for text in ["", "\n\n", "# nothing here\n", "   \t\n# and here\n"] {
        let v = doc(text);
        assert!(v.as_table().unwrap().is_empty(), "for {text:?}");
    }
}
