//! Happiness — `docs/kingdom.md` §4.4, `Happiness_UpdateAll` (`0x0044BAEA`).
//!
//! ```text
//! county.happinessLast = county.happiness;
//! county.happiness += dHapTax + dHapHealth + dHapRation;
//! shownTax/shownHealth/shownRation = the three terms;  shownArmy/Events/Ale = 0;
//! if (population == 0 && owner is AI)      county.happiness = 50;
//! if (county.happiness < 75 && owner == 0) { county.happiness += 5; shownEvents = 5; }
//! clamp 0..100;
//! county.happinessSum += county.happiness;
//! county.happinessAvg  = county.happinessSum / g_turnCount;
//! ```
//!
//! Three terms, and the whole steady state of the layer falls out of them: a
//! county holds its happiness when `(5 - taxRate) + healthHappiness +
//! (3 x ration - 8) = 0`.
//!
//! The *army* and *ale* terms exist as fields and as `L2.eng` group 85 labels
//! but are written elsewhere; `docs/kingdom.md` §12 records that neither writer
//! was found, so this pass zeroes them exactly as the original does.

use crate::county::County;
use crate::math::clamp;

pub const HAPPINESS_MIN: i32 = 0;
pub const HAPPINESS_MAX: i32 = 100;

/// An AI-owned county with nobody left in it is pinned here rather than
/// drifting. `docs/kingdom.md` §4.4.
pub const EMPTY_AI_COUNTY_HAPPINESS: i32 = 50;

/// An unowned county below this is nudged up, and the nudge is reported in the
/// *"From events"* slot.
pub const UNOWNED_BONUS_THRESHOLD: i32 = 75;
pub const UNOWNED_BONUS: i32 = 5;

/// One county's happiness pass.
///
/// `owner_is_human` distinguishes the two owned cases; an unowned county
/// (`owner == 0`) is neither.
///
/// `turn_count` is `g_turnCount`, which is 1 on the first season — the average
/// is a plain division by it, so this must never be handed 0.
pub fn update(county: &mut County, owner_is_human: bool, turn_count: u32) {
    county.happiness_last = county.happiness;
    county.happiness += county.d_hap_tax + county.d_hap_health + county.d_hap_ration;

    county.shown_tax = county.d_hap_tax;
    county.shown_health = county.d_hap_health;
    county.shown_ration = county.d_hap_ration;
    county.shown_army = 0;
    county.shown_events = 0;
    county.shown_ale = 0;

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

/// The happiness a county holds steady at, given its three terms. Zero means
/// the county neither rises nor falls.
///
/// This is not a rule of its own — it is the sum [`update`] adds — but it is
/// the number every player-facing statement about the game is really about, so
/// it is worth being able to ask for directly.
pub fn steady_state(tax_rate: i32, health_band: u8, ration_level: i32) -> i32 {
    (crate::tax::FREE_TAX_RATE - tax_rate)
        + crate::health::happiness(health_band)
        + crate::tables::ration_happiness(ration_level)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn county_at(happiness: i32, tax: i32, health: i32, ration: i32) -> County {
        let mut c = County::new();
        c.happiness = happiness;
        c.d_hap_tax = tax;
        c.d_hap_health = health;
        c.d_hap_ration = ration;
        c
    }

    /// **`docs/kingdom.md` §9 point 5.** Every county in the shipped save has
    /// `happinessLast = 65`, `shownTax = +5`, `shownHealth = +1`,
    /// `shownRation = +1`. Player-owned counties store **72 = 65 + 5 + 1 + 1**;
    /// unowned counties store **77**, with `shownEvents = +5`.
    #[test]
    fn both_happiness_cases_in_the_shipped_save_reproduce() {
        let mut owned = county_at(65, 5, 1, 1);
        owned.owner = 1;
        update(&mut owned, true, 1);
        assert_eq!(owned.happiness_last, 65);
        assert_eq!(owned.happiness, 72, "65 + 5 + 1 + 1");
        assert_eq!((owned.shown_tax, owned.shown_health, owned.shown_ration), (5, 1, 1));
        assert_eq!(owned.shown_events, 0);

        let mut unowned = county_at(65, 5, 1, 1);
        unowned.owner = 0;
        update(&mut unowned, false, 1);
        assert_eq!(unowned.happiness, 77, "72 + the unowned bonus");
        assert_eq!(unowned.shown_events, 5);
    }

    /// The average is the running sum over the turn count, and after one turn
    /// it is just this turn's value.
    #[test]
    fn the_average_is_the_running_sum_over_the_turn_count() {
        let mut c = county_at(65, 5, 1, 1);
        c.owner = 1;
        update(&mut c, true, 1);
        assert_eq!(c.happiness_sum, 72);
        assert_eq!(c.happiness_avg, 72);

        c.d_hap_tax = 0;
        c.d_hap_health = 0;
        c.d_hap_ration = 0;
        update(&mut c, true, 2);
        assert_eq!(c.happiness_sum, 144);
        assert_eq!(c.happiness_avg, 72);
    }

    #[test]
    fn happiness_is_clamped_to_the_meter_at_both_ends() {
        let mut c = county_at(98, 5, 2, 7);
        c.owner = 1;
        update(&mut c, true, 1);
        assert_eq!(c.happiness, 100);

        let mut c = county_at(3, -20, -10, -8);
        c.owner = 1;
        update(&mut c, true, 1);
        assert_eq!(c.happiness, 0);
    }

    /// The bonus is a *threshold*, not a floor: an unowned county already at 75
    /// gets nothing.
    #[test]
    fn the_unowned_bonus_only_applies_below_seventy_five() {
        let mut c = county_at(75, 0, 0, 0);
        c.owner = 0;
        update(&mut c, false, 1);
        assert_eq!(c.happiness, 75);
        assert_eq!(c.shown_events, 0);

        let mut c = county_at(74, 0, 0, 0);
        c.owner = 0;
        update(&mut c, false, 1);
        assert_eq!(c.happiness, 79);
        assert_eq!(c.shown_events, 5);
    }

    #[test]
    fn an_emptied_ai_county_is_pinned_at_fifty_and_a_human_one_is_not() {
        let mut ai = county_at(10, -30, -10, -8);
        ai.owner = 2;
        ai.population = 0;
        update(&mut ai, false, 1);
        assert_eq!(ai.happiness, 50);

        let mut human = county_at(10, -30, -10, -8);
        human.owner = 1;
        human.population = 0;
        update(&mut human, true, 1);
        assert_eq!(human.happiness, 0, "the human's empty county is not rescued");
    }

    /// An emptied *unowned* county takes the unowned bonus, not the AI pin —
    /// the two clauses are tested in that order and `owner == 0` is not an AI.
    #[test]
    fn an_emptied_unowned_county_takes_the_bonus_rather_than_the_pin() {
        let mut c = county_at(20, 0, 0, 0);
        c.owner = 0;
        c.population = 0;
        update(&mut c, false, 1);
        assert_eq!(c.happiness, 25);
    }

    /// The army and ale terms are zeroed by this pass. `docs/kingdom.md` §12
    /// records that neither writer was ever found, so if either is ever
    /// non-zero after an update, something outside this crate wrote it.
    #[test]
    fn the_army_and_ale_terms_are_cleared_every_season() {
        let mut c = county_at(50, 0, 0, 0);
        c.shown_army = 9;
        c.shown_ale = 9;
        update(&mut c, true, 1);
        assert_eq!(c.shown_army, 0);
        assert_eq!(c.shown_ale, 0);
    }

    /// The published FAQ, and the sentence `docs/kingdom.md` §4.4 calls "the
    /// arithmetic in the guide and the arithmetic in the binary are the same
    /// arithmetic": Good health plus Normal rations holds at 7%, Perfect at 8%.
    #[test]
    fn a_county_holds_its_happiness_at_the_break_even_rate_forever() {
        for (band, rate) in [(3u8, 7), (4u8, 8)] {
            assert_eq!(steady_state(rate, band, 3), 0);
            let mut c = County::new();
            c.owner = 1;
            c.happiness = 100;
            for turn in 1..=40u32 {
                c.d_hap_tax = crate::tax::FREE_TAX_RATE - rate;
                c.d_hap_health = crate::health::happiness(band);
                c.d_hap_ration = crate::tables::ration_happiness(3);
                update(&mut c, true, turn);
                assert_eq!(c.happiness, 100, "band {band} rate {rate} turn {turn}");
            }
        }
        assert!(steady_state(8, 3, 3) < 0, "one point too many at Good health");
    }
}
