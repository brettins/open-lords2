#![allow(unused_imports)]
use super::*;
use super::update_part::*;
use super::calculator::*;
use crate::county::County;
use crate::math::clamp;
use crate::tables::Tables;
use l2_net::{Quirk, Quirks};

/// `FUN_00428C42` — buy ale for a county, in crowns' worth.
///
/// **This settles the published claim `docs/kingdom.md` §12 lists as
/// unverified.** The guides say *"+1 per 20% of the population, cap +5"*; the
/// step is `population / 10`, so it is **+1 per 10%** and the cap is right. The
/// panel's own preview (`FUN_00435673`) computes the identical ladder, which is
/// the second source.
pub fn buy_ale(t: &Tables, county: &mut County, crowns: i32, quirks: Quirks) -> i32 {
    if crowns <= 0 {
        return 0;
    }
    let step = county.population / t.ale.step_pct;
    let step = if step == 0 && !quirks.reproduces(Quirk::AnyAleFillsATinyVillage) {
        1
    } else {
        step
    };
    let mut bonus = 0;
    let mut rung = t.ale.max;
    while rung >= 1 {
        if crowns >= rung * step {
            bonus = rung;
            break;
        }
        rung -= 1;
    }
    let remaining = t.ale.max - county.ale_happiness_given;
    if bonus > remaining {
        bonus = remaining;
    }
    if bonus < 0 {
        bonus = 0;
    }
    county.ale_happiness_given += bonus;
    county.happiness += bonus;
    county.shown_ale += bonus;
    if county.happiness > HAPPINESS_MAX {
        county.happiness = HAPPINESS_MAX;
    }
    bonus
}

/// The `L2.eng` group 85 *"From army"* term — `FUN_004A9A9A`'s tail, the writer
/// `docs/kingdom.md` §12 records as not found.
pub fn raise_army(t: &Tables, county: &mut County, men: i32) -> i32 {
    if men <= 0 {
        return 0;
    }
    let share = crate::industry::pct_of(men, county.population);
    let cost = t.army_happiness_cost(share);
    let taken = if county.happiness < cost {
        let taken = county.happiness;
        county.happiness = 0;
        taken
    } else {
        county.happiness -= cost;
        cost
    };
    county.shown_army -= taken;
    taken
}

