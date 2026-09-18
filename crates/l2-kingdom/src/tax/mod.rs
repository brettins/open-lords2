//! Taxation — `docs/kingdom.md` §4.1, `Tax_CollectAll` (`0x0044B59B`).

mod calc;
pub use calc::*;

use crate::county::County;
use crate::math::pct;
use crate::realm::Realm;
use crate::tables::{Tables, MAX_TAX_RATE};
use l2_net::{Quirk, Quirks};

pub const FREE_TAX_RATE: i32 = 5;

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    const Q: Quirks = Quirks::FAITHFUL;

    use crate::tables::{CASTLE_TAX_BONUS_PCT, CASTLE_TYPE_COUNT};

    const T: &Tables = &Tables::DEFAULT;

    fn county_with(pop: i32, rate: i32, castle: u8) -> County {
        let mut c = County::new();
        c.population = pop;
        c.tax_rate = rate;
        c.castle_type = castle;
        c
    }

    #[test]
    fn a_castleless_county_yields_three_point_two_crowns_a_head_at_full_rate() {
        let mut c = county_with(1000, 100, 0);
        assert_eq!(collect(T, &mut c, 0), 3200);
        let mut c = county_with(1000, 10, 0);
        assert_eq!(collect(T, &mut c, 0), 320);
    }

    #[test]
    fn a_royal_castle_yields_two_and_a_half_times_nothing_at_all() {
        let mut none = county_with(1000, 100, 0);
        let mut royal = county_with(1000, 100, 5);
        let a = collect(T, &mut none, 0);
        let b = collect(T, &mut royal, 0);
        assert_eq!(b * 2, a * 5, "{b} should be 2.5x {a}");
    }

    #[test]
    fn every_castle_multiplier_matches_its_published_bonus() {
        let mut base = county_with(10_000, 100, 0);
        let base_take = collect(T, &mut base, 0);
        for castle in 1..CASTLE_TYPE_COUNT as u8 {
            let mut c = county_with(10_000, 100, castle);
            let take = collect(T, &mut c, 0);
            let expected = base_take + base_take * CASTLE_TAX_BONUS_PCT[castle as usize - 1] / 100;
            assert_eq!(take, expected, "castle type {castle}");
        }
    }

    #[test]
    fn a_rate_of_five_is_free_and_every_point_above_costs_one_happiness() {
        for rate in 0..=20 {
            let mut c = county_with(400, rate, 3);
            collect(T, &mut c, 0);
            assert_eq!(c.d_hap_tax, 5 - rate);
        }
        let mut c = county_with(400, 5, 3);
        collect(T, &mut c, 0);
        assert_eq!(c.d_hap_tax, 0, "rate 5 is the free rate");
    }

    #[test]
    fn the_shipped_save_s_tax_term_reproduces() {
        let mut c = county_with(417, 0, 3);
        collect(T, &mut c, 0);
        assert_eq!(c.d_hap_tax, 5);
        assert_eq!(c.tax_collected, 0, "a rate of 0 banks nothing");
    }

    #[test]
    fn a_realm_at_the_free_rate_has_no_empire_term_at_all() {
        let mut counties = vec![County::new(); 5];
        let mut realms = vec![Realm::new(); 6];
        for id in 1..=4 {
            counties[id].owner = 1;
            counties[id].tax_rate = 0;
        }
        sum_empire_happiness(T, &mut counties, &mut realms, 4, Q);
        assert_eq!(realms[1].tax_hap_empire, 0);
        for id in 1..=4 {
            assert_eq!(counties[id].tax_hap_other, 0);
        }
    }

    #[test]
    fn one_punitive_county_poisons_the_whole_realm() {
        let mut counties = vec![County::new(); 5];
        let mut realms = vec![Realm::new(); 6];
        for id in 1..=4 {
            counties[id].owner = 1;
            counties[id].population = 400;
        }
        counties[2].tax_rate = 25;
        sum_empire_happiness(T, &mut counties, &mut realms, 4, Q);

        assert_eq!(realms[1].tax_hap_empire, -2);

        let empire = realms[1].tax_hap_empire as i32;
        collect(T, &mut counties[1], empire);
        assert_eq!(counties[1].d_hap_tax, 5 - 2, "an untaxed county still suffers");
        collect(T, &mut counties[2], empire);
        assert_eq!(counties[2].d_hap_tax, (5 - 25) - 2, "and the culprit suffers twice");
    }

    #[test]
    fn the_empire_tax_term_is_flat_until_twenty_and_gentle_after() {
        for rate in 0..=19 {
            assert_eq!(empire_contribution(T, rate), 0, "rate {rate} must cost nothing");
        }
        assert_eq!(empire_contribution(T, 20), -1, "the ramp starts at 20");
        assert_eq!(empire_contribution(T, MAX_TAX_RATE), -15, "and ends at -15");

        let mut previous = 0;
        for rate in 0..=MAX_TAX_RATE {
            let v = empire_contribution(T, rate);
            assert!(v <= previous, "rate {rate} must not be kinder than rate {}", rate - 1);
            assert!(v >= -15, "rate {rate} falls through the table's floor");
            previous = v;
        }

        assert_eq!(empire_contribution(T, 999), -15);
        assert_eq!(empire_contribution(T, -5), 0);

        let agreements =
            (0..=MAX_TAX_RATE).filter(|&r| empire_contribution(T, r) == (5 - r).min(0)).count();
        assert_eq!(agreements, 6, "min(5 - rate, 0) agreed six times out of 51");
    }

    #[test]
    fn the_empire_term_does_not_cross_realms() {
        let mut counties = vec![County::new(); 5];
        let mut realms = vec![Realm::new(); 6];
        counties[1].owner = 1;
        counties[2].owner = 2;
        counties[2].tax_rate = 30;
        sum_empire_happiness(T, &mut counties, &mut realms, 2, Q);
        assert_eq!(realms[1].tax_hap_empire, 0);
        assert_eq!(realms[2].tax_hap_empire, -3, "rate 30 is -3 in the table, not -25");
    }

    #[test]
    fn an_unowned_county_contributes_to_no_realm() {
        let mut counties = vec![County::new(); 3];
        let mut realms = vec![Realm::new(); 6];
        counties[1].owner = 0;
        counties[1].tax_rate = 40;
        sum_empire_happiness(T, &mut counties, &mut realms, 1, Q);
        for r in realms.iter() {
            assert_eq!(r.tax_hap_empire, 0);
        }
        assert_eq!(counties[1].tax_hap_other, -7, "rate 40 is -7 in the table, not -35");
    }

    #[test]
    fn the_two_halves_of_the_tax_term_are_different_rules_at_every_legal_rate() {
        for rate in 0..=MAX_TAX_RATE {
            let mut counties = vec![County::new(); 2];
            let mut realms = vec![Realm::new(); 2];
            counties[1].owner = 1;
            counties[1].population = 400;
            counties[1].tax_rate = rate;
            sum_empire_happiness(T, &mut counties, &mut realms, 1, Q);
            let empire = realms[1].tax_hap_empire as i32;
            collect(T, &mut counties[1], empire);

            // `+0x0F` really is `5 - rate`, unbanded and unclamped, at all 51.
            assert_eq!(counties[1].d_hap_tax_local, FREE_TAX_RATE - rate, "local, rate {rate}");
            // `+0x16` is the table, and it is *not* `min(5 - rate, 0)`.
            assert_eq!(counties[1].tax_hap_other, T.tax_happiness_other[rate as usize]);
            assert_eq!(counties[1].d_hap_tax, (FREE_TAX_RATE - rate) + empire, "sum, rate {rate}");
        }
    }

    /// `Tax_IncreaseCounty` (`0x0043AA83`) guards `taxRate < 0x32`, so the rate
    /// tops out at 50; and `g_taxHappinessOther` holds one entry per rate. If
    /// either reading were wrong the other would not fit it.
    #[test]
    fn the_tax_table_is_exactly_one_row_per_settable_rate() {
        assert_eq!(MAX_TAX_RATE, 50);
        assert_eq!(
            T.tax_happiness_other.len(),
            MAX_TAX_RATE as usize + 1,
            "one row per rate 0..=50, and the 52nd word is g_healthDeltaTable's neighbour"
        );
    }

    #[test]
    fn the_ceiling_is_the_panels_and_the_collection_rule_never_sees_it() {
        let mut c = county_with(1000, 2 * MAX_TAX_RATE, 0);
        assert_eq!(collect(T, &mut c, 0), 3200, "rate 100 collects at rate 100");
        assert_eq!(c.d_hap_tax_local, FREE_TAX_RATE - 100);
        assert_eq!(
            empire_contribution(T, 2 * MAX_TAX_RATE),
            empire_contribution(T, MAX_TAX_RATE),
            "but the table lookup saturates rather than reading past its end"
        );
    }

    #[test]
    fn the_untraced_gate_at_1a8_stops_collection_dead() {
        let mut c = county_with(1000, 100, 5);
        c.tax_suppressed = true;
        assert_eq!(collect(T, &mut c, 0), 0);
        assert_eq!(c.d_hap_tax, 5 - 100, "but the happiness cost still lands");
    }

    #[test]
    fn a_degraded_castle_taxes_at_the_lower_of_the_two_types() {
        let mut c = county_with(1000, 100, 5);
        c.castle_degraded = crate::siege::CASTLE_DEGRADED_BUILDING;
        c.castle_building = 2;
        assert_eq!(collect(T, &mut c, 0), pct(pct(1000, 560), 100));

        let mut c = county_with(1000, 100, 5);
        c.castle_degraded = crate::siege::CASTLE_DEGRADED_BUILDING;
        c.castle_building = 0;
        assert_eq!(collect(T, &mut c, 0), pct(pct(1000, 320), 100));
    }

    #[test]
    fn a_lordless_county_banks_its_tax_in_its_own_purse() {
        let mut realms = vec![Realm::new(); crate::realm::MAX_REALMS];
        for (pop, was, then) in [(580, 186, 297), (634, 195, 316)] {
            let mut c = county_with(pop, 6, 0);
            c.owner = 0;
            c.purse = was;
            let take = collect(T, &mut c, 0);
            bank(&mut c, &mut realms, take);
            assert_eq!(c.purse, then, "{pop} people at rate 6 bank {take} into {was}");
            assert_eq!(realms[0].gold, 0, "realm 0 is not a treasury and must not be credited");
            assert_eq!(realms[0].tax_ledger, [0, 0], "nor is its +0xF4/+0xF8 pair");
        }
    }

    /// And an owned one still banks into its realm, which is the other limb of
    /// the same branch — **three writes, not one**: the treasury, then
    /// `+0xF4`, then `+0xF8` (`0x0044B59B`). A second county's take lands on
    /// all three again, so the pair is an accumulator and not a copy of the
    /// last take.
    ///
    /// *Ablation*: delete either `tax_ledger` line in [`bank`] and the matching
    /// slot's assertion goes red while the gold one stays green.
    #[test]
    fn an_owned_county_banks_its_tax_in_its_realms_gold() {
        let mut realms = vec![Realm::new(); crate::realm::MAX_REALMS];
        let mut c = county_with(1000, 10, 0);
        c.owner = 2;
        c.purse = 500;
        let take = collect(T, &mut c, 0);
        bank(&mut c, &mut realms, take);
        assert_eq!(take, 320);
        assert_eq!(realms[2].gold, 320);
        assert_eq!(realms[2].tax_ledger, [320, 320], "+0xF4 and +0xF8 take the same number");
        assert_eq!(c.purse, 500, "the county's own purse is untouched when it has a lord");

        let mut d = county_with(500, 10, 0);
        d.owner = 2;
        let second = collect(T, &mut d, 0);
        bank(&mut d, &mut realms, second);
        assert_eq!(realms[2].tax_ledger, [320 + second, 320 + second], "and they accumulate");
    }
}

