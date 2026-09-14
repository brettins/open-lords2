//! **The England turn-one fixture**, read against itself.
//!
//! ```text
//! LORDS2_FIXTURES="E:\dev\lords2-fixtures" cargo test -p l2-formats --test save_england_turn1
//! ```
//!
//! # What this file is for
//!
//! `tests/save.rs` asserts what is true of *any* save. This file asserts one
//! particular saved game: the England map at the start of turn one, Winter
//! 1268. Every test here is gated on `l2_testkit::england_turn1`, which finds
//! `%LORDS2_FIXTURES%\england-turn1.sav` and **checks it is that game** before
//! handing it over. Absent, the tests skip and say so; present and wrong, they
//! fail with a message naming the fixture.
//!
//! # Why the file is called a fixture and not "the shipped save"
//!
//! A clean GOG install ships no saves at all — verified by diffing a pristine
//! copy of the install against a played one: six files differ and five of them
//! are saves. This save was produced by someone in an earlier session of this
//! project starting a campaign. The old name said "shipped", the old code read
//! it out of the game directory, and the game rewrites that file every turn
//! somebody plays. Ten minutes of play turned nine tests red, and the fix that
//! matters is not the path — it is that a fixture is now a name with a
//! fingerprint behind it.
//!
//! # Three assertions that were accidents
//!
//! Comparing two independently created England turn-one saves settled what is
//! scenario and what is per-game noise. Three of the nine tests below were
//! asserting noise, and each one is called out where it now stands:
//!
//! * the **realm→county assignment** is rolled per game; only the set of five
//!   counties is fixed;
//! * `g_weatherCounty` is a per-game roll;
//! * "county 1 is the odd one out on food" is really **"realm 5's starting
//!   county begins on Half rations"** — a genuine mechanic that had been pinned
//!   to a county index by coincidence.
//!
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

/// The fixture is what it says it is. Runs the fingerprint explicitly so a
/// green run has said, out loud, which game it asserted against.
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

