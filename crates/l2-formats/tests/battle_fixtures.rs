//! That is the whole of `docs/decisions.md` C23: five tests asserted a saved
//! game's numbers for months and the file underneath them was a rolling
//! autosave. So the *county-side* facts of these five are asserted here, in the
//! crate that can read them today, before anything is built on top. The unit
//! array is not read by `l2-formats` yet; when it is, the army assertions belong
//! next to it and this file's job is done.

use l2_formats::save::Save;

const COUNTY_THREE: [(&str, i32); 3] = [
    ("battle-before.sav", 728),
    ("battle-during.sav", 546),
    ("battle-after.sav", 582),
];

macro_rules! open {
    ($name:expr) => {
        l2_testkit::fixture!($name)
    };
}

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

#[test]
fn county_three_stays_neutral_and_its_population_is_the_fixture_fingerprint() {
    for (file, population) in COUNTY_THREE {
        let save = open!(file);
        let c = save.counties().expect("counties")[3];
        assert!(c.is_county(), "{file}: county 3 is a real county");
        assert_eq!(c.owner, 0, "{file}: county 3 is neutral");
        assert_eq!(c.population, population, "{file}: county 3 population");
    }
    let [(_, b), (_, d), (_, a)] = COUNTY_THREE;
    assert!(d < b, "the defence is drawn out of the county");
    assert!(a > d && a < b, "some of them come back, and not all");
}

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
