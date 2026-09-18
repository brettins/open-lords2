
use crate::value::{Table, Value};
use crate::Ruleset;
use l2_net::Canonical;

const TAG_STRING: u8 = 1;
const TAG_INTEGER: u8 = 2;
const TAG_FLOAT: u8 = 3;
const TAG_BOOLEAN: u8 = 4;
const TAG_ARRAY: u8 = 5;
const TAG_TABLE: u8 = 6;

pub fn encode(rules: &Ruleset) -> Vec<u8> {
    let mut c = Canonical::recording();
    encode_table(&mut c, rules.root());
    c.finish().bytes.expect("recording encoder keeps its bytes")
}

pub fn digest(rules: &Ruleset) -> u64 {
    let mut c = Canonical::hashing();
    encode_table(&mut c, rules.root());
    c.finish().hash
}

pub fn digest_hex(rules: &Ruleset) -> String {
    format!("{:016x}", digest(rules))
}

pub fn session_digest(rules: &Ruleset, load_order: &[crate::ModMeta]) -> u64 {
    let mut c = Canonical::hashing();
    c.section("rules");
    encode_table(&mut c, rules.root());
    c.section("mods");
    c.len32(load_order.len());
    for m in load_order {
        c.str(&m.id);
    }
    c.finish().hash
}

fn encode_table(c: &mut Canonical, table: &Table) {
    c.u8(TAG_TABLE);
    c.len32(table.len());
    for (key, spanned) in table {
        c.str(key);
        encode_value(c, &spanned.value);
    }
}

fn encode_value(c: &mut Canonical, value: &Value) {
    match value {
        Value::String(s) => {
            c.u8(TAG_STRING);
            c.str(s);
        }
        Value::Integer(i) => {
            c.u8(TAG_INTEGER);
            c.i64(*i);
        }
        Value::Float(f) => {
            c.u8(TAG_FLOAT);
            c.u64(f.to_bits());
        }
        Value::Boolean(b) => {
            c.u8(TAG_BOOLEAN);
            c.bool(*b);
        }
        Value::Array(items) => {
            c.u8(TAG_ARRAY);
            c.len32(items.len());
            for item in items {
                encode_value(c, &item.value);
            }
        }
        Value::Table(t) => encode_table(c, t),
    }
}
