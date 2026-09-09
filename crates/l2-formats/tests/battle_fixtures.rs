//! **The battle fixtures**: one battle caught at three moments, plus two
//! earlier turns of the same game.
//!
//! ```text
//! LORDS2_FIXTURES="E:\dev\lords2-fixtures" cargo test -p l2-formats --test battle_fixtures
//! ```
//!
//! # What these are for
//!
//! `crates/l2-formats/tests/save.rs` runs invariants over whatever saves exist,
//! and `save_england_turn1.rs` asserts one named position. These five sit
//! between: they are a *different* scenario — a small map with two owned
//! counties — preserved so that the campaign-army work has a real before and
//! after to read rather than a synthetic one.
//!
//! | fixture | what happened |
//! |---|---|
//! | `battle-before.sav` | the player's army of 178 stands at (33, 17); county 3 is neutral |
//! | `battle-during.sav` | mid-battle: a defender has appeared, ownerless, and county 3 has lost people to it |
//! | `battle-after.sav` | the player lost; both armies are gone and county 3 is still neutral |
//! | `old_turn.sav`, `safeturn.sav` | earlier turns of the same game — a multi-turn economy |
//!
//! # Why this file exists rather than the army tests alone
//!
//! **A fixture nobody checks is a fixture that quietly becomes something else.**
//! That is the whole of `docs/decisions.md` C23: five tests asserted a saved
//! game's numbers for months and the file underneath them was a rolling
//! autosave. So the *county-side* facts of these five are asserted here, in the
//! crate that can read them today, before anything is built on top. The unit
//! array is not read by `l2-formats` yet; when it is, the army assertions belong
//! next to it and this file's job is done.
//!
//! Nothing here asserts a value that could not be re-derived by opening the
//! file, and every number is labelled with which fixture it came from — so a
//! replaced fixture fails loudly and names itself instead of failing eleven
//! tests in another crate.

use l2_formats::save::Save;

/// Population of county 3 across the three moments of the battle.
///
/// The drop is the interesting part and it is not ours to explain yet: county 3
/// holds 728 before, 546 during and 582 after, so the defending force is drawn
/// **out of the county's people** and some of them come back. That is a claim
/// about the original's siege bookkeeping and it is recorded here as an
/// observation with a file behind it, not as a rule.
const COUNTY_THREE: [(&str, i32); 3] = [
    ("battle-before.sav", 728),
    ("battle-during.sav", 546),
    ("battle-after.sav", 582),
];

/// A macro rather than a function because the gate skips by returning from the
/// **test**, and a helper returning `Save` has nothing to return.
macro_rules! open {
    ($name:expr) => {
        l2_testkit::fixture!($name)
    };
}

/// The three moments are the same game on the same map, one battle apart.
#[test]
fn the_battle_triple_is_one_game_on_one_map() {
    let before = open!("battle-before.sav");
    let during = open!("battle-during.sav");
    let after = open!("battle-after.sav");

    for (name, save) in
        [("before", &before), ("during", &during), ("after", &after)]
    {
        let g = save.globals().expect("globals");
        assert_eq!(g.county_count, 4, "{name}: a four-county map");
        assert_eq!(g.local_player, 1, "{name}");
        assert_ne!(g.scenario_index, 0, "{name}: not the England map");
    }

    // Same map, so the same adjacency, and the same two owners.
    let owners = |s: &Save| -> Vec<(usize, u8)> {
        s.counties().unwrap().iter().filter(|c| c.is_owned()).map(|c| (c.index, c.owner)).collect()
    };
    assert_eq!(owners(&before), owners(&during));
    assert_eq!(owners(&before), owners(&after));
    assert_eq!(owners(&before).len(), 2, "two owned counties, realms 1 and 2");

    let neighbours = |s: &Save| -> Vec<Vec<u8>> {
        s.counties().unwrap().iter().map(|c| c.neighbours().to_vec()).collect()
    };
    assert_eq!(neighbours(&before), neighbours(&during), "the map did not change");
    assert_eq!(neighbours(&before), neighbours(&after));
}

/// **County 3 is neutral throughout, and its population moves with the
/// battle.** This is the fixture contract the army work reads: if the numbers
/// below stop matching, the files have been replaced and the army assertions
/// resting on them are about a different game.
#[test]
fn county_three_stays_neutral_and_its_population_is_the_fixture_fingerprint() {
    for (file, population) in COUNTY_THREE {
        let save = open!(file);
        let c = save.counties().expect("counties")[3];
        assert!(c.is_county(), "{file}: county 3 is a real county");
        assert_eq!(c.owner, 0, "{file}: county 3 is neutral");
        assert_eq!(c.population, population, "{file}: county 3 population");
    }
    // Stated as a relation as well as three literals, because the relation is
    // what the battle means: people are taken and some come back.
    let [(_, b), (_, d), (_, a)] = COUNTY_THREE;
    assert!(d < b, "the defence is drawn out of the county");
    assert!(a > d && a < b, "some of them come back, and not all");
}

/// The two earlier turns are the same game further back, so the clock runs
/// forward and the map does not change.
#[test]
fn the_turn_pair_is_the_same_game_at_earlier_turns() {
    let old = open!("old_turn.sav");
    let safe = open!("safeturn.sav");
    let after = open!("battle-after.sav");

    let turn = |s: &Save| s.globals().unwrap().turn_count;
    assert!(turn(&old) > 0 && turn(&safe) > 0, "both are past the start");
    assert!(
        turn(&old) <= turn(&after) && turn(&safe) <= turn(&after),
        "old_turn {} and safeturn {} are not later than battle-after {}",
        turn(&old),
        turn(&safe),
        turn(&after)
    );

    for (name, save) in [("old_turn", &old), ("safeturn", &safe)] {
        let g = save.globals().unwrap();
        assert_eq!(g.county_count, 4, "{name}: the same four-county map");
        assert_eq!(g.local_player, 1, "{name}");
    }
    eprintln!(
        "turn pair: old_turn {}, safeturn {}, battle-after {}",
        turn(&old),
        turn(&safe),
        turn(&after)
    );
}
