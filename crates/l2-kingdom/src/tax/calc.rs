#![allow(unused_imports)]
use super::*;

use crate::county::County;
use crate::math::pct;
use crate::realm::Realm;
use crate::tables::{Tables, MAX_TAX_RATE};
use l2_net::{Quirk, Quirks};

/// `docs/kingdom.md` §4.1: normally `castleType`, but when the untraced flag at
/// `+0x1C3` is set, the *lower* of `castleType` and the castle under
/// construction is used, and a zero `castleBuilding` forces type 0. The flag's
/// meaning is **`[D]`** — a siege or a partly razed castle would both fit and
/// neither is established.
pub fn effective_castle_type(county: &County) -> u8 {
    if county.castle_degraded == 0 {
        return county.castle_type;
    }
    if county.castle_building == 0 {
        0
    } else {
        county.castle_type.min(county.castle_building)
    }
}

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
/// `5 - rate` is real, but it is `+0x0F`. `+0x16` is a lookup in
/// `g_taxHappinessOther` (`0x004D63D8`), 51 entries for rates 0 …
/// [`MAX_TAX_RATE`]. The 52nd word is a zero no rate can reach, and
/// `0x004D63D8 + 52 * 4` is exactly `0x004D64A8`, where `g_healthDeltaTable`
/// begins — which is what fixes the length.
///
/// It survived because **every county in the only save we test sits at rate 0**,
/// which is one of the six columns where the two agree. See `docs/decisions.md`
/// C26.
pub fn empire_contribution(t: &Tables, tax_rate: i32) -> i32 {
    let rate = tax_rate.clamp(0, MAX_TAX_RATE) as usize;
    t.tax_happiness_other[rate]
}

/// `Tax_SumEmpireHappiness` (`0x0044B99A`) sums signed bytes into a signed byte
/// with nothing clamping it, and sixteen counties at −15 is −240, which wraps
/// to +16 — so taxing a large empire hard enough can make its people happier.
pub fn sum_empire_happiness(
    t: &Tables,
    counties: &mut [County],
    realms: &mut [Realm],
    county_count: usize,
    quirks: Quirks,
) {
    let faithful = quirks.reproduces(Quirk::EmpireTaxHappinessWraps);
    let mut totals = [0i32; crate::realm::MAX_REALMS];
    for realm in realms.iter_mut() {
        realm.tax_hap_empire = 0;
    }
    for id in 1..=county_count {
        let contribution = empire_contribution(t, counties[id].tax_rate);
        counties[id].tax_hap_other = contribution;
        let owner = counties[id].owner as usize;
        if owner != 0 && owner < realms.len() {
            if faithful {
                realms[owner].add_empire_tax_happiness(contribution, quirks);
            } else if owner < totals.len() {
                totals[owner] += contribution;
            }
        }
    }
    if !faithful {
        for (owner, realm) in realms.iter_mut().enumerate() {
            if let Some(total) = totals.get(owner) {
                realm.set_empire_tax_happiness(*total);
            }
        }
    }
}

/// `Tax_RecomputePreview` (`0x0044B80B`) — the two happiness terms the tax
/// panel draws, without collecting anything.
///
/// It is **the only writer of `+0x16`** anywhere in the binary, and it writes
/// two different things to two different fields: `+0x0F` is the local half,
/// `5 - rate`, and `+0x16` is the empire half, which is a table lookup and not
/// that. `docs/kingdom.md` §4.1 and `docs/decisions.md` C26.
///
/// The missing one is **`taxShown` (`+0xC0`)**, which is `L2.eng` 86/2,
/// *"People pay"* — the number on the tax panel. Verbatim:
///
/// `docs/kingdom.md` §1.3 lists the two fields side by side and says it does not
/// know how they ever differ; this is how. `[D]` — a reading of the two
/// functions, not an observation.
pub fn recompute_preview(t: &Tables, county: &mut County) {
    let base = tax_base(t, effective_castle_type(county));
    county.tax_shown = pct(pct(county.population, base), county.tax_rate);
    county.d_hap_tax_local = FREE_TAX_RATE - county.tax_rate;
    county.tax_hap_other = empire_contribution(t, county.tax_rate);
}

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

/// **`+0xF4` and `+0xF8` are [`Realm::tax_ledger`], and they are not the trade
/// pair.** [`Realm::trade_received_a`] and `_b` are `+0x10C` and `+0x110`,
/// which `Merchant_Trade` writes; these are a third accumulator pair that only
/// this branch writes. They used to be named here and not ported, on the
/// reading that *"nothing has been found that reads either"* — which is still
/// true, and is now established: no function in
/// the decompilation names them except this one and the new-game clear
/// instruction in `Lords2.exe` carries either absolute address except those
/// two functions' four. See [`Realm::tax_ledger`] for the scan and what it
/// cannot see. **Unread is not unstored**: the realm block is in the save and
/// `Sync_CompareState` compares it, so the pair is carried — credited here,
/// saved, and imported from a `.sav` —
///
/// **`[V]`**, read out of `0x0044B59B` at the moment of writing. `docs/symbols.md`
/// said only *"credited to the owner realm's gold"* and that sentence is why an
/// unowned county's tax went nowhere here for months — the `realm == 0` arm is
/// not an edge case, it is one of the branch's two limbs and it is the one that
/// funds every trade a lordless county makes.
///
/// What it costs when it is missing is measurable and was measured: an unowned
/// county's purse is the only thing `Ai_BuyGood` tests before buying grain
/// purse permanently at zero is a county that never shops. On the `old_turn` →
/// `battle-before` pair the original's counties 1 and 3 each buy the cascade's
/// 50-sack lot for 200 crowns out of purses of 297 and 316 — and on the
/// *previous* turn, with 186 and 195 in the same purses, neither can afford it
/// and neither buys. `docs/decisions.md` C149.
pub fn bank(county: &mut County, realms: &mut [Realm], take: i32) {
    let owner = county.owner as usize;
    if owner == 0 {
        county.purse += take;
    } else if let Some(realm) = realms.get_mut(owner) {
        // `0x0044B59B`'s order: the treasury, then `+0xF4`, then `+0xF8`.
        realm.gold += take;
        realm.tax_ledger[0] += take;
        realm.tax_ledger[1] += take;
    }
}

