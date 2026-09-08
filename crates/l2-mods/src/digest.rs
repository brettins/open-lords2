//! The ruleset digest: one number two machines must agree on.
//!
//! # Why a ruleset needs a checksum at all
//!
//! `docs/netcode.md` makes deterministic lockstep the whole architecture: two
//! peers execute the same commands and must reach bit-identical state. That
//! guarantee assumes they are running the same *rules*, and once rules come
//! out of mod directories that assumption stops being free. Two players with
//! the same mods in different order, or one with a mod the other has not
//! updated, will desync on the first tick where a changed number matters —
//! which may be minutes in, and which will look like a simulation bug.
//!
//! A digest exchanged at session setup turns that into a refusal at the
//! lobby, naming the problem. That is the same reasoning as
//! `l2_net::lockstep`'s per-tick checksum, one level earlier.
//!
//! # Why it is `l2-net`'s encoder and not a new one
//!
//! `l2_net::Canonical` exists precisely so that everything which turns state
//! into bytes produces the *same* bytes (§6). Writing a second encoder here
//! would mean two byte streams that could disagree with each other, and the
//! first hour of any investigation would go on establishing which one was
//! right. So this is `Canonical`, seeded and sectioned the same way.
//!
//! # What is deliberately not in the stream
//!
//! **Origins.** Every value in the tree remembers the file and line that set
//! it, and none of that is hashed. A mod installed at a different path, or a
//! base ruleset generated on a machine whose install lives on another drive,
//! is the same *rules* — and a digest that said otherwise would refuse
//! sessions that would have run perfectly. The digest answers "are we playing
//! the same game", not "do we have the same files".
//!
//! **Load order, and which mod set what.** Two different load orders that
//! merge to the same numbers are the same rules, and the simulation cannot
//! tell them apart, so neither does this. [`crate::Report`] is where the
//! difference between them is visible.
//!
//! # Floats
//!
//! Floats are hashed by their bit pattern, which is deterministic. That is not
//! an endorsement: `docs/netcode.md` forbids floats in anything the simulation
//! evaluates, and [`crate::Ruleset::float_rules`] lists any that appear so the
//! question gets asked. Hashing them by bits at least means a float that does
//! slip in cannot silently differ between peers.

use crate::value::{Table, Value};
use crate::Ruleset;
use l2_net::Canonical;

/// Type tags, so that a string `"1"` and an integer `1` cannot produce the
/// same byte stream. Frozen: changing one changes every digest.
const TAG_STRING: u8 = 1;
const TAG_INTEGER: u8 = 2;
const TAG_FLOAT: u8 = 3;
const TAG_BOOLEAN: u8 = 4;
const TAG_ARRAY: u8 = 5;
const TAG_TABLE: u8 = 6;

/// The canonical byte form of a merged ruleset.
///
/// Keys are walked in `BTreeMap` order, which is byte order on the key, which
/// is the same on every machine. Nothing here iterates a hash map, reads a
/// directory, or depends on the order documents happened to be applied in —
/// only on the values that survived.
pub fn encode(rules: &Ruleset) -> Vec<u8> {
    let mut c = Canonical::recording();
    encode_table(&mut c, rules.root());
    c.finish().bytes.expect("recording encoder keeps its bytes")
}

/// The 64-bit checksum of a merged ruleset.
pub fn digest(rules: &Ruleset) -> u64 {
    let mut c = Canonical::hashing();
    encode_table(&mut c, rules.root());
    c.finish().hash
}

/// The digest as the sixteen lowercase hex digits a user can read out over
/// voice chat and compare.
pub fn digest_hex(rules: &Ruleset) -> String {
    format!("{:016x}", digest(rules))
}

/// The digest that goes in `l2_net::Hello::ruleset_hash`: the merged rules
/// **plus the mod ids in load order**.
///
/// # Why this is stricter than [`digest`], and why both exist
///
/// [`digest`] answers "do our rules agree", which is the right question for a
/// diagnostic and the wrong one for a handshake. `docs/netcode.md` D-12 says
/// two peers must agree on *the mod set and load order*, and `l2_net::Hello`
/// documents `ruleset_hash` as covering the mod that set each value. The
/// difference matters because the merged rule tree is not everything a mod can
/// change:
///
/// * **A mod can replace an asset that is simulation input.** A `.skr`
///   battlefield is terrain, and terrain decides pathfinding. Nothing in the
///   rule tree would move, and the two peers would desync on the first unit to
///   walk. Hashing 1,196 files at load would cover it properly and is not done
///   here; hashing the mod list is the cheap proxy that at least catches "you
///   have a mod I do not". **This is a real gap and it is not closed** —
///   `docs/modding.md` §12 says so in the same words.
/// * **A mod can set rules a future build will read.** Values the current
///   engine ignores are still a difference between the two installs, and one
///   of them may be running the build that reads them.
///
/// So the handshake takes the strict answer and a false refusal — two peers
/// with harmlessly different mod lists being told to fix it — is the cheap
/// failure. The expensive one is letting them in and desyncing an hour later
/// with a symptom that points nowhere.
///
/// Ids and order only. Versions are deliberately out: a mod that changed its
/// version without changing a rule is the same simulation, and the rules
/// digest already catches one that changed a rule without changing its
/// version, which is the dangerous direction.
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
            // By bit pattern, not by decimal rendering: two peers must agree
            // on the bytes, and a formatter is a place they could differ.
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
