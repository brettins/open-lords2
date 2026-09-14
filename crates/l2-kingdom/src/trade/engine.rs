#![allow(unused_imports)]
use super::*;
use super::tests_part::*;
use crate::county::County;
use crate::kingdom::Kingdom;
use crate::math::pct;
use crate::realm::Realm;
use crate::tables::{Tables, WEAPON_TYPE_COUNT};
use l2_net::{Quirk, Quirks};

/// How much of `good` the seller holds — the positive form of [`min_qty`], and
/// the same lookup `Merchant_Trade`'s refusal uses.
pub fn stock(counties: &[County], realms: &[Realm], good: Good, realm: usize, county: usize) -> i32 {
    let Some(c) = counties.get(county) else {
        return 0;
    };
    let Some(r) = realms.get(realm) else {
        return 0;
    };
    match good {
        Good::Grain => c.grain,
        Good::Cattle => c.herd,
        Good::Iron => r.iron,
        Good::Stone => r.stone,
        Good::Timber => r.wood,
        Good::Ale | Good::Sheep | Good::Wool => 0,
        other => other.weapon_slot().map_or(0, |s| r.weapons[s]),
    }
}

/// `Merchant_Trade` (`0x004284CE`).
///
/// `Ok(Receipt::default())` for a zero quantity: the original's whole body is
/// inside `if (qty != 0)`.
/// recomputes the county.
///
/// # Two guards that only fire for an owned county
///
/// Both the stock check and the gold check are inside `if (realm != 0)`. An
/// **unowned** county trading on its own account is checked for neither: it can
/// sell grain it does not have and buy with a purse it has emptied, and the
/// stores and the purse both go negative. Reproduced,
/// reaches this path with a realm of 0 for every unowned county on the map and
/// its own purse test is the only thing standing in front of it — so the
/// behaviour is live, not theoretical. `docs/bugs.md`.
pub fn trade(kingdom: &mut Kingdom, order: Order) -> Result<Receipt, Refusal> {
    if order.qty == 0 {
        return Ok(Receipt::default());
    }
    if order.county >= kingdom.counties.len() || order.realm >= kingdom.realms.len() {
        return Ok(Receipt::default());
    }
    let owned = order.realm != 0;
    // **Switchable** - [`Quirk::UnownedCountyTradesUnchecked`], `docs/bugs.md`
    // B11a. Both of the original.s guards are inside `if (realm != 0)`; with the
    // quirk fixed they are asked of an unowned county too, against the two
// things such a county has - its own stores and its own purse.
    let checked = owned || !kingdom.options.quirks.reproduces(Quirk::UnownedCountyTradesUnchecked);
    let mut receipt = Receipt::default();

    if order.qty < 0 {
        let want = -order.qty;
        receipt.crowns = order.sell_price * want;
        if checked
            && order.good.tradeable()
            && order.good != Good::Ale
            && stock(&kingdom.counties, &kingdom.realms, order.good, order.realm, order.county) < want
        {
            return Err(Refusal::NotEnoughStock);
        }
        receipt.moved = move_stock(kingdom, order.good, order.qty, order.realm, order.county);
        if owned {
            kingdom.realms[order.realm].gold += receipt.crowns;
            // Both accumulators, in the original's order. Neither is ever read
            // and neither is ever reset — see [`Realm::trade_received`].
            kingdom.realms[order.realm].trade_received_b += receipt.crowns;
            kingdom.realms[order.realm].trade_received_a += receipt.crowns;
        } else {
            kingdom.counties[order.county].purse += receipt.crowns;
        }
    } else {
        receipt.crowns = order.buy_price * order.qty;
        let purse = if owned { kingdom.realms[order.realm].gold } else { kingdom.counties[order.county].purse };
        if checked && purse < receipt.crowns {
            return Err(Refusal::NotEnoughGold);
        }
        if order.good == Good::Ale {
            let t = kingdom.tables;
            let quirks = kingdom.options.quirks;
            receipt.ale_happiness = crate::happiness::buy_ale(
                &t,
                &mut kingdom.counties[order.county],
                receipt.crowns,
                quirks,
            );
        } else {
            receipt.moved = move_stock(kingdom, order.good, order.qty, order.realm, order.county);
        }
        if owned {
            kingdom.realms[order.realm].gold -= receipt.crowns;
            kingdom.realms[order.realm].trade_spent_b += receipt.crowns;
            kingdom.realms[order.realm].trade_spent_a += receipt.crowns;
        } else {
            kingdom.counties[order.county].purse -= receipt.crowns;
        }
    }

    // `Merchant_Trade`'s own `if (good == 2 && qty >= 0) County_EnsurePasture(county)`,
    // between `Castle_DeliverMaterials` and the tail below.
    if order.good == Good::Cattle && order.qty >= 0 {
        let Kingdom { counties, campaign, .. } = &mut *kingdom;
        crate::field::ensure_pasture(&mut counties[order.county], &mut campaign.map);
    }
    settle(kingdom, order.county);
    Ok(receipt)
}

/// The store half of both arms, which is the same eleven-branch chain twice in
/// the original. Returns what moved — 0 for a good with no branch.
fn move_stock(kingdom: &mut Kingdom, good: Good, qty: i32, realm: usize, county: usize) -> i32 {
    match good {
        Good::Grain => kingdom.counties[county].grain += qty,
        Good::Cattle => kingdom.counties[county].herd += qty,
        Good::Iron => kingdom.realms[realm].iron += qty,
        Good::Stone => kingdom.realms[realm].stone += qty,
        Good::Timber => kingdom.realms[realm].wood += qty,
        // Sheep, wool and — on the selling arm — ale fall through with no
        // branch at all. The money still moves; nothing else does.
        Good::Sheep | Good::Wool | Good::Ale => return 0,
        other => match other.weapon_slot() {
            Some(s) => kingdom.realms[realm].weapons[s] += qty,
            None => return 0,
        },
    }
    qty
}

/// `Merchant_Trade`'s tail: the county is re-derived from its new stores.
///
/// The original runs `Ration_Apply` and `County_RefreshEstimates` **twice**,
/// with `Labour_Allocate` between them — the first pair sizes the ration off
/// the new stock, the reallocation moves people onto the work that ration
/// implies, and the second pair re-reads it. Reproduced in that order,
/// the order is the rule: a single pass leaves the estimate describing the
/// labour split from before the trade.
fn settle(kingdom: &mut Kingdom, county: usize) {
    let t = kingdom.tables;
    let armies_eat = kingdom.options.armies_eat;
    let season_next = kingdom.season_next;
    // Split off `kingdom` before the county is borrowed, because the repaint
    // needs the map and the rest of the tail needs the county.
    let Kingdom { counties, campaign, .. } = kingdom;
    settle_county(&t, &mut counties[county], &mut campaign.map, armies_eat, season_next);
}

/// `Merchant_Trade`'s tail, against one county and the map.
///
/// Split out of [`settle`] because [`crate::ai_farm::CountyStall`] reaches this
/// path with no `Kingdom` in hand: the neutral farming pass already holds
/// `&mut counties[id]` and `&mut map` when the cascade fires, so the tail has
/// to be expressible in those two. **One body, two callers** — the alternative
/// was a second copy of the seven calls in `ai_farm`, which is the two-artefacts
/// failure `docs/agents.md` warns about, maintained by the same person in the
/// same commit.
///
/// `season_next` is `g_seasonNext`, which is what `County_RefreshEstimates` is
/// passed here.
pub fn settle_county(
    t: &Tables,
    county: &mut County,
    map: &mut crate::map::CampaignMap,
    armies_eat: bool,
    season_next: u8,
) {
    // `Merchant_Trade`'s own `Herd_UpdateCrowding` — buying or selling cattle
    // moves the herd, so the animals on the county's pasture move with it.
    crate::field::herd_update_crowding(t, county, map);
    // **[`crate::ration::preview`], not [`crate::ration::apply`]** — the tail
    // recomputes the ration, it does not serve it.
    //
    // > This was `apply` on both lines, and `apply` **debits the store**. So
    // > every trade at the merchant made the county eat two extra meals on the
    // > spot, and the more you bought the more of it vanished before the season
    // > began. It went unmeasured for as long as it did because the only caller
    // > was a human clicking a stall, which no test drives against a fixture —
    // > `docs/agents.md`, *a field is only tested if something a test reads was
    // > written by something the game runs*. Wiring the neutral counties'
    // > cascade through here gave it a caller the behavioural differential
    // > watches, and it showed up the same hour as **eight sacks** on county 3
    // > of `old_turn.sav → battle-before.sav`: exactly two helpings of four.
    // >
    // > `[V]`. `Ration_Apply` (`0x0044DF5F`) reads `grain` and `herd` and
    // > writes `grainAvailable`, `herdAvailable`, `rationAchieved`,
    // > `grainEaten`, `herdEaten` and `dHapRation`. **It contains no store
    // > `-=` of any kind**, on either of its two passes over the ladder. The
    // > food is taken out of the county later in the season, and
    // > `docs/decisions.md` C149 records that our own
    // > `ration::apply` is therefore a pass the original splits in two.
    crate::ration::preview(t, county, armies_eat);
    crate::land::herd_preview(t, county, season_next);
    crate::labour::allocate(county);
    crate::ration::preview(t, county, armies_eat);
    crate::land::herd_preview(t, county, season_next);
}

