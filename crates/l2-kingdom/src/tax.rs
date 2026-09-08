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
use crate::tables::CASTLE_TAX_BASE;

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
pub fn tax_base(castle_type: u8) -> i32 {
    CASTLE_TAX_BASE[(castle_type as usize).min(CASTLE_TAX_BASE.len() - 1)]
}

/// What one county contributes to its realm's empire-wide tax happiness term
/// (county `+0x16`, *"Other counties"*, summed into realm `+0x28`).
///
/// **This is inferred, and it is the one place in this crate where the document
/// cannot be taken literally.** `docs/kingdom.md` §4.1 says
/// `dHapTax = (5 - rate) + realm.taxHapEmpire` and §2 says `taxHapEmpire` is
/// the sum of every owned county's `+0x16`. If `+0x16` were simply `5 - rate`,
/// then the shipped `lastturn.sav` — where every rate is 0 and there are four
/// player-owned counties — would give `taxHapEmpire = 20` and `dHapTax = 25`.
/// The save stores `shownTax = +5` (§9). So `+0x16` cannot be `5 - rate`.
///
/// Taking only the **negative** part reproduces the save (a rate of 0
/// contributes nothing) *and* matches the manual's statement of what the term
/// is for: *"if you set taxes outrageously high in one county, this will damage
/// the happiness ratings of all your other counties."* A generous county does
/// not subsidise its neighbours' mood; a punitive one poisons it.
pub fn empire_contribution(tax_rate: i32) -> i32 {
    (FREE_TAX_RATE - tax_rate).min(0)
}

/// Recompute every owned county's contribution and sum it into its realm.
///
/// Two passes over the counties, in index order: the sum has to be complete
/// before any county reads it, and a single pass would make a county's tax
/// happiness depend on its position in the array.
pub fn sum_empire_happiness(counties: &mut [County], realms: &mut [Realm], county_count: usize) {
    for realm in realms.iter_mut() {
        realm.tax_hap_empire = 0;
    }
    for id in 1..=county_count {
        let contribution = empire_contribution(counties[id].tax_rate);
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
pub fn collect(county: &mut County, empire: i32) -> i32 {
    let base = if county.tax_suppressed { 0 } else { tax_base(effective_castle_type(county)) };
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
        assert_eq!(collect(&mut c, 0), 3200);
        let mut c = county_with(1000, 10, 0);
        assert_eq!(collect(&mut c, 0), 320);
    }

    /// A royal castle yields two and a half times a castle-less county.
    #[test]
    fn a_royal_castle_yields_two_and_a_half_times_nothing_at_all() {
        let mut none = county_with(1000, 100, 0);
        let mut royal = county_with(1000, 100, 5);
        let a = collect(&mut none, 0);
        let b = collect(&mut royal, 0);
        assert_eq!(b * 2, a * 5, "{b} should be 2.5x {a}");
    }

    /// Every castle row against the published bonus percentages.
    #[test]
    fn every_castle_multiplier_matches_its_published_bonus() {
        let mut base = county_with(10_000, 100, 0);
        let base_take = collect(&mut base, 0);
        for t in 1..CASTLE_TYPE_COUNT as u8 {
            let mut c = county_with(10_000, 100, t);
            let take = collect(&mut c, 0);
            let expected = base_take + base_take * CASTLE_TAX_BONUS_PCT[t as usize - 1] / 100;
            assert_eq!(take, expected, "castle type {t}");
        }
    }

    #[test]
    fn a_rate_of_five_is_free_and_every_point_above_costs_one_happiness() {
        for rate in 0..=20 {
            let mut c = county_with(400, rate, 3);
            collect(&mut c, 0);
            assert_eq!(c.d_hap_tax, 5 - rate);
        }
        let mut c = county_with(400, 5, 3);
        collect(&mut c, 0);
        assert_eq!(c.d_hap_tax, 0, "rate 5 is the free rate");
    }

    /// `docs/kingdom.md` §9 point 6: `taxRate` is 0 everywhere in the shipped
    /// save and `dHapTax` is 5.
    #[test]
    fn the_shipped_save_s_tax_term_reproduces() {
        let mut c = county_with(417, 0, 3);
        collect(&mut c, 0);
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
        sum_empire_happiness(&mut counties, &mut realms, 4);
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
        counties[2].tax_rate = 25; // outrageous
        sum_empire_happiness(&mut counties, &mut realms, 4);
        assert_eq!(realms[1].tax_hap_empire, -20);

        let empire = realms[1].tax_hap_empire as i32;
        collect(&mut counties[1], empire);
        assert_eq!(counties[1].d_hap_tax, 5 - 20, "an untaxed county still suffers");
        collect(&mut counties[2], empire);
        assert_eq!(counties[2].d_hap_tax, (5 - 25) - 20, "and the culprit suffers twice");
    }

    /// A second realm's rates never touch the first realm's counties.
    #[test]
    fn the_empire_term_does_not_cross_realms() {
        let mut counties = vec![County::new(); 5];
        let mut realms = vec![Realm::new(); 6];
        counties[1].owner = 1;
        counties[2].owner = 2;
        counties[2].tax_rate = 30;
        sum_empire_happiness(&mut counties, &mut realms, 2);
        assert_eq!(realms[1].tax_hap_empire, 0);
        assert_eq!(realms[2].tax_hap_empire, -25);
    }

    /// An unowned county's rate contributes to nobody.
    #[test]
    fn an_unowned_county_contributes_to_no_realm() {
        let mut counties = vec![County::new(); 3];
        let mut realms = vec![Realm::new(); 6];
        counties[1].owner = 0;
        counties[1].tax_rate = 40;
        sum_empire_happiness(&mut counties, &mut realms, 1);
        for r in realms.iter() {
            assert_eq!(r.tax_hap_empire, 0);
        }
        assert_eq!(counties[1].tax_hap_other, -35, "the field is still written");
    }

    #[test]
    fn the_untraced_gate_at_1a8_stops_collection_dead() {
        let mut c = county_with(1000, 100, 5);
        c.tax_suppressed = true;
        assert_eq!(collect(&mut c, 0), 0);
        assert_eq!(c.d_hap_tax, 5 - 100, "but the happiness cost still lands");
    }

    #[test]
    fn a_degraded_castle_taxes_at_the_lower_of_the_two_types() {
        let mut c = county_with(1000, 100, 5);
        c.castle_degraded = true;
        c.castle_building = 2;
        assert_eq!(collect(&mut c, 0), pct(pct(1000, 560), 100));

        // A zero castle_building forces type 0 outright.
        let mut c = county_with(1000, 100, 5);
        c.castle_degraded = true;
        c.castle_building = 0;
        assert_eq!(collect(&mut c, 0), pct(pct(1000, 320), 100));
    }
}
