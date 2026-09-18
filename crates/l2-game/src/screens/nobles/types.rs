#![allow(unused_imports)]
use super::*;
use super::view::*;
use l2_view::Canvas;
use l2_kingdom::realm::Realm;
use l2_kingdom::tables::SCORE_INPUT_CASTLES;
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Pen};

/// **`FUN_00415E42`** — one realm's score in one category.
pub fn value(realm: &Realm, category: usize, year: i32) -> i32 {
    if !realm.in_play {
        return 0;
    }
    match category {
        COUNTIES => realm.county_count as i32,
        CASTLES => realm.score_inputs[SCORE_INPUT_CASTLES] as u8 as i32,
        TROOPS => realm.total_men,
        CROWNS => realm.gold,
        HAPPINESS => realm.mean_happiness,
        PEOPLE => realm.population_total,
        _ => {
            if year < GREATEST_NOBLE_YEAR {
                GREATEST_NOBLE_EARLY
            } else {
                6 - realm.rank as i32
            }
        }
    }
}

impl Standings {
    /// The line prints a name only when neither flag is set — the painter's
    /// own `if (DAT_00522C78 == 0 && DAT_00522C94 == 0)`.
    pub fn decided(&self) -> bool {
        !self.all_level && !self.tied_at_top
    }
}

/// **`FUN_00415BDC`** — score every realm in one category, turn the scores
/// into percentages of the leader's, and decide whether anything leads.
pub fn rank(realms: &[Realm], category: usize, year: i32) -> Standings {
    let at = |r: usize| realms.get(r).map_or(0, |realm| value(realm, category, year));
    let in_play = |r: usize| realms.get(r).is_some_and(|realm| realm.in_play);

    let mut leader = 1u8;
    let mut best = 0i32;
    for r in 1..l2_kingdom::MAX_REALMS {
        let v = at(r);
        if best <= v {
            leader = r as u8;
            best = v;
        }
    }

    let mut pct = [0i32; l2_kingdom::MAX_REALMS];
    for r in 1..l2_kingdom::MAX_REALMS {
        pct[r] = l2_kingdom::math::pct_of(at(r), best).clamp(0, 100);
    }

    let mut first: Option<i32> = None;
    let mut all_level = true;
    for r in 1..l2_kingdom::MAX_REALMS {
        if !in_play(r) {
            continue;
        }
        match first {
            None => first = Some(pct[r]),
            Some(f) if pct[r] != f => all_level = false,
            Some(_) => {}
        }
    }

    let mut tied_at_top = false;
    if all_level {
        for r in 1..l2_kingdom::MAX_REALMS {
            if in_play(r) {
                pct[r] = LEVEL_BAR_PCT;
            }
        }
    } else {
        for r in 1..l2_kingdom::MAX_REALMS {
            if in_play(r) && r as u8 != leader && pct[r] == pct[leader as usize] {
                tied_at_top = true;
            }
        }
    }

    Standings { pct, leader, all_level, tied_at_top }
}

