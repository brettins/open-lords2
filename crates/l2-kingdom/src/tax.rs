//! Taxation — `docs/kingdom.md` §4.1, `Tax_CollectAll` (`0x0044B59B`).
//!
//! ```text
//! base = f1A8 != 0 ? 0 : castleType == 0 ? 320 : {480,560,640,720,800}[castleType - 1];
//! take = Pct(Pct(population, base), taxRate);
//! county.taxCollected = take;   realm.gold += take;
//! county.dHapTax = (5 - taxRate) + realm.taxHapEmpire;
//! ```
//!
//! So a county with no castle yields `pop * 3.2 * taxRate / 100` crowns a
//! season, and a royal castle two and a half times that.
//!
//! **Tax costs happiness at one point per point of rate above 5.** A rate of 5
//! is free. That single expression, plus the health and ration terms, is why a
//! county at Good health on Normal rations holds its happiness at exactly 7%
//! and at Perfect health at 8% — which a published FAQ states in those words.

use crate::county::County;
use crate::math::pct;
use crate::realm::Realm;
use crate::tables::{Tables, MAX_TAX_RATE};

/// The tax rate that costs nothing. `dHapTax = 5 - rate`.
pub const FREE_TAX_RATE: i32 = 5;

/// The castle type `Tax_CollectAll` actually uses.
///
/// `docs/kingdom.md` §4.1: normally `castleType`, but when the untraced flag at
/// `+0x1C3` is set, the *lower* of `castleType` and the castle under
/// construction is used, and a zero `castleBuilding` forces type 0. The flag's
/// meaning is **`[D]`** — a siege or a partly razed castle would both fit and
/// neither is established.
pub fn effective_castle_type(county: &County) -> u8 {
    if !county.castle_degraded {
        return county.castle_type;
    }
    if county.castle_building == 0 {
        0
    } else {
        county.castle_type.min(county.castle_building)
    }
}

/// The population multiplier for a castle type, clamped into the table.
pub fn tax_base(t: &Tables, castle_type: u8) -> i32 {
    let table = &t.castle.tax_base;
    table[(castle_type as usize).min(table.len() - 1)]
}

/// What one county contributes to its realm's empire-wide tax happiness term
/// (county `+0x16`, *"Other counties"*, summed into realm `+0x28`).
///
/// **It is a table, and it was inferred wrongly until somebody read it.** `[V]`
///
/// `Tax_RecomputePreview` (`0x0044B80B`) is the only writer of `+0x16` anywhere
/// in the binary, and its last three statements settle the question:
///
/// ```c
/// county[0x0F] = 5 - taxRate;                        /* a *separate* field  */
/// county[0x16] = g_taxHappinessOther[taxRate * 4];   /* i32 stride, 51 rows */
/// Tax_SumEmpireHappiness(county.owner);
/// ```
///
/// `5 - rate` is real, but it is `+0x0F`. `+0x16` is a lookup in
/// `g_taxHappinessOther` (`0x004D63D8`), 51 entries for rates 0 …
/// [`MAX_TAX_RATE`]. The 52nd word is a zero no rate can reach, and
/// `0x004D63D8 + 52 * 4` is exactly `0x004D64A8`, where `g_healthDeltaTable`
/// begins — which is what fixes the length.
///
/// The old reading here was `min(5 - rate, 0)`, arrived at honestly: it
/// reproduces the England turn-one fixture and it matches the manual's *"outrageously high
/// taxes damage your other counties"*. It is also **wrong at 45 of the 51
/// rates** — flat until 21 in the binary against biting from 6 in ours, and
/// −15 against −45 at the top, a factor of three.
///
/// It survived because **every county in the only save we test sits at rate 0**,
/// which is one of the six columns where the two agree. See `docs/decisions.md`
/// C26.
pub fn empire_contribution(t: &Tables, tax_rate: i32) -> i32 {
    let rate = tax_rate.clamp(0, MAX_TAX_RATE) as usize;
    t.tax_happiness_other[rate]
}

/// Recompute every owned county's contribution and sum it into its realm.
///
/// Two passes over the counties, in index order: the sum has to be complete
/// before any county reads it, and a single pass would make a county's tax
/// happiness depend on its position in the array.
pub fn sum_empire_happiness(
    t: &Tables,
    counties: &mut [County],
    realms: &mut [Realm],
    county_count: usize,
) {
    for realm in realms.iter_mut() {
        realm.tax_hap_empire = 0;
    }
    for id in 1..=county_count {
        let contribution = empire_contribution(t, counties[id].tax_rate);
        counties[id].tax_hap_other = contribution;
        let owner = counties[id].owner as usize;
        if owner != 0 && owner < realms.len() {
            realms[owner].add_empire_tax_happiness(contribution);
        }
    }
}

/// Collect one county's tax and write its tax happiness term.
///
/// `empire` is the owning realm's `taxHapEmpire`; an unowned county has no
/// realm and therefore no empire term.
///
/// Returns what the treasury banks, which the caller credits — an unowned
/// county's take goes nowhere, because realm 0 is not a realm.
pub fn collect(t: &Tables, county: &mut County, empire: i32) -> i32 {
    let base = if county.tax_suppressed { 0 } else { tax_base(t, effective_castle_type(county)) };
    let take = pct(pct(county.population, base), county.tax_rate);
    county.tax_collected = take;
    // `taxShown` is L2.eng group 86 index 2, *"People pay"*. docs/kingdom.md
    // §1.3 lists it beside `taxCollected` without saying how the two ever
    // differ, so they are written the same until something says otherwise.
    county.tax_shown = take;
    county.d_hap_tax_local = FREE_TAX_RATE - county.tax_rate;
    county.d_hap_tax = county.d_hap_tax_local + empire;
    take
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::tables::{CASTLE_TAX_BONUS_PCT, CASTLE_TYPE_COUNT};

    /// The stock ruleset. Every test here runs on it explicitly, which is the
    /// point: the rules arrive as an argument rather than as a constant.
    const T: &Tables = &Tables::DEFAULT;

    fn county_with(pop: i32, rate: i32, castle: u8) -> County {
        let mut c = County::new();
        c.population = pop;
        c.tax_rate = rate;
        c.castle_type = castle;
        c
    }

    /// `pop * 3.2 * rate / 100` for a castle-less county, stated in
    /// `docs/kingdom.md` §4.1.
    #[test]
    fn a_castleless_county_yields_three_point_two_crowns_a_head_at_full_rate() {
        let mut c = county_with(1000, 100, 0);
        assert_eq!(collect(T, &mut c, 0), 3200);
        let mut c = county_with(1000, 10, 0);
        assert_eq!(collect(T, &mut c, 0), 320);
    }

    /// A royal castle yields two and a half times a castle-less county.
    #[test]
    fn a_royal_castle_yields_two_and_a_half_times_nothing_at_all() {
        let mut none = county_with(1000, 100, 0);
        let mut royal = county_with(1000, 100, 5);
        let a = collect(T, &mut none, 0);
        let b = collect(T, &mut royal, 0);
        assert_eq!(b * 2, a * 5, "{b} should be 2.5x {a}");
    }

    /// Every castle row against the published bonus percentages.
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

    /// `docs/kingdom.md` §9 point 6: `taxRate` is 0 everywhere in the shipped
    /// save and `dHapTax` is 5.
    #[test]
    fn the_shipped_save_s_tax_term_reproduces() {
        let mut c = county_with(417, 0, 3);
        collect(T, &mut c, 0);
        assert_eq!(c.d_hap_tax, 5);
        assert_eq!(c.tax_collected, 0, "a rate of 0 banks nothing");
    }

    /// The empire term as it has to behave for §9 to reproduce: with every
    /// rate at 0 the sum is 0, and no county's tax term moves.
    #[test]
    fn a_realm_at_the_free_rate_has_no_empire_term_at_all() {
        let mut counties = vec![County::new(); 5];
        let mut realms = vec![Realm::new(); 6];
        for id in 1..=4 {
            counties[id].owner = 1;
            counties[id].tax_rate = 0;
        }
        sum_empire_happiness(T, &mut counties, &mut realms, 4);
        assert_eq!(realms[1].tax_hap_empire, 0);
        for id in 1..=4 {
            assert_eq!(counties[id].tax_hap_other, 0);
        }
    }

    /// The manual's rule: *"if you set taxes outrageously high in one county,
    /// this will damage the happiness ratings of all your other counties."*
    #[test]
    fn one_punitive_county_poisons_the_whole_realm() {
        let mut counties = vec![County::new(); 5];
        let mut realms = vec![Realm::new(); 6];
        for id in 1..=4 {
            counties[id].owner = 1;
            counties[id].population = 400;
        }
        counties[2].tax_rate = 25;
        sum_empire_happiness(T, &mut counties, &mut realms, 4);

        // -2 from the table, not -20 from `min(5 - rate, 0)`. This assertion
        // read -20 until `g_taxHappinessOther` was actually read: the real
        // empire term is an order of magnitude gentler than the formula.
        assert_eq!(realms[1].tax_hap_empire, -2);

        let empire = realms[1].tax_hap_empire as i32;
        collect(T, &mut counties[1], empire);
        assert_eq!(counties[1].d_hap_tax, 5 - 2, "an untaxed county still suffers");
        collect(T, &mut counties[2], empire);
        assert_eq!(counties[2].d_hap_tax, (5 - 25) - 2, "and the culprit suffers twice");
    }

    /// The shape of the real table, which is nothing like the formula it
    /// replaced: **taxing at 19% costs the rest of the realm nothing at all**,
    /// and even the maximum rate costs only 15.
    ///
    /// Walks every rate rather than sampling, because the rule it replaced was
    /// wrong at 45 of 51 and survived on the six where they agree — one of
    /// which is rate 0, the only rate the England turn-one fixture contains.
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

        // Out of range clamps rather than panicking. The UI cannot produce
        // these; a mod or a corrupt save could.
        assert_eq!(empire_contribution(T, 999), -15);
        assert_eq!(empire_contribution(T, -5), 0);

        // For the record, and to stop anyone reinstating it: the old rule was
        // right at six rates out of 51.
        let agreements =
            (0..=MAX_TAX_RATE).filter(|&r| empire_contribution(T, r) == (5 - r).min(0)).count();
        assert_eq!(agreements, 6, "min(5 - rate, 0) agreed six times out of 51");
    }

    /// A second realm's rates never touch the first realm's counties.
    #[test]
    fn the_empire_term_does_not_cross_realms() {
        let mut counties = vec![County::new(); 5];
        let mut realms = vec![Realm::new(); 6];
        counties[1].owner = 1;
        counties[2].owner = 2;
        counties[2].tax_rate = 30;
        sum_empire_happiness(T, &mut counties, &mut realms, 2);
        assert_eq!(realms[1].tax_hap_empire, 0);
        assert_eq!(realms[2].tax_hap_empire, -3, "rate 30 is -3 in the table, not -25");
    }

    /// An unowned county's rate contributes to nobody.
    #[test]
    fn an_unowned_county_contributes_to_no_realm() {
        let mut counties = vec![County::new(); 3];
        let mut realms = vec![Realm::new(); 6];
        counties[1].owner = 0;
        counties[1].tax_rate = 40;
        sum_empire_happiness(T, &mut counties, &mut realms, 1);
        for r in realms.iter() {
            assert_eq!(r.tax_hap_empire, 0);
        }
        assert_eq!(counties[1].tax_hap_other, -7, "rate 40 is -7 in the table, not -35");
    }

    /// The **local** half of the term, at every rate the interface can set.
    ///
    /// `a_rate_of_five_is_free_and_every_point_above_costs_one_happiness`
    /// stops at 20, which is inside the flat head of the empire table — the
    /// region where the rule this crate had and the rule the binary has agree.
    /// A test that never leaves the region where two rules agree cannot tell
    /// them apart, which is exactly how the old one survived. So walk the lot.
    #[test]
    fn the_two_halves_of_the_tax_term_are_different_rules_at_every_legal_rate() {
        for rate in 0..=MAX_TAX_RATE {
            let mut counties = vec![County::new(); 2];
            let mut realms = vec![Realm::new(); 2];
            counties[1].owner = 1;
            counties[1].population = 400;
            counties[1].tax_rate = rate;
            sum_empire_happiness(T, &mut counties, &mut realms, 1);
            let empire = realms[1].tax_hap_empire as i32;
            collect(T, &mut counties[1], empire);

            // `+0x0F` really is `5 - rate`, unbanded and unclamped, at all 51.
            assert_eq!(counties[1].d_hap_tax_local, FREE_TAX_RATE - rate, "local, rate {rate}");
            // `+0x16` is the table, and it is *not* `min(5 - rate, 0)`.
            assert_eq!(counties[1].tax_hap_other, T.tax_happiness_other[rate as usize]);
            assert_eq!(counties[1].d_hap_tax, (FREE_TAX_RATE - rate) + empire, "sum, rate {rate}");
        }
    }

    /// The two independent readings of the ceiling, held against each other.
    ///
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

    /// **Collection is not clamped, and that is not an oversight.**
    ///
    /// `Tax_CollectAll` reads the rate byte and multiplies; the 0..=50 guard is
    /// in the *panel*, not in the rule. So a mod or a corrupt save can present
    /// a rate of 100 and it will be collected — while
    /// [`empire_contribution`] clamps, because it indexes an array.
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
        c.castle_degraded = true;
        c.castle_building = 2;
        assert_eq!(collect(T, &mut c, 0), pct(pct(1000, 560), 100));

        // A zero castle_building forces type 0 outright.
        let mut c = county_with(1000, 100, 5);
        c.castle_degraded = true;
        c.castle_building = 0;
        assert_eq!(collect(T, &mut c, 0), pct(pct(1000, 320), 100));
    }
}
