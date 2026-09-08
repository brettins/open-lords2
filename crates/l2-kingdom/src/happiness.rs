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
//! `L2.eng` group 85 labels, and this pass **zeroes them**, exactly as the
//! original does — they are written when the player acts, not once a season.
//! `docs/kingdom.md` §12 records both writers as not found. Both are found now,
//! and they are [`buy_ale`] and [`raise_army`].
//!
//! The ordering consequence is worth stating, because it is the whole reason
//! both terms are "shown" fields rather than deltas: a season's
//! `Happiness_UpdateAll` **wipes** whatever ale and army did during the turn
//! before it, so their contribution to happiness is permanent but their
//! contribution to the *panel* lasts exactly one turn.

use crate::county::County;
use crate::math::clamp;
use crate::tables::Tables;

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
/// The cap is **cumulative and never reset** — see
/// [`crate::county::County::ale_happiness_given`]. Returns the happiness
/// actually gained, which is 0 once the county has had its five.
///
/// `crowns` is `price x quantity` at the call site. Ale's base price is 1
/// (`docs/kingdom.md` §10), so in the shipped game a barrel is a crown and the
/// two are the same number.
pub fn buy_ale(t: &Tables, county: &mut County, crowns: i32) -> i32 {
    if crowns <= 0 {
        return 0;
    }
    let step = county.population / t.ale.step_pct;
    let mut bonus = 0;
    // Counted upward rather than as the original's nested `if`s; the ladder is
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
/// cannot afford the full cost, `shownArmy` is debited only what was actually
/// taken, so the panel and the happiness always agree.
///
/// Returns the happiness actually lost.
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

/// The happiness a county holds steady at, given its three terms. Zero means
/// the county neither rises nor falls.
///
/// This is not a rule of its own — it is the sum [`update`] adds — but it is
/// the number every player-facing statement about the game is really about, so
/// it is worth being able to ask for directly.
pub fn steady_state(t: &Tables, tax_rate: i32, health_band: u8, ration_level: i32) -> i32 {
    (crate::tax::FREE_TAX_RATE - tax_rate)
        + crate::health::happiness(t, health_band)
        + t.ration_happiness(ration_level)
}

#[cfg(test)]
mod tests {
    use super::*;

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
            buy_ale(T, &mut c, crowns)
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
            total += buy_ale(T, &mut c, 250);
        }
        assert_eq!(total, T.ale.max);
        assert_eq!(c.ale_happiness_given, T.ale.max);
        assert_eq!(c.happiness, T.ale.max);
        assert_eq!(c.shown_ale, T.ale.max);
    }

    /// A county too small to have a tenth: `population / 10` is zero, every
    /// rung's threshold is zero, and any ale at all buys the full five.
    /// Reproduced rather than guarded, because the guard would be ours.
    #[test]
    fn a_county_of_nine_people_gets_the_whole_bonus_for_one_crown() {
        let mut c = County::new();
        c.population = 9;
        assert_eq!(buy_ale(T, &mut c, 1), 5);
    }

    #[test]
    fn ale_cannot_push_happiness_past_a_hundred() {
        let mut c = County::new();
        c.population = 100;
        c.happiness = 98;
        assert_eq!(buy_ale(T, &mut c, 1000), 5);
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
    /// debits only what was actually taken.
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
        buy_ale(T, &mut c, 250);
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
