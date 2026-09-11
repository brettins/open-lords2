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

/// **The four siege saves are one catapult being built, and the arithmetic
/// closes.** This is the evidence for [`l2_formats::save::SiegeEngine`]'s
/// offsets and for `l2_kingdom::siege::build_tick`'s port at the same time,
/// because neither was fitted to it.
///
/// Realm 1's 43-man army stands outside county 4's castle in all four, with one
/// catapult ordered. `g_siegeEngineWork[catapult]` is **200** man-seasons, read
/// out of `Lords2.exe` and written down long before anybody opened these files.
/// A besieging army puts `men` man-seasons a season into its unfinished
/// records — `Siege_BuildTick` (`0x004A8507`) — and `Siege_RecomputeBuildTime`
/// (`0x004A80DB`) writes `percent = work * 100 / total` and
/// `seasonsLeft = ceil((total − work) / men)`.
///
/// | save | work | step | `work * 100 / 200` | `ceil((200−work)/43)` | stored % | stored `+0x19C` |
/// |---|---:|---|---:|---:|---:|---:|
/// | `siege-safeturn` | 86 | — | 43 | 3 | 43 | 3 |
/// | `siege-old_turn` | 129 | +43 | 64 | 2 | 64 | 2 |
/// | `siege-lastturn` | 172 | +43 | 86 | 1 | 86 | 1 |
/// | `siege-sieging` | 200 | capped | 100 | 0 | 100 | 0 |
///
/// **Four independent quantities agree at once** — the +43 step, the
/// percentage, the countdown, and the 200 that came from the binary — so the
/// three `i16` at `+0x182` are what they are claimed to be. `[V]`
///
/// And the consequence, which is why this test exists rather than the offsets
/// being taken on trust: the importer used to default all of this to zero, so
/// our End Turn read *no work remaining*, wrote `seasonsLeft = 0` and launched
/// the assault **three seasons early** on each of the three turn openings.
/// `docs/decisions.md` CNEW-siege-engines-import.
#[test]
fn the_siege_ladder_of_the_three_siege_saves_is_one_catapult_being_built() {
    /// `g_siegeEngineWork[0]` (`0x004DE440`), from `Lords2.exe`, not from these
    /// files.
    const CATAPULT_WORK: i32 = 200;
    /// The besieger's headcount, which is also its man-seasons a season.
    const MEN: i32 = 43;
    /// `(file, work done, seasons left)`.
    const LADDER: [(&str, i16, u8); 4] = [
        ("siege-safeturn.sav", 86, 3),
        ("siege-old_turn.sav", 129, 2),
        ("siege-lastturn.sav", 172, 1),
        ("siege-sieging.sav", 200, 0),
    ];

    let mut previous: Option<i16> = None;
    for (file, work, seasons) in LADDER {
        let save = open!(file);
        let besieger = save
            .units()
            .expect("units")
            .into_iter()
            .find(|u| u.is_live() && u.kind == 1 && u.besieging_county != 0)
            .unwrap_or_else(|| panic!("{file}: no besieging army"));

        assert_eq!(i32::from(besieger.men as i16), MEN, "{file}: the besieger's headcount");
        assert_eq!(besieger.besieging_county, 4, "{file}: besieging county 4");
        assert_eq!(besieger.engines[0].ordered, 1, "{file}: one catapult ordered");
        assert_eq!(besieger.engines[1].ordered, 0, "{file}: no towers");
        assert_eq!(besieger.engines[2].ordered, 0, "{file}: no rams");
        assert_eq!(besieger.engines[0].work_done, work, "{file}: man-seasons done");

        // The percentage and the countdown are *derived*, so they are checked
        // against the formula rather than against a second literal — two
        // literals maintained together would agree because they were written to
        // (`docs/agents.md`, *two artefacts that must agree*).
        let done = i32::from(work);
        assert_eq!(
            i32::from(besieger.engines[0].percent),
            done * 100 / CATAPULT_WORK,
            "{file}: Siege_RecomputeBuildTime's percentage"
        );
        let remaining = CATAPULT_WORK - done;
        assert_eq!(
            i32::from(seasons),
            (remaining + MEN - 1) / MEN,
            "{file}: ceil({remaining} / {MEN}) seasons"
        );
        assert_eq!(besieger.siege_seasons_left, seasons, "{file}: +0x19C");

        // And the step between consecutive saves is the headcount, capped.
        if let Some(before) = previous {
            assert_eq!(
                done,
                (i32::from(before) + MEN).min(CATAPULT_WORK),
                "{file}: one season of {MEN} men, capped at the catapult's {CATAPULT_WORK}"
            );
        }
        previous = Some(work);
    }

    // The assault does happen — three turns later. `siege-aftersie.sav` has no
    // besieging army left and the garrison is down from 149 to 133, so the
    // question our End Turn got wrong was never *whether*, only *when*.
    let after = open!("siege-aftersie.sav");
    let armies: Vec<_> =
        after.units().unwrap().into_iter().filter(|u| u.is_live() && u.kind == 1).collect();
    assert_eq!(armies.len(), 1, "only the garrison is left after the assault");
    assert_eq!(armies[0].men, 133, "the garrison of 149 lost sixteen men repelling it");
    assert_eq!(armies[0].besieged_by, 0, "and the siege link is gone");
}
