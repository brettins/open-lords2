#![allow(unused_imports)]
use super::*;
use super::steps::*;
use super::grants::*;
use super::construction::*;
use super::industry::*;
use super::diplomacy::*;
use super::*;
use crate::county::County;
use crate::realm::{Realm, AI_STEP_DONE};
use crate::tables::{
    ai_grant_tier, tax_rate_for, Tables, AI_CASTLE_LADDER_LEN, AI_GOLD_GRANT_SMALL_COUNTIES,
    AI_WEAPON_ROTA_ORDER,
};

/// `AI_SetTaxRates`' first half — set every county the realm owns to the rate
/// its happiness earns, on one of four ladders.
///
/// `realm_lord` is the owning realm's `lord` byte, and `realm` **0 means the
/// unowned counties**, which phase 1 (`docs/kingdom.md` §3.1) runs once a turn
/// with [`crate::tables::AiTable::tax_ladder_neutral`]. An AI realm uses the
/// ladder its lord's personality names — see [`Tables::ai_tax_ladder`].
///
/// The four ladders are the piece `docs/kingdom.md` §8.2 says exists and does
/// not give. What they say, in one line each:
///
/// * **neutral** — eight rungs from 0% below 20 happiness up to 12% at 90;
/// * **ladder 0** — the greediest, 15% on anything at 80 or above;
/// * **ladder 1** — ladder 0 softened, topping out at 12%;
/// * **ladder 2** — the gentlest and the one three of the four lords use:
///   nothing at all below 60 happiness, and 10% only above 95.
///
/// A *neutral* county is taxed harder at low happiness than any AI
/// taxes its own — 1% at 20 happiness where every lord's ladder charges
/// nothing below 30. Nobody is collecting it, though: `Tax_CollectAll` banks an
/// unowned county's take into the county itself.
pub fn set_tax_rates(
    t: &Tables,
    counties: &mut [County],
    county_count: usize,
    realm: u8,
    realm_lord: u8,
) {
    let ladder = if realm == 0 {
        Some(&t.ai.tax_ladder_neutral)
    } else {
        t.ai_tax_ladder(realm_lord)
    };
// A lord with no personality record sets no rates at all.
    // falling back to a ladder that. See
    // `crate::tables::AI_PERSONALITY_COUNT`.
    let Some(ladder) = ladder else { return };
    for id in 1..=county_count {
        if counties[id].owner != realm {
            continue;
        }
        counties[id].tax_rate = tax_rate_for(ladder, counties[id].happiness);
    }
}

/// Step 14 — `FUN_0049D1E0`, which recomputes the realm-wide totals every other
/// pass reads.
///
/// This is where realm `+0x29` (the county count the grant tiers turn on) comes
/// from, and **five of the six score inputs `docs/kingdom.md` §8.3 could not
/// identify** — see [`crate::tables::SCORE_INPUT_OFFSETS`].
///
/// ```c
/// countyCount = 0; totalPopulation = 0; sumHappiness = 0; sumHealth = 0;
/// for each owned county { countyCount++; totalPopulation += pop;
///                         sumHappiness += happiness; sumHealth += healthMeter; }
/// meanPopulation = totalPopulation / countyCount;      /* +0x14 */
/// shareOfMap     = PctOf(countyCount, g_countyCount);  /* +0x60 */
/// meanHappiness  = sumHappiness / countyCount;         /* +0x0C */
/// meanHealth     = sumHealth / countyCount;            /* +0x58 */
/// ```
///
/// Every division is guarded on `countyCount != 0` and writes 0 instead
/// realm about to be eliminated does not divide by zero.
///
/// `armies` and `total_men` come from the unit array, which is not this
/// crate's; the caller supplies them and they land in `+0x2C` and `+0x54`.
pub fn update_realm_totals(
    realm: &mut Realm,
    counties: &[County],
    county_count: usize,
    realm_id: u8,
    armies: u8,
    total_men: i32,
) {
    realm.population_last = realm.population_total;
    realm.county_count = 0;
    realm.population_total = 0;
    let mut sum_happiness: i64 = 0;
    let mut sum_health: i64 = 0;
    for id in 1..=county_count {
        if counties[id].owner != realm_id {
            continue;
        }
        realm.county_count += 1;
        realm.population_total += counties[id].population;
        sum_happiness += counties[id].happiness as i64;
        sum_health += counties[id].health_meter as i64;
    }
    let n = realm.county_count as i64;
    if n == 0 {
        realm.population_mean = 0;
        realm.share_of_map_pct = 0;
        realm.mean_happiness = 0;
        realm.mean_health = 0;
    } else {
        realm.population_mean = (realm.population_total as i64 / n) as i32;
        realm.share_of_map_pct =
            crate::industry::pct_of(realm.county_count as i32, county_count as i32);
        realm.mean_happiness = (sum_happiness / n) as i32;
        realm.mean_health = (sum_health / n) as i32;
    }
    realm.army_count = armies;
    realm.total_men = total_men;
    realm.sync_score_inputs();
}

