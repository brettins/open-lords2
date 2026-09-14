//! Order-determinism
//! preference.
//!
//! `docs/netcode.md` makes deterministic lockstep the architecture: two peers
//! execute the same commands and must reach bit-identical state. That
//! guarantee assumes they are running the same *rules*, and once rules arrive
//! by merging mod directories that assumption stops being free. `l2-net`'s
//! lobby now hashes the resolved ruleset and refuses a peer whose hash differs,
//! so a merge that is even slightly order-dependent turns into a refused
//! session — or worse, a session that starts and desyncs later.
//!
//! Three things could break it, and the two
//!
//! * **Iteration in hash order.** The value tree is `BTreeMap` throughout, and
//!   a source-level test makes adding a `HashMap` a visible decision rather
//!   than an accident.
//! * **Filesystem enumeration order.** `read_dir` gives no ordering guarantee
//!   and differs between filesystems. Every layer's documents are sorted by
//!   name before they apply.
//! * **Floats.** Not forbidden by the reader — a mod may carry one for
//! something the simulation never reads — but they are reported, and the
//!   engine's own rules contain none.
//!
//! And one thing that must *not* affect the answer: where anything is
//! installed. Two players with the same mods at different paths are playing
//! the same game.

mod order_determinism;
pub use order_determinism::*;
mod hashing_and_types;
pub use hashing_and_types::*;

mod common;

use common::TempDir;
use l2_mods::{digest, Platform, Ruleset};

fn install(dir: &TempDir) {
    dir.write("Base1a.pl8", "sprite");
    dir.write(
        "rules/troops.toml",
        r#"
[troop.archers]
column = 5
name = "Archers"
siege_engine = false

[difficulty.normal]
order = 2
scale_percent = 100

[battle.three_bridges]
index = 0
name = "Three Bridges"
defensive_advantage = 5

[battle.three_bridges.attacker]
archers = 200
"#,
    );
}

fn write_mod(dir: &TempDir, id: &str, rules: &str) {
    dir.write(&format!("{id}/mod.toml"), &format!("[mod]\nid = \"{id}\"\n"));
    dir.write(&format!("{id}/rules/{id}.toml"), rules);
}

