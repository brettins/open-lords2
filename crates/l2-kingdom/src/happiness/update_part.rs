#![allow(unused_imports)]
use super::*;
use super::actions::*;
use super::calculator::*;
use crate::county::County;
use crate::math::clamp;
use crate::tables::Tables;
use l2_net::{Quirk, Quirks};

///.
pub fn decay_levy_surcharge(county: &mut County) {
    if county.levy_surcharge != 0 {
        county.levy_surcharge -= LEVY_SURCHARGE_DECAY;
    }
}

pub fn update(county: &mut County, owner_is_human: bool, turn_count: u32) {
    decay_levy_surcharge(county);
    county.happiness_last = county.happiness;
    county.happiness += county.d_hap_tax + county.d_hap_health + county.d_hap_ration;

    county.shown_tax = county.d_hap_tax;
    county.shown_health = county.d_hap_health;
    county.shown_ration = county.d_hap_ration;
    county.shown_army = 0;
    county.shown_events = 0;
    county.shown_ale = 0;
    // **And the ale allowance itself.** `Happiness_UpdateAll` clears `+0x219`
    // in the same breath as the display field beside it, which makes the five
    // points a seasonal allowance.
    //
    // `docs/kingdom.md` §7.6, `docs/mechanics.md` and `docs/symbols.json` all
    // claimed. Reading the reset off the wrong line for a whole subsystem is
    // C53; the line is here.
    county.ale_happiness_given = 0;

    if county.population == 0 && !county.is_unowned() && !owner_is_human {
        county.happiness = EMPTY_AI_COUNTY_HAPPINESS;
    }
    if county.happiness < UNOWNED_BONUS_THRESHOLD && county.is_unowned() {
        county.happiness += UNOWNED_BONUS;
        county.shown_events = UNOWNED_BONUS;
    }

    county.happiness = clamp(county.happiness, HAPPINESS_MIN, HAPPINESS_MAX);
    county.happiness_sum += county.happiness;
    county.happiness_avg = if turn_count == 0 {
        county.happiness
    } else {
        county.happiness_sum / turn_count as i32
    };
}

