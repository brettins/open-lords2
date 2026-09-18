//! `FUN_004ABD0F`'s three arms and every write each one makes.

use super::*;
use crate::county::County;

fn county(owner: u8, happiness: i32) -> County {
    let mut c = County::new();
    c.owner = owner;
    c.happiness = happiness;
    c
}

#[test]
fn a_mob_says_one_of_three_things_by_the_moods_it_walked_into() {
    let cases: [(i32, u16, bool); 6] = [
        (0, GROUP_REVOLUTION_SPREAD, true),
        (9, GROUP_REVOLUTION_SPREAD, true),
        (10, GROUP_TROUBLE_SPREADING, false),
        (29, GROUP_TROUBLE_SPREADING, false),
        (30, GROUP_PEOPLE_TROUBLED, false),
        (100, GROUP_PEOPLE_TROUBLED, false),
    ];
    for (happiness, group, revolt) in cases {
        let x = crossing(&county(2, happiness), 5).expect("an owned county answers");
        assert_eq!(
            (x.letter.group, x.raise_revolt),
            (group, revolt),
            "happiness {happiness}"
        );
        assert_eq!(
            (x.letter.from, x.letter.to, x.letter.category, x.letter.county, x.letter.variant),
            (0, 2, CATEGORY_COUNTY_NOTICE, 5, 0),
            "Msg_Enqueue(0, county.owner, group, 0, 3, county, 0, 0)"
        );
    }
}

/// Ablation: drop the `c.owner == 0` guard and this goes red.
#[test]
fn a_mob_crossing_into_neutral_land_says_nothing() {
    for happiness in [0, 9, 10, 50] {
        assert_eq!(crossing(&county(0, happiness), 5), None, "happiness {happiness}");
    }
}

/// Ablation: drop the `shown_events -= HAPPINESS_TOLL` and the second column
/// goes red; drop the toll and the first does.
#[test]
fn the_calm_county_pays_ten_and_nothing_else() {
    let mut c = county(2, 50);
    c.shown_events = 7;
    c.unrest = 0;
    let x = crossing(&c, 5).expect("owned");
    settle(&mut c, &x, false);
    assert_eq!((c.happiness, c.shown_events, c.unrest), (40, -3, 0));
}

#[test]
fn the_troubled_county_is_put_on_unrest_one_but_never_pushed_further() {
    for (before, after) in [(0u8, 1u8), (1, 1), (3, 3)] {
        let mut c = county(2, 20);
        c.unrest = before;
        let x = crossing(&c, 5).expect("owned");
        settle(&mut c, &x, false);
        assert_eq!((c.unrest, c.happiness), (after, 10), "unrest {before}");
    }
}

#[test]
fn a_wretched_county_with_nowhere_to_revolt_is_floored_not_debited() {
    let mut c = county(2, 4);
    c.shown_events = 4;
    c.unrest = 2;
    let x = crossing(&c, 5).expect("owned");
    settle(&mut c, &x, false);
    assert_eq!((c.happiness, c.shown_events, c.unrest), (0, 0, 2));
}

#[test]
fn a_revolt_puts_thirty_back_and_the_toll_then_cannot_floor_it() {
    let mut c = county(2, 4);
    c.shown_events = -6;
    c.unrest = 4;
    let x = crossing(&c, 5).expect("owned");
    assert!(x.raise_revolt, "the wretched arm is the only one that raises");
    settle(&mut c, &x, true);
    assert_eq!((c.happiness, c.shown_events, c.unrest), (24, 14, 0));
}

/// Ablation: drop the floor arm's `shown_events -= happiness` and the second
/// assert goes red; the clamp itself has no probe because nothing can reach it.
#[test]
fn the_last_clamp_is_unreachable_because_the_floor_arm_gets_there_first() {
    let mut c = county(2, -1);
    let x = crossing(&c, 5).expect("owned");
    settle(&mut c, &x, false);
    assert_eq!(c.happiness, 0);
    assert_eq!(c.shown_events, 1, "shownEvents -= happiness, with happiness negative");
}
