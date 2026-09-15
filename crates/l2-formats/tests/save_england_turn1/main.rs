//! All three passed against the original file. They would have passed forever,
//! for the wrong reason, which is `docs/decisions.md` C12's shape.

mod counties;
pub use counties::*;
mod globals;
pub use globals::*;
mod merchants;
pub use merchants::*;

use l2_formats::save::COUNTY_RECORDS;
use l2_testkit::{england, england_county_of_realm, ENGLAND_TURN1_COUNTIES};

#[test]
fn the_fixture_is_the_england_turn_one_position() {
    let save = england!();
    l2_testkit::england_turn1_fingerprint(&save).expect("the gate already checked this");
    let g = save.globals().unwrap();
    eprintln!(
        "england-turn1: scenario {}, {} counties, turn {}, season {}, year {}, realm {} is the person",
        g.scenario_index, g.county_count, g.turn_count, g.season, g.year, g.local_player
    );
    for r in 1..=5u8 {
        eprintln!("  realm {r} holds county {}", england_county_of_realm(&save, r));
    }
}

