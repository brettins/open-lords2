#![allow(unused_imports)]
use super::*;
use super::tests_part::*;
use crate::county::County;
use crate::kingdom::Kingdom;
use crate::math::pct;
use crate::realm::Realm;
use crate::tables::{Tables, WEAPON_TYPE_COUNT};
use l2_net::{Quirk, Quirks};

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
pub fn trade(kingdom: &mut Kingdom, order: Order) -> Result<Receipt, Refusal> {
    if order.qty == 0 {
        return Ok(Receipt::default());
    }
    if order.county >= kingdom.counties.len() || order.realm >= kingdom.realms.len() {
        return Ok(Receipt::default());
    }
    let owned = order.realm != 0;
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

    if order.good == Good::Cattle && order.qty >= 0 {
        let Kingdom { counties, campaign, .. } = &mut *kingdom;
        crate::field::ensure_pasture(&mut counties[order.county], &mut campaign.map);
    }
    settle(kingdom, order.county);
    Ok(receipt)
}

fn move_stock(kingdom: &mut Kingdom, good: Good, qty: i32, realm: usize, county: usize) -> i32 {
    match good {
        Good::Grain => kingdom.counties[county].grain += qty,
        Good::Cattle => kingdom.counties[county].herd += qty,
        Good::Iron => kingdom.realms[realm].iron += qty,
        Good::Stone => kingdom.realms[realm].stone += qty,
        Good::Timber => kingdom.realms[realm].wood += qty,
        Good::Sheep | Good::Wool | Good::Ale => return 0,
        other => match other.weapon_slot() {
            Some(s) => kingdom.realms[realm].weapons[s] += qty,
            None => return 0,
        },
    }
    qty
}

fn settle(kingdom: &mut Kingdom, county: usize) {
    let t = kingdom.tables;
    let armies_eat = kingdom.options.armies_eat;
    let season_next = kingdom.season_next;
    let sowing = crate::ration::Sowing::from_index(kingdom.season, kingdom.options.advanced_farming);
    let Kingdom { counties, campaign, .. } = kingdom;
    settle_county(&t, &mut counties[county], &mut campaign.map, armies_eat, season_next, sowing);
}

pub fn settle_county(
    t: &Tables,
    county: &mut County,
    map: &mut crate::map::CampaignMap,
    armies_eat: bool,
    season_next: u8,
    sowing: crate::ration::Sowing,
) {
    crate::field::herd_update_crowding(t, county, map);
    // >
    // > `[V]`. `Ration_Apply` (`0x0044DF5F`) reads `grain` and `herd` and
    // > writes `grainAvailable`, `herdAvailable`, `rationAchieved`,
    // > `grainEaten`, `herdEaten` and `dHapRation`. **It contains no store
    // > `-=` of any kind**, on either of its two passes over the ladder. The
    // > food is taken out of the county later in the season, and
    // > `docs/decisions.md` C149 records that our own
    // > `ration::apply` is therefore a pass the original splits in two.
    crate::ration::preview(t, county, armies_eat, sowing);
    crate::land::herd_preview(t, county, season_next);
    crate::labour::allocate(county);
    crate::ration::preview(t, county, armies_eat, sowing);
    crate::land::herd_preview(t, county, season_next);
}

