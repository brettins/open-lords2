#![allow(unused_imports)]
use super::*;
use super::harvest::*;
use super::rules::*;
use super::ai::*;
use super::*;
use super::combat::*;
use common::TempDir;
use l2_mods::Platform;
use l2_sim::{Battle, Troop, TroopTable, SIDE_A, SIDE_B};
use l2_kingdom::tables::{health_band, Tables};
use l2_kingdom::{Kingdom, Options};
use l2_kingdom::tables::{Commodity, JOB_COUNT};

#[test]
fn an_impossible_new_kingdom_rule_is_refused_with_the_line_that_wrote_it() {
    let refusal = |name: &'static str, rules: &str| -> String {
        let base = TempDir::new(&format!("{name}-base"));
        let mods = TempDir::new(&format!("{name}-mods"));
        empty_base(&base);
        mods.write(&format!("{name}/mod.toml"), &format!("[mod]\nid = \"{name}\"\n"));
        mods.write(&format!("{name}/rules/{name}.toml"), rules);
        Platform::builder()
            .base(base.path())
            .mods_dir(mods.path())
            .enable([name])
            .build()
            .expect("the document is well formed, so the load itself succeeds")
            .kingdom_tables()
            .expect_err("but the rule cannot be run")
            .to_string()
    };

    let err = refusal("free-ale", "[kingdom.happiness]\nale_step_pct = 0\n");
    assert!(err.contains("free-ale:rules/free-ale.toml:2:16"), "{err}");
    assert!(err.contains("kingdom.happiness.ale_step_pct"), "{err}");
    assert!(err.contains("0 is outside 1..=10000"), "{err}");

    let mut ninth = String::new();
    for rung in 0..9 {
        ninth.push_str(&format!("[[kingdom.ai.tax_ladder.0]]\nbelow = {}\nrate = 1\n\n", rung * 10));
    }
    let err = refusal("long-ladder", &ninth);
    assert!(err.contains("kingdom.ai.tax_ladder.0"), "{err}");
    assert!(err.contains("1 to 8 rungs, found 9"), "{err}");
}


