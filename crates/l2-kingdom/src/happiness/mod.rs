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
//! # The two terms this pass does not compute
//!
//! The *army* and *ale* terms exist as fields (`+0x15` and `+0x194`) and as
//! `L2.eng` group 85 labels, and this pass **zeroes them**.
//! original does — they are written when the player acts, not once a season.
//! `docs/kingdom.md` §12 records both writers as not found. Both are found now,
//! and they are [`buy_ale`] and [`raise_army`].
//!
//! The ordering consequence is worth stating, because it is the whole reason
//! both terms are "shown" fields:
//! `Happiness_UpdateAll` **wipes** whatever ale and army did during the turn
//! before it, so their contribution to happiness is permanent but their
//! contribution to the *panel* lasts exactly one turn.

mod update_part;
pub use update_part::*;
mod actions;
pub use actions::*;
mod calculator;
pub use calculator::*;

use crate::county::County;
use crate::math::clamp;
use crate::tables::Tables;
use l2_net::{Quirk, Quirks};

pub const HAPPINESS_MIN: i32 = 0;
pub const HAPPINESS_MAX: i32 = 100;

/// An AI-owned county with nobody left in it is pinned here.
/// drifting. `docs/kingdom.md` §4.4.
pub const EMPTY_AI_COUNTY_HAPPINESS: i32 = 50;

/// An unowned county below this is nudged up, and the nudge is reported in the
/// *"From events"* slot.
pub const UNOWNED_BONUS_THRESHOLD: i32 = 75;
pub const UNOWNED_BONUS: i32 = 5;

/// How much of the levy surcharge `Happiness_UpdateAll` gives back each season.
///
/// **This closes an open question in `docs/armies.md` §6.1**, which records
/// `Army_Create` writing 15 into county `+0x2F4` and says *"where it decays was
/// not traced"*. It decays here: `Happiness_UpdateAll` (`0x0044BAEA`) runs
/// `if (county[+0x2F4] != 0) county[+0x2F4] -= 5;` over counties
/// 1…`g_countyCount` at the top of its own pass, so a county that has just
/// raised an army is back to no surcharge after **three seasons**. `[D]`
pub const LEVY_SURCHARGE_DECAY: i32 = 5;

#[cfg(test)]
mod tests {
    use super::*;

    /// Faithful. The switched-off answers live in `tests/quirks.rs`.
    const Q: Quirks = Quirks::FAITHFUL;

    /// The stock ruleset. Every rule below takes it as an argument now.
    const T: &Tables = &Tables::DEFAULT;

    fn county_at(happiness: i32, tax: i32, health: i32, ration: i32) -> County {
        let mut c = County::new();
        c.happiness = happiness;
        c.d_hap_tax = tax;
        c.d_hap_health = health;
        c.d_hap_ration = ration;
        c
    }

    /// **`docs/kingdom.md` §9 point 5.** Every county in the England turn-one fixture has
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
            assert_eq!(steady_state(T, rate, band, 3), 0);
            let mut c = County::new();
            c.owner = 1;
            c.happiness = 100;
            for turn in 1..=40u32 {
                c.d_hap_tax = crate::tax::FREE_TAX_RATE - rate;
                c.d_hap_health = crate::health::happiness(T, band);
                c.d_hap_ration = crate::tables::ration_happiness(3);
                update(&mut c, true, turn);
                assert_eq!(c.happiness, 100, "band {band} rate {rate} turn {turn}");
            }
        }
        assert!(steady_state(T, 8, 3, 3) < 0, "one point too many at Good health");
    }

    // --- the two terms this pass zeroes ------------------------------------

    /// **`+1 per 10% of the population, cap +5`** — and the published claim of
    /// *"+1 per 20%"* is twice too coarse.
    #[test]
    fn ale_is_worth_one_happiness_per_tenth_of_the_county() {
        let bought = |crowns: i32| {
            let mut c = County::new();
            c.population = 500;
            buy_ale(T, &mut c, crowns, Q)
        };
        assert_eq!(bought(0), 0, "and nothing at all for nothing");
        assert_eq!(bought(49), 0, "just under a tenth");
        assert_eq!(bought(50), 1, "a tenth of 500");
        assert_eq!(bought(99), 1);
        assert_eq!(bought(100), 2);
        assert_eq!(bought(150), 3);
        assert_eq!(bought(200), 4);
        assert_eq!(bought(250), 5, "half the county, and the cap");
        assert_eq!(bought(10_000), 5, "no more however much is bought");
        // The published "+1 per 20%" would put +1 at 100 crowns, not 50.
        assert_eq!(bought(100), 2, "which the binary says is +2");
    }

    /// **The cap is cumulative and nothing ever resets it.** A county gets five
    /// happiness from ale for the whole game, not five a season.
    #[test]
    fn a_county_can_be_given_five_happiness_from_ale_in_its_whole_history() {
        let mut c = County::new();
        c.population = 500;
        let mut total = 0;
        for _ in 0..20 {
            total += buy_ale(T, &mut c, 250, Q);
        }
        assert_eq!(total, T.ale.max);
        assert_eq!(c.ale_happiness_given, T.ale.max);
        assert_eq!(c.happiness, T.ale.max);
        assert_eq!(c.shown_ale, T.ale.max);
    }

    /// A county too small to have a tenth: `population / 10` is zero, every
    /// rung's threshold is zero, and any ale at all buys the full five.
    /// Reproduced.
    #[test]
    fn a_county_of_nine_people_gets_the_whole_bonus_for_one_crown() {
        let mut c = County::new();
        c.population = 9;
        assert_eq!(buy_ale(T, &mut c, 1, Q), 5);
    }

    #[test]
    fn ale_cannot_push_happiness_past_a_hundred() {
        let mut c = County::new();
        c.population = 100;
        c.happiness = 98;
        assert_eq!(buy_ale(T, &mut c, 1000, Q), 5);
        assert_eq!(c.happiness, HAPPINESS_MAX);
    }

    /// **The army term** — `L2.eng` group 85 *"From army"*, whose writer
    /// `docs/kingdom.md` §12 records as not found.
    #[test]
    fn raising_men_costs_happiness_by_the_share_of_the_county_taken() {
        let cost = |population: i32, men: i32| {
            let mut c = County::new();
            c.population = population;
            c.happiness = 100;
            let taken = raise_army(T, &mut c, men);
            assert_eq!(c.happiness, 100 - taken);
            assert_eq!(c.shown_army, -taken, "the panel shows the same number, negated");
            taken
        };
        // 50 men is a twentieth of 1000 and a tenth of 500.
        assert_eq!(cost(1000, 50), 2);
        assert_eq!(cost(500, 50), 5);
        assert_eq!(cost(250, 50), 10, "a fifth of the county");
        assert_eq!(cost(200, 50), 19, "a quarter");
        assert_eq!(cost(100, 50), 90, "half a county is ruinous");
        assert_eq!(cost(1000, 0), 0, "and nobody is free");
    }

    /// The cost is progressive and steeply so — the whole reason a player
    /// raises men from a big county.
    #[test]
    fn the_army_cost_never_falls_as_the_share_rises() {
        let mut last = -1;
        for share in 0..=101 {
            let c = T.army_happiness_cost(share);
            assert!(c >= last, "cost fell at {share}");
            last = c;
        }
        assert_eq!(T.army_happiness_cost(0), 0);
        assert_eq!(T.army_happiness_cost(101), 101);
        assert_eq!(T.army_happiness_cost(200), 101, "clamped, not read past the end");
    }

    /// A county that cannot afford the cost is taken to zero and the panel
    /// debits only what.
    #[test]
    fn a_poor_county_pays_what_it_has_and_the_panel_agrees() {
        let mut c = County::new();
        c.population = 200;
        c.happiness = 10;
        let taken = raise_army(T, &mut c, 50);
        assert_eq!(taken, 10, "19 was the price, 10 was all there was");
        assert_eq!(c.happiness, 0);
        assert_eq!(c.shown_army, -10);
    }

    /// The season's pass wipes both, so their effect on happiness is permanent
    /// and their effect on the panel lasts one turn.
    #[test]
    fn the_seasons_pass_wipes_what_ale_and_the_army_wrote() {
        let mut c = County::new();
        c.owner = 1;
        c.population = 500;
        c.happiness = 50;
        buy_ale(T, &mut c, 250, Q);
        raise_army(T, &mut c, 50);
        assert_eq!(c.shown_ale, 5);
        assert_eq!(c.shown_army, -5);
        let carried = c.happiness;

        update(&mut c, true, 1);
        assert_eq!(c.shown_ale, 0);
        assert_eq!(c.shown_army, 0);
        assert_eq!(c.happiness, carried, "but the happiness itself is kept");
    }
}

