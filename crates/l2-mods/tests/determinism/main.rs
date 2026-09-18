
mod order_determinism;
pub use order_determinism::*;
mod hashing_and_types;
pub use hashing_and_types::*;

#[path = "../common/mod.rs"]
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

