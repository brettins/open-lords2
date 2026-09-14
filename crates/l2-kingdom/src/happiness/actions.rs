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
/// ```c
/// if (crowns <= 0) return;
/// tenth = population / 10;
/// bonus = crowns >= 5*tenth ? 5 : crowns >= 4*tenth ? 4 : ... : crowns >= tenth ? 1 : 0;
/// if (bonus > 5 - alreadyGiven) bonus = 5 - alreadyGiven;
/// if (bonus < 0)                bonus = 0;
/// alreadyGiven += bonus;  happiness += bonus;  shownAle += bonus;
/// if (happiness > 99) happiness = 100;
/// ```
///
/// **This settles the published claim `docs/kingdom.md` §12 lists as
/// unverified.** The guides say *"+1 per 20% of the population, cap +5"*; the
/// step is `population / 10`, so it is **+1 per 10%** and the cap is right. The
/// panel's own preview (`FUN_00435673`) computes the identical ladder, which is
/// the second source.
///
/// The cap is **cumulative within a season**: [`update`] clears
/// `ale_happiness_given` every season, so a county can have five points of ale
/// happiness a season and no more. Returns the happiness.
/// is 0 once the county has had its five. See
/// [`crate::county::County::ale_happiness_given`].
///
/// `crowns` is `price x quantity` at the call site. Ale's base price is 1
/// (`docs/kingdom.md` §10), so in the shipped game a barrel is a crown and the
/// two are the same number.
///
/// **Switchable** — [`Quirk::AnyAleFillsATinyVillage`], `docs/bugs.md` B10.
/// With the quirk fixed a county whose `population / step_pct` is 0 buys one
/// rung per crown instead of all five for one, which is the ladder the rest of
/// the function is written to walk. The county's *own* ale allowance
/// (`ale_happiness_given`) still caps it, so the fix cannot make ale worth more
/// than five a season either way.
pub fn buy_ale(t: &Tables, county: &mut County, crowns: i32, quirks: Quirks) -> i32 {
    if crowns <= 0 {
        return 0;
    }
    let step = county.population / t.ale.step_pct;
    // A village under ten people has `step == 0`, and then `crowns >= rung * 0`
    // is true at the top rung for any ale at all.
    let step = if step == 0 && !quirks.reproduces(Quirk::AnyAleFillsATinyVillage) {
        1
    } else {
        step
    };
    let mut bonus = 0;
    // Counted upward;
    // the same. A county of fewer than ten people has `step == 0`, and the
    // original's `crowns >= 5 * 0` is then true at the top rung — so any ale at
    // all buys the full five. Reproduced: `0 * n` is 0 for every rung.
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
    // The original's clamp is `if (happiness > 99) happiness = 100`, which is
    // the same as clamping to 100 for any integer.
    if county.happiness > HAPPINESS_MAX {
        county.happiness = HAPPINESS_MAX;
    }
    bonus
}

/// The `L2.eng` group 85 *"From army"* term — `FUN_004A9A9A`'s tail, the writer
/// `docs/kingdom.md` §12 records as not found.
///
/// Raising men costs the county happiness, and the cost is a table lookup on
/// **the share of the county being taken**, not on the number of men:
///
/// ```c
/// share = PctOf(men, population);                 /* 50 men of 500 is 10 */
/// cost  = g_armyHappinessCost[share];             /* 10 -> 5 */
/// if (happiness < cost) { shownArmy -= happiness; happiness = 0; }
/// else                  { happiness -= cost;      shownArmy -= cost; }
/// ```
///
/// So it is progressive and steeply so: a twentieth of a county costs 2, a
/// tenth costs 5, a fifth costs 10, a quarter costs 19 and a half costs 90.
/// See
/// [`crate::tables::ARMY_HAPPINESS_COST`].
///
/// Note the asymmetry in the clamp, reproduced as written: when the county
/// cannot afford the full cost, `shownArmy` is debited only what.
/// taken, so the panel and the happiness always agree.
///
/// Returns the happiness.
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

