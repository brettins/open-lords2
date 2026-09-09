//! **The merchant's transaction** — `Merchant_Trade` (`0x004284CE`), the price
//! the screen quotes, and the two limits the arrows clamp to.
//!
//! `crate::merchant` walks a merchant round its route. This is what happens
//! when it stops and you click on it. The two are deliberately separate
//! modules: one is a table of counties and a cursor, the other never learns
//! that a merchant moves at all.
//!
//! # One function moves every good
//!
//! ```c
//! Merchant_Trade(unused, qty, good, buyPrice, sellPrice, realm, county)
//! ```
//!
//! Positive `qty` buys and pays `buyPrice * qty`; negative sells and banks
//! `sellPrice * |qty|`. There is **no partial fill**: if the stock or the
//! treasury will not cover the whole order the function returns having done
//! nothing at all, and no message is raised. [`Refusal`] is ours — the original
//! is silent, and a silent refusal is a screen a player thinks is broken.
//!
//! Where each good lands is the interesting part, and it is `L2.eng` group 6's
//! id order:
//!
//! | id | good | where it goes |
//! |---:|---|---|
//! | 1 | Grain | `county.grain` |
//! | 2 | Cattle | `county.herd` |
//! | **3** | **Sheep** | **nowhere — there is no branch** |
//! | 4 | Ale | straight into [`crate::happiness::buy_ale`]; never stored |
//! | **5** | **Wool** | **nowhere — there is no branch** |
//! | 6, 7, 8 | Iron, Stone, Timber | `realm.iron` / `.stone` / `.wood` |
//! | 9 … 14 | Pikes, Bows, Maces, Crossbows, Swords, Mail | `realm.weapons[3]`, `[4]`, `[1]`, `[0]`, `[2]`, `[5]` |
//!
//! **Sheep and wool are carried and priced zero on purpose.** They have no
//! branch in either direction, so nothing can move them; `Merchant_ResetStall`
//! still copies their rows into the live stall. `docs/mechanics.md` records the
//! decision: reproduce them, let the game demonstrate its own dead end, rather
//! than decide in advance that they do not exist. [`Good::tradeable`] is what
//! says so, and it is derived from the absence of a branch rather than asserted.
//!
//! # How the price is set, and what moves it
//!
//! `FUN_00428DAF`, which runs when a good is picked, is the whole of it:
//!
//! ```c
//! sellPrice = g_merchantStall[good].price;              /* the base table */
//! markup    = Pct(sellPrice, g_units[tradingUnit].morale);
//! if (markup < 1) markup = 1;
//! if (good == 4) markup = 0;                            /* ale */
//! buyPrice  = sellPrice + markup;
//! maxQty    = buyPrice == 0 ? 0 : g_realms[player].gold / buyPrice;
//! minQty    = -(what you already hold);
//! ```
//!
//! So the **sell price is the base table price and never moves**, and the
//! **buy price is the base plus the merchant's own morale as a percentage of
//! it**, with a floor of one crown so that a cheap good is never free. That
//! floor is what makes ale's exemption necessary: at a base of 1 the markup
//! would round to 1 and double it.
//!
//! **In the shipped game the merchant's morale is 100, so the buy price is
//! exactly twice the sell price** — `Merchant_SpawnAll` writes `morale = 100`
//! into every merchant it creates and nothing in the binary ever writes a
//! merchant's morale again. That closes a disagreement `docs/kingdom.md` §11
//! left open: a published guide's merchant prices are uniformly twice the
//! table's, and the manual's *"such as 30/60 — the left number is the selling
//! price… the right number is the buying price"* says the same. Both are the
//! `morale = 100` case of this formula, and the formula is the general rule.
//!
//! The AI uses the identical markup through `Ai_BuyGood` (`0x004A4B12`) and
//! `Ai_SellGood` (`0x004A4A3F`), reading the merchant off the county's own
//! stall slot rather than off a click. So [`quote`] is the price for everybody.
//!
//! # The stall never changes
//!
//! `Merchant_ResetStall` (`0x0042847C`) copies `g_goodsPrice` into the live
//! stall at `0x005532A0` and zeroes each good's second word. It is called
//! **exactly once in the whole binary**, from the new-game path, and nothing
//! else writes either word. The second word — a per-good counter that would be
//! the obvious place for a stock — stays 0 for the life of the game.
//!
//! **So a merchant's stock is infinite.** The only limit on buying is your
//! treasury and the only limit on selling is what you hold. There is a
//! fifteen-entry table at `0x004D8950` that reads exactly like a stock list —
//! 1000 grain, 100 cattle, 200 sheep, 100 ale, 500 wool, 100 each of iron and
//! stone, 200 timber, 500 of every weapon — and **no instruction in the binary
//! reads it**. `docs/kingdom.md` §12 asks what it is; it is unreferenced data,
//! and the mechanic it would have served is not in the shipped game.
//!
//! # What this module does not reproduce, and where to find it
//!
//! `Merchant_Trade`'s tail runs seven more calls on the county. Five are here
//! ([`Order::apply`]'s tail). Two are not, and both are named rather than
//! quietly dropped:
//!
//! * `FUN_0046921D` — **buying cattle into a county with no pasture converts a
//!   field into one.** It recounts the fields, and if `fieldsCattle == 0` it
//!   turns one fallow field (`FUN_0046958F`) or, failing that, one grain field
//!   (`FUN_0046965A`) into pasture, walking a per-county round-robin cursor at
//!   `+0x15A` that this crate does not model. The grain case also docks a share
//!   of the standing crop. It needs the tile map, a cursor field and a save
//!   entry; it is a rule of its own and it is left whole for whoever takes it.
//! * `FUN_00450CCD` — a degraded castle draws its owed wood and stone out of
//!   the realm's stockpile, which is why it is called here: it is what makes
//!   stone bought at the merchant reach the castle. `Readme.txt` says the
//!   opposite (*"don't expect to see it deducted right away"*), and settling
//!   which is right needs `castleDegraded` set in a live game.

use crate::county::County;
use crate::kingdom::Kingdom;
use crate::math::pct;
use crate::realm::Realm;
use crate::tables::{Tables, WEAPON_TYPE_COUNT};
use l2_net::{Quirk, Quirks};

/// `L2.eng` group 6 has fifteen strings and `g_goodsPrice` fifteen entries;
/// index 0 is a placeholder in both, so a good id runs 1 … 14.
pub const GOOD_COUNT: usize = 15;

/// One good, by its `L2.eng` group 6 id. The discriminants **are** the ids the
/// original passes to `Merchant_Trade`, so `good as i32` is the wire value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Good {
    Grain = 1,
    Cattle = 2,
    Sheep = 3,
    Ale = 4,
    Wool = 5,
    Iron = 6,
    Stone = 7,
    Timber = 8,
    Pikes = 9,
    Bows = 10,
    Maces = 11,
    Crossbows = 12,
    Swords = 13,
    Mail = 14,
}

/// Every good, in id order — which is `L2.eng` group 6's order, the merchant
/// screen's order, and the order `g_goodsPrice` is indexed in.
pub const ALL_GOODS: [Good; 14] = [
    Good::Grain,
    Good::Cattle,
    Good::Sheep,
    Good::Ale,
    Good::Wool,
    Good::Iron,
    Good::Stone,
    Good::Timber,
    Good::Pikes,
    Good::Bows,
    Good::Maces,
    Good::Crossbows,
    Good::Swords,
    Good::Mail,
];

impl Good {
    pub fn id(self) -> usize {
        self as usize
    }

    pub fn from_id(id: usize) -> Option<Good> {
        ALL_GOODS.iter().copied().find(|g| g.id() == id)
    }

    /// A fallback name. **`L2.eng` group 6 is the real one** and the view layer
    /// reads it; this is what a headless test prints and what appears when the
    /// game's own text is not installed.
    pub fn name(self) -> &'static str {
        match self {
            Good::Grain => "Grain",
            Good::Cattle => "Cattle",
            Good::Sheep => "Sheep",
            Good::Ale => "Ale",
            Good::Wool => "Wool",
            Good::Iron => "Iron",
            Good::Stone => "Stone",
            Good::Timber => "Timber",
            Good::Pikes => "Pikes",
            Good::Bows => "Bows",
            Good::Maces => "Maces",
            Good::Crossbows => "Crossbows",
            Good::Swords => "Swords",
            Good::Mail => "Mail",
        }
    }

    /// Which of the realm's six weapon counters this good is, if it is one.
    ///
    /// **The mapping is not the id order**, and it agrees with a derivation
    /// that shares no step with it: reading `weapons[]` through group 6 gives
    /// crossbow, mace, sword, pike, bow, mail, which is exactly what
    /// `docs/armies.md` §6 derived from `g_weaponCost` from the other end.
    pub fn weapon_slot(self) -> Option<usize> {
        match self {
            Good::Pikes => Some(3),
            Good::Bows => Some(4),
            Good::Maces => Some(1),
            Good::Crossbows => Some(0),
            Good::Swords => Some(2),
            Good::Mail => Some(5),
            _ => None,
        }
        .filter(|&s| s < WEAPON_TYPE_COUNT)
    }

    /// Whether `Merchant_Trade` has a branch that moves this good.
    ///
    /// **False for sheep and wool only.** A price of zero would still let a
    /// good be sold for nothing if a branch existed; none does, in either
    /// direction, so nothing can move them at all. Ale is `true` — it moves,
    /// it just does not end up anywhere you can point at.
    pub fn tradeable(self) -> bool {
        !matches!(self, Good::Sheep | Good::Wool)
    }

    /// Whether the good is stored by the **county** rather than the realm.
    /// Grain and cattle; everything else that is stored at all is realm-wide.
    pub fn is_county_store(self) -> bool {
        matches!(self, Good::Grain | Good::Cattle)
    }
}

/// The two numbers the merchant screen prints side by side — the manual's
/// *"30/60"*.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quote {
    /// What the merchant pays you. The base table price, unmarked up.
    pub sell: i32,
    /// What he charges you: [`Quote::sell`] plus the markup.
    pub buy: i32,
}

impl Quote {
    /// The markup itself, which is the only part of a quote that moves.
    pub fn markup(self) -> i32 {
        self.buy - self.sell
    }
}

/// `FUN_00428DAF`'s price arithmetic.
///
/// `merchant_morale` is `g_units[DAT_00553C64].morale` — the morale of the
/// merchant *being clicked*, which is 100 for every merchant the shipped game
/// creates. The floor of one crown is applied **before** ale's exemption, which
/// is the order that makes ale free of markup rather than one crown dearer.
pub fn quote(t: &Tables, good: Good, merchant_morale: i32) -> Quote {
    let sell = t.good[good.id()].sell_price;
    let mut markup = pct(sell, merchant_morale);
    if markup < 1 {
        markup = 1;
    }
    if good == Good::Ale {
        markup = 0;
    }
    Quote { sell, buy: sell + markup }
}

/// The arrows' upper clamp: `gold / buyPrice`, and **0 rather than a division
/// by zero** when the good is free.
///
/// Ale is the case that reaches the guard in the shipped game only if a mod
/// prices it 0; sheep and wool reach it always, and get an upper limit of 0 to
/// go with their lower limit of 0.
pub fn max_buy(gold: i32, buy_price: i32) -> i32 {
    if buy_price == 0 {
        0
    } else {
        gold / buy_price
    }
}

/// The arrows' lower clamp, as a **negative** number — the original stores
/// `minQty = -stock` and the panel prints `-minQty`.
///
/// What you can sell is what the store the good lives in holds. **Ale is 0**,
/// explicitly, so ale cannot be sold; sheep and wool fall through every branch
/// and are 0 for the same reason they cannot be bought.
pub fn min_qty(counties: &[County], realms: &[Realm], good: Good, realm: usize, county: usize) -> i32 {
    -stock(counties, realms, good, realm, county)
}

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

/// Why a trade did nothing. **Ours** — the original returns silently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// A sale of more than the store holds. `Merchant_Trade`'s ten stock
    /// guards, one per good that has one.
    NotEnoughStock,
    /// `g_realms[realm].gold < buyPrice * qty`. `L2.eng` 68/20 says it in the
    /// game's own words: *"You do not have enough crowns to buy, my Lord."*
    NotEnoughGold,
}

/// What a trade did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Receipt {
    /// Crowns that changed hands — positive whichever way the goods went.
    pub crowns: i32,
    /// Units moved, positive when bought and negative when sold. **Zero for
    /// sheep and wool even on a successful trade**, because no branch moves
    /// them, and zero for ale, which is drunk rather than stored.
    pub moved: i32,
    /// Happiness [`crate::happiness::buy_ale`] gave the county, 0 for
    /// everything else.
    pub ale_happiness: i32,
}

/// One order, in the argument order `Merchant_Trade` takes them.
///
/// `buy_price` and `sell_price` are carried rather than recomputed because the
/// original carries them: the screen quotes a price, the player agrees to it,
/// and the confirmed trade runs on the quoted number. In multiplayer they go
/// on the wire (`Net_WriteField(&DAT_0055CD70, 4)`), which is the same
/// statement — the price is part of the command, not something the far end
/// works out again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Order {
    /// Positive buys, negative sells, 0 does nothing at all.
    pub qty: i32,
    pub good: Good,
    pub buy_price: i32,
    pub sell_price: i32,
    /// The paying realm, **or 0 for an unowned county trading on its own
    /// account** out of [`County::purse`].
    pub realm: usize,
    pub county: usize,
}

impl Order {
    /// A buy at the quoted prices.
    pub fn buy(good: Good, qty: i32, q: Quote, realm: usize, county: usize) -> Order {
        Order { qty, good, buy_price: q.buy, sell_price: q.sell, realm, county }
    }

    /// A sale — `qty` is given positive and stored negative, because every
    /// caller has a positive quantity in hand and the sign is the protocol.
    pub fn sell(good: Good, qty: i32, q: Quote, realm: usize, county: usize) -> Order {
        Order { qty: -qty, good, buy_price: q.buy, sell_price: q.sell, realm, county }
    }
}

/// `Merchant_Trade` (`0x004284CE`).
///
/// `Ok(Receipt::default())` for a zero quantity: the original's whole body is
/// inside `if (qty != 0)`, so a zero order does not even run the tail that
/// recomputes the county.
///
/// # Two guards that only fire for an owned county
///
/// Both the stock check and the gold check are inside `if (realm != 0)`. An
/// **unowned** county trading on its own account is checked for neither: it can
/// sell grain it does not have and buy with a purse it has emptied, and the
/// stores and the purse both go negative. Reproduced, because `Ai_BuyGood`
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
    // things such a county actually has - its own stores and its own purse.
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

    settle(kingdom, order.county);
    Ok(receipt)
}

/// The store half of both arms, which is the same eleven-branch chain twice in
/// the original. Returns what actually moved — 0 for a good with no branch.
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
/// implies, and the second pair re-reads it. Reproduced in that order, because
/// the order is the rule: a single pass leaves the estimate describing the
/// labour split from before the trade.
fn settle(kingdom: &mut Kingdom, county: usize) {
    let t = kingdom.tables;
    let armies_eat = kingdom.options.armies_eat;
    let season_next = kingdom.season_next;
    let c = &mut kingdom.counties[county];
    c.herd_crowding = crate::land::herd_crowding(&t, c.herd, c.fields_cattle);
    crate::ration::apply(&t, c, armies_eat);
    crate::land::herd_preview(&t, c, season_next);
    crate::labour::allocate(c);
    crate::ration::apply(&t, c, armies_eat);
    crate::land::herd_preview(&t, c, season_next);
}

/// The `±1` / `±10` the trade panel's arrows step by.
///
/// `FUN_00435339` and `FUN_0043543D` both read the widget layer's auto-repeat
/// counter (`0x00591554`) and step by 10 once it has reached 44. The counter is
/// 0 on the click that begins a press and climbs while the button is held, so
/// **a click is one and a long hold is ten** — the arrows accelerate rather
/// than offering two buttons.
pub const REPEAT_ACCELERATES_AT: u32 = 0x2C;

pub fn arrow_step(repeat: u32) -> i32 {
    if repeat < REPEAT_ACCELERATES_AT {
        1
    } else {
        10
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Faithful. The switched-off answers live in `tests/quirks.rs`.
    #[allow(dead_code)]
    const Q: Quirks = Quirks::FAITHFUL;
    use crate::tables::Tables;

    const T: &Tables = &Tables::DEFAULT;

    fn kingdom() -> Kingdom {
        let mut k = Kingdom::new(1);
        k.county_count = 2;
        k.counties[1].owner = 1;
        k.counties[1].population = 1000;
        k.counties[1].grain = 100;
        k.counties[1].herd = 50;
        k.realms[1].gold = 1000;
        k
    }

    /// The headline: at the merchant's shipped morale of 100 the buy price is
    /// exactly twice the sell price, for every good but ale — which is the
    /// manual's *"30/60"* and the published guides' doubled table, both.
    #[test]
    fn a_merchant_at_full_morale_charges_double_what_he_pays() {
        for good in ALL_GOODS {
            let q = quote(T, good, 100);
            assert_eq!(q.sell, T.good[good.id()].sell_price, "{good:?}");
            if good == Good::Ale {
                assert_eq!(q.markup(), 0, "ale is exempt from the markup");
                assert_eq!(q.buy, q.sell);
            } else if q.sell == 0 {
                // Sheep and wool: the floor of one crown is all there is.
                assert_eq!(q.buy, 1, "{good:?}");
            } else {
                assert_eq!(q.buy, q.sell * 2, "{good:?}");
            }
        }
    }

    /// The floor bites before the exemption does, which is why ale needs the
    /// exemption at all: a base of 1 at any morale below 100 would still be
    /// marked up by the minimum crown.
    #[test]
    fn the_markup_never_rounds_away_to_nothing() {
        for morale in 0..=100 {
            let q = quote(T, Good::Grain, morale);
            assert!(q.markup() >= 1, "morale {morale} bought grain at cost");
            assert_eq!(quote(T, Good::Ale, morale).markup(), 0, "ale at morale {morale}");
        }
    }

    #[test]
    fn buying_grain_moves_sacks_and_crowns_and_nothing_else() {
        let mut k = kingdom();
        let q = quote(T, Good::Grain, 100);
        let before = k.realms[1].gold;
        let r = trade(&mut k, Order::buy(Good::Grain, 10, q, 1, 1)).unwrap();
        assert_eq!(r.moved, 10);
        assert_eq!(r.crowns, 10 * q.buy);
        assert_eq!(k.counties[1].grain, 110);
        assert_eq!(k.realms[1].gold, before - 10 * q.buy);
    }

    /// The refusal is all-or-nothing: an order a crown too dear moves neither
    /// goods nor money.
    #[test]
    fn an_unaffordable_order_is_refused_whole() {
        let mut k = kingdom();
        let q = quote(T, Good::Grain, 100);
        k.realms[1].gold = 10 * q.buy - 1;
        let before = (k.realms[1].gold, k.counties[1].grain);
        assert_eq!(trade(&mut k, Order::buy(Good::Grain, 10, q, 1, 1)), Err(Refusal::NotEnoughGold));
        assert_eq!((k.realms[1].gold, k.counties[1].grain), before);
        // One fewer fits exactly.
        assert!(trade(&mut k, Order::buy(Good::Grain, 9, q, 1, 1)).is_ok());
    }

    #[test]
    fn selling_more_than_the_county_holds_is_refused_whole() {
        let mut k = kingdom();
        let q = quote(T, Good::Grain, 100);
        let before = (k.realms[1].gold, k.counties[1].grain);
        assert_eq!(
            trade(&mut k, Order::sell(Good::Grain, 101, q, 1, 1)),
            Err(Refusal::NotEnoughStock)
        );
        assert_eq!((k.realms[1].gold, k.counties[1].grain), before);
        let r = trade(&mut k, Order::sell(Good::Grain, 100, q, 1, 1)).unwrap();
        assert_eq!(r.crowns, 100 * q.sell);
        assert_eq!(k.counties[1].grain, 0);
    }

    /// The weapons go to the realm and to the slot the group 6 mapping names,
    /// not to the slot their id order suggests.
    #[test]
    fn weapons_land_in_the_armoury_slots_group_six_names() {
        let mut k = kingdom();
        k.realms[1].gold = 100_000;
        for good in [Good::Pikes, Good::Bows, Good::Maces, Good::Crossbows, Good::Swords, Good::Mail]
        {
            let q = quote(T, good, 100);
            trade(&mut k, Order::buy(good, 3, q, 1, 1)).unwrap();
            let slot = good.weapon_slot().unwrap();
            assert_eq!(k.realms[1].weapons[slot], 3, "{good:?} -> weapons[{slot}]");
        }
        assert_eq!(k.realms[1].weapons, [3; WEAPON_TYPE_COUNT]);
    }

    /// Sheep and wool are carried, priced and quotable, and nothing moves them.
    /// This is the dead end being demonstrated rather than asserted away.
    #[test]
    fn sheep_and_wool_can_be_named_and_cannot_be_moved() {
        let mut k = kingdom();
        for good in [Good::Sheep, Good::Wool] {
            assert!(!good.tradeable(), "{good:?}");
            assert_eq!(T.good[good.id()].sell_price, 0, "{good:?} is priced zero");
            let q = quote(T, good, 100);
            let gold = k.realms[1].gold;
            let r = trade(&mut k, Order::buy(good, 5, q, 1, 1)).unwrap();
            assert_eq!(r.moved, 0, "{good:?} went somewhere");
            // The crowns still move: the buy arm has no branch for the good but
            // it always pays. At a base price of 0 the floor of one crown is
            // the whole bill.
            assert_eq!(r.crowns, 5);
            assert_eq!(k.realms[1].gold, gold - 5);
        }
    }

    /// Ale is bought and drunk in the same instruction: no store, happiness now.
    #[test]
    fn ale_becomes_happiness_and_is_never_stored() {
        let mut k = kingdom();
        k.counties[1].happiness = 50;
        let q = quote(T, Good::Ale, 100);
        assert_eq!(q.buy, 1, "a barrel is a crown");
        // population 1000, so a tenth is 100 crowns a point.
        let r = trade(&mut k, Order::buy(Good::Ale, 300, q, 1, 1)).unwrap();
        assert_eq!(r.ale_happiness, 3);
        assert_eq!(r.moved, 0);
        assert_eq!(k.counties[1].happiness, 53);
        assert_eq!(k.realms[1].gold, 1000 - 300);
    }

    /// Ale cannot be sold, and the limit is what says so rather than a branch.
    #[test]
    fn ale_has_no_sale_limit_at_all() {
        let k = kingdom();
        assert_eq!(min_qty(&k.counties, &k.realms, Good::Ale, 1, 1), 0);
        assert_eq!(min_qty(&k.counties, &k.realms, Good::Grain, 1, 1), -100);
        assert_eq!(min_qty(&k.counties, &k.realms, Good::Cattle, 1, 1), -50);
    }

    /// The arrows' two clamps, which are the whole of what the panel can ask
    /// for: gold over the buy price above, and the store below.
    #[test]
    fn the_arrows_clamp_to_the_treasury_above_and_the_store_below() {
        let k = kingdom();
        let q = quote(T, Good::Grain, 100);
        assert_eq!(q.buy, 4);
        assert_eq!(max_buy(k.realms[1].gold, q.buy), 250);
        assert_eq!(max_buy(0, q.buy), 0);
        assert_eq!(max_buy(1000, 0), 0, "a free good does not divide by zero");
    }

    /// An unowned county pays out of its own purse, and neither guard fires.
    #[test]
    fn an_unowned_county_trades_out_of_its_purse_and_is_never_refused() {
        let mut k = kingdom();
        k.counties[1].owner = 0;
        k.counties[1].purse = 5;
        // Nobody left to eat, so the tail's `Ration_Apply` cannot reach into
        // the store and hide what the missing guard did to it.
        k.counties[1].population = 0;
        let q = quote(T, Good::Grain, 100);
        // Far more than the purse holds, and it goes through.
        let r = trade(&mut k, Order::buy(Good::Grain, 100, q, 0, 1)).unwrap();
        assert_eq!(r.moved, 100);
        assert_eq!(k.counties[1].purse, 5 - 100 * q.buy, "the purse goes negative");
        // And it can sell grain it does not have: the sale is not refused, and
        // the crowns are banked for sacks that were never there.
        k.counties[1].grain = 0;
        let purse = k.counties[1].purse;
        let r = trade(&mut k, Order::sell(Good::Grain, 10, q, 0, 1)).unwrap();
        assert_eq!(r.moved, -10);
        assert_eq!(k.counties[1].purse, purse + 10 * q.sell);
        // The store went to -10 and the tail's `Ration_Apply` then pulled it
        // back to 0 — `sacks = min(wanted, grain)` is negative against a
        // negative store, so subtracting it *adds*. The missing guard is
        // therefore worth free crowns rather than a visible negative number,
        // which is why nothing has ever noticed it.
        assert_eq!(k.counties[1].grain, 0);
    }

    /// The four realm accumulators, which are the state a trade exists to
    /// produce for `docs/hypotheses.json`. Both of each pair take the same
    /// number, and nothing here resets either.
    #[test]
    fn a_trade_writes_both_accumulators_of_the_pair_it_belongs_to() {
        let mut k = kingdom();
        let q = quote(T, Good::Grain, 100);
        trade(&mut k, Order::buy(Good::Grain, 10, q, 1, 1)).unwrap();
        let r = &k.realms[1];
        assert_eq!((r.trade_spent_a, r.trade_spent_b), (10 * q.buy, 10 * q.buy));
        assert_eq!((r.trade_received_a, r.trade_received_b), (0, 0));

        trade(&mut k, Order::sell(Good::Grain, 4, q, 1, 1)).unwrap();
        let r = &k.realms[1];
        assert_eq!((r.trade_received_a, r.trade_received_b), (4 * q.sell, 4 * q.sell));
        assert_eq!((r.trade_spent_a, r.trade_spent_b), (10 * q.buy, 10 * q.buy));
    }

    #[test]
    fn a_zero_quantity_does_nothing_at_all() {
        let mut k = kingdom();
        let before = k.clone();
        let q = quote(T, Good::Grain, 100);
        assert_eq!(trade(&mut k, Order::buy(Good::Grain, 0, q, 1, 1)), Ok(Receipt::default()));
        assert_eq!(k.counties[1], before.counties[1]);
        assert_eq!(k.realms[1], before.realms[1]);
    }

    #[test]
    fn the_arrows_step_by_one_until_the_repeat_counter_reaches_forty_four() {
        assert_eq!(arrow_step(0), 1);
        assert_eq!(arrow_step(0x2B), 1);
        assert_eq!(arrow_step(0x2C), 10);
        assert_eq!(arrow_step(0x2F), 10);
    }

    /// Every good id round-trips, and the ids are group 6's.
    #[test]
    fn the_good_ids_are_one_to_fourteen_with_no_gap() {
        for (i, good) in ALL_GOODS.iter().enumerate() {
            assert_eq!(good.id(), i + 1);
            assert_eq!(Good::from_id(good.id()), Some(*good));
        }
        assert_eq!(Good::from_id(0), None);
        assert_eq!(Good::from_id(15), None);
        assert_eq!(ALL_GOODS.len() + 1, GOOD_COUNT);
    }
}
