use super::*;

/// A good the AI's farming pass can buy, by `Merchant_Trade`'s good id.
///
/// The ids are `L2.eng` group 6
/// live in the county. See `docs/symbols.md` on `0x004284CE` for the full list
/// — 3 *Sheep* and 5 *Wool* have no branch in either direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Good {
    /// Good 1 — sacks, into [`County::grain`].
    Grain = 1,
    /// Good 2 — head, into [`County::herd`].
    Cattle = 2,
}

impl Good {
    pub fn id(self) -> u8 {
        self as u8
    }
}

/// The merchant seam — `FUN_004A4B12(county, qty, good)`.
///
/// The original checks the county has a stall (`+0x1A4`), prices the good from
/// the live stall marked up by the merchant's morale, and refuses the trade
/// unless the buyer's purse covers it: the realm's treasury for an owned
/// county, and county `+0x1F4` for an unowned one trading with itself.
///
/// An implementation must **move both sides** — credit [`County::grain`] or
/// [`County::herd`] and debit whichever purse — because the caller re-reads the
/// county's stock immediately afterwards to decide whether to try the next lot
/// down.
pub trait Market {
    /// Buy `qty` of `good` for this county. Returns whether the goods moved.
    ///
    /// `id` is the county index, which is what `Ai_BuyGood` is passed and what
    /// it reads the stall out of; the record comes with it because the caller
    /// already holds the borrow
    /// lines.
    ///
    /// The map is here because `Merchant_Trade`'s tail is — `Herd_UpdateCrowding`
    /// repaints the county's pasture from the new herd,
    /// store has to be able to move the ground with it. See
    /// [`crate::trade::settle_county`].
    fn buy(
        &mut self,
        id: usize,
        county: &mut County,
        map: &mut CampaignMap,
        qty: i32,
        good: Good,
    ) -> bool;

    /// `Ai_TradeForCounty` (`0x0049E39B`) — the surplus sale
    /// purchase the three **realm** styles run before they shop for food, at
    /// this county's stall and on the owning realm's account.
    ///
    /// Default: nothing, which is [`NoMarket`] and a caller with no merchant
    /// model. [`FarmStyle::sells_first`] is what decides whether [`lay_out`]
    /// calls it at all.
    fn trade_for_county(&mut self, _id: usize, _county: &mut County, _map: &mut CampaignMap) {}
}

/// A market that refuses every trade.
///
///
/// keeping at the type. Its comment used to read *"there is
/// no stall yet, so every style's opening shopping cascade is refused
/// county farms what it already has"*, and that stated reason was checked and
/// was **false**: `Ai_BuyGood`'s stall gate is county `+0x1A4`, which the six
/// one-turn-apart fixtures carry non-zero on every county holding a merchant,
///
/// was there, the money was there, and refusing the cascade cost the unowned
/// counties **50 sacks of grain a season each**. `docs/decisions.md`
/// C149; [`CountyStall`] is what phase 1 passes now.
///
/// It stays as the thing a caller with no merchant model at all passes — a
/// hand-built `Kingdom` in a unit test, mostly — and because a style still lays
/// out its fields, sets its rations and splits its labour against it.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoMarket;

impl Market for NoMarket {
    fn buy(
        &mut self,
        _id: usize,
        _county: &mut County,
        _map: &mut CampaignMap,
        _qty: i32,
        _good: Good,
    ) -> bool {
        false
    }
}

/// **`Ai_BuyGood` (`0x004A4B12`)** — the county's own merchant stall, which is
/// what an AI lord and an unowned county buy through.
///
/// ```c
/// void Ai_BuyGood(county, qty, good) {
///     if (g_counties[county].merchantCount != '\0') {
///         realm = g_counties[county].owner;
///         base  = g_merchantStall[good].price;
///         markup = Pct(base, g_units[g_counties[county].merchantUnit].morale);
///         if (markup < 1) markup = 1;
///         if (good == 4) markup = 0;                       /* ale */
///         price = base + markup;
///         if (((realm != 0) || (price * qty <= g_counties[county].purse)) &&
///             ((realm == 0) || (price * qty <= g_realms[realm].gold)))
///             Merchant_Trade(0, qty, good, price, base, realm, county);
///     }
/// }
/// ```
///
/// Three gates, in that order, and **each one of them is load-bearing on the
/// real fixtures**:
///
/// 1. **the stall.** No merchant standing in the county, no purchase — not a
///    cheaper one, none at all.
/// 2. **the price.** `g_goodsPrice[1]` is 2 for grain and every merchant the
///    shipped game creates has morale 100, so `Pct(2, 100) = 2` and a sack
///    costs **4 crowns**. That number is the whole of why the cascade lands
///    where it does.
/// 3. **the purse.** `price * qty <= purse`, whole lot or nothing —
///    partial fill anywhere in this path.
///
/// Put together with [`FarmStyle::buys`], the cascade is *"take the largest lot
/// whose bill the purse covers"* expressed as four `if`s. The 400, 200 and 100
/// sack lots cost 1600, 800 and 400 crowns, which an unowned county has never
/// had; the 50-sack lot costs **200**, and that is the one that moves. So the
/// fifty sacks an unowned county gains in a season are not a constant anybody
/// adds — they are `min(lot : 4 * lot <= purse)` over `{400, 200, 100, 50}`,
///
/// 186 and 195. `docs/decisions.md` C149.
///
/// `morale` is looked up per county because
/// the price is the *county's* merchant's morale: two counties in the same
/// season can quote different prices, and nothing about the shipped game's
/// uniform morale of 100 is a rule this should bake in.
pub struct CountyStall<'a> {
    /// The tables the stall's base prices come from.
    t: &'a Tables,
    /// Whether each county has a stall, and at what morale — indexed by county
/// id. A fixed-size array, because the lookup happens
    /// inside the simulation (`docs/netcode.md` D-4).
    stall: [Option<i32>; crate::county::MAX_COUNTIES],
    /// **The realms, mutably** — the treasury an owned county's bill is tested
    /// against and paid out of,
    /// `Merchant_Trade` books it in. The farming pass itself is handed a
    /// *copy* of the realm array for its estimates; see
    /// [`crate::Kingdom::run_ai_farms_at_the_stall`] for why the copy is exact.
    realms: &'a mut [Realm],
    /// `g_seasonNext`, for `County_RefreshEstimates` in the trade's tail.
    season_next: Season,
    /// `g_optArmiesEat`, for the `Ration_Apply` in the same tail.
    armies_eat: bool,
/// How many lots moved, for the caller's report.
    pub bought: i32,
}

impl<'a> CountyStall<'a> {
    /// Build the stall from the counties' own `+0x1A4` / `+0x1A5`
    /// array — exactly the two reads `Ai_BuyGood` makes.
    ///
    /// A county whose `merchant_unit` names a slot that is empty gets **no
/// stall at all**. That is a deliberate
    /// departure and it is the safe direction: the original would index
    /// `g_units` unchecked and mark up by whatever it found, and a morale of
    /// zero would still leave the one-crown floor and let the trade happen at
    /// 3 crowns a sack. Refusing is the smaller invention, and
    /// [`crate::merchant::recount_all`] is the only writer of the pair, so the
    /// two cannot disagree in a game this crate has driven.
    pub fn new(
        t: &'a Tables,
        counties: &[County; crate::county::MAX_COUNTIES],
        units: &crate::unit::Units,
        realms: &'a mut [Realm],
        season_next: Season,
        armies_eat: bool,
    ) -> CountyStall<'a> {
        let mut stall = [None; crate::county::MAX_COUNTIES];
        for (id, slot) in stall.iter_mut().enumerate() {
            let county = &counties[id];
            if county.merchant_count == 0 {
                continue;
            }
            *slot = units.get(county.merchant_unit as usize).map(|u| u.morale);
        }
        CountyStall { t, stall, realms, season_next, armies_eat, bought: 0 }
    }

    /// `Ai_BuyGood`'s price: the stall's base plus the merchant's morale as a
    /// percentage of it, floored at one crown. Ale's exemption is carried
    /// because the function has it, even though no farming style buys ale.
    ///
    /// This is [`crate::trade::quote`] with the merchant read off the county's
/// stall, which is the sentence `docs/symbols.md` 
    /// uses for it: *"Ai_BuyGood and Ai_SellGood apply the identical markup off
    /// the county's own stall slot, so this is the price for everybody."*
    pub fn price(&self, good: Good, morale: i32) -> i32 {
        let g = match good {
            Good::Grain => crate::trade::Good::Grain,
            Good::Cattle => crate::trade::Good::Cattle,
        };
        crate::trade::quote(self.t, g, morale).buy
    }

    /// `Ai_SellGood` (`0x004A4A3F`) — `Merchant_Trade(1, -qty, …)`, the stall's
    /// **base** price, no purse test of its own.
    ///
    /// ```c
    /// Merchant_Trade(1,-qty,good,markup + base,base,owner,county);
    /// ```
    ///
    /// Only the three realm stores reach this, so the stock
    /// both the realm's. `Merchant_Trade`'s own `stock < want` guard is
    /// reproduced even though the one caller subtracts the reserve from the
    /// stock and so cannot trip it.
    fn sell_good(
        &mut self,
        county: &mut County,
        map: &mut CampaignMap,
        morale: i32,
        qty: i32,
        good: crate::trade::Good,
    ) {
        if qty <= 0 {
            return;
        }
        let owner = county.owner as usize;
        let q = crate::trade::quote(self.t, good, morale);
        let Some(realm) = self.realms.get_mut(owner) else { return };
        let store = match good {
            crate::trade::Good::Timber => &mut realm.wood,
            crate::trade::Good::Iron => &mut realm.iron,
            crate::trade::Good::Stone => &mut realm.stone,
            _ => return,
        };
        if *store < qty {
            return;
        }
        *store -= qty;
        let crowns = q.sell * qty;
        realm.gold += crowns;
        realm.trade_received_b += crowns;
        realm.trade_received_a += crowns;
        crate::trade::settle_county(
            self.t,
            county,
            map,
            self.armies_eat,
            self.season_next.index(),
        );
    }

    /// `Ai_BuyGoodDownTo` (`0x004A4C41`) — `Ai_BuyGood` that **halves the lot**
    /// until the treasury covers it
    ///
    /// ```c
    /// do { if ((markup + base) * qty <= gold) { Merchant_Trade(0,qty,…); return; }
    ///      qty = qty / 2; } while (0 < qty);
    /// ```
    ///
    /// The gold is read **once**, before the loop,
    /// realm — the three resources
    /// caller passes.
    fn buy_good_down_to(
        &mut self,
        county: &mut County,
        map: &mut CampaignMap,
        morale: i32,
        qty: i32,
        good: crate::trade::Good,
    ) {
        let owner = county.owner as usize;
        let q = crate::trade::quote(self.t, good, morale);
        let Some(realm) = self.realms.get_mut(owner) else { return };
        let gold = realm.gold;
        let mut qty = qty;
        while qty > 0 {
            if q.buy * qty <= gold {
                let crowns = q.buy * qty;
                match good {
                    crate::trade::Good::Timber => realm.wood += qty,
                    crate::trade::Good::Iron => realm.iron += qty,
                    crate::trade::Good::Stone => realm.stone += qty,
                    other => match other.weapon_slot() {
                        Some(s) => realm.weapons[s] += qty,
                        None => return,
                    },
                }
                realm.gold -= crowns;
                realm.trade_spent_b += crowns;
                realm.trade_spent_a += crowns;
                crate::trade::settle_county(
                    self.t,
                    county,
                    map,
                    self.armies_eat,
                    self.season_next.index(),
                );
                return;
            }
            qty /= 2;
        }
    }
}

/// County `+0x1FD` weapon type 0..5 → the `Merchant_Trade` good id
/// `Ai_TradeForCounty` (`0x0049E39B`) buys, which is **not** the order of
/// either list: `0 → 12, 1 → 11, 2 → 13, 3 → 9, 4 → 10, else → 14`.
fn weapon_good(weapon_type: usize) -> crate::trade::Good {
    match weapon_type {
        0 => crate::trade::Good::Crossbows,
        1 => crate::trade::Good::Maces,
        2 => crate::trade::Good::Swords,
        3 => crate::trade::Good::Pikes,
        4 => crate::trade::Good::Bows,
        _ => crate::trade::Good::Mail,
    }
}

impl Market for CountyStall<'_> {
    fn buy(
        &mut self,
        id: usize,
        county: &mut County,
        map: &mut CampaignMap,
        qty: i32,
        good: Good,
    ) -> bool {
        // Gate 1 — the stall.
        let Some(morale) = self.stall.get(id).copied().flatten() else {
            return false;
        };
        let price = self.price(good, morale);
        let bill = price * qty;
        // Gate 3 — the purse, and **which purse is the county's owner**:
        //
        // ```c
        // if (((realm != 0) || (price * qty <= g_counties[county].purse)) &&
        //     ((realm == 0) || (price * qty <= g_realms[realm].gold)))
        // ```
        //
        // For an unowned county `Merchant_Trade`'s own gold guard is inside
        // `if (realm != 0)` and so never runs; this test in `Ai_BuyGood` is the
// only thing in front of it, so `crate::trade`'s
        // `UnownedCountyTradesUnchecked` quirk is about a *reachable* path
        //
        //
        // > The owned arm used to be refused outright here,
        // > `manage_county_farms` still passed `NoMarket` and so the arm had no
        // > caller. That made the note true
        // > counties farmed without ever shopping. Both callers pass the stall
        // > now — AI step 5 and `Ai_ManageFarmsAll` at the head of the season.
        let owner = county.owner as usize;
        let affordable = if owner == 0 {
            bill <= county.purse
        } else {
            self.realms.get(owner).is_some_and(|r| bill <= r.gold)
        };
        if !affordable {
            return false;
        }
        // `Merchant_Trade(0, qty, good, price, base, realm, county)`'s buy limb
        // (`0x004284CE`). Its own `gold < bill` refusal is the test just made,
        // so it cannot fire; then the store, then the money, in this order.
        match good {
            Good::Grain => county.grain += qty,
            Good::Cattle => county.herd += qty,
        }
        if owner == 0 {
            county.purse -= bill;
        } else {
            // `gold -= bill; tradeSpentB += bill; tradeSpentA += bill;` — the
            // same three writes, in the same order, as `crate::trade::trade`'s
            // owned buy limb, which is the player's door into the same function.
            let realm = &mut self.realms[owner];
            realm.gold -= bill;
            realm.trade_spent_b += bill;
            realm.trade_spent_a += bill;
        }
        // `Merchant_Trade`'s tail also runs `Realm_RecountWeapons` (a derived
        // total here, `Realm::weapons_total`), `Castle_DeliverMaterials` and —
        // for cattle — `County_EnsurePasture`. The last two are not in
        // `settle_county` and are not in `crate::trade::trade` either; both are
        // named in `crate::trade`'s module documentation as not reproduced.
        crate::trade::settle_county(
            self.t,
            county,
            map,
            self.armies_eat,
            self.season_next.index(),
        );
        self.bought += 1;
        true
    }

    /// `Ai_TradeForCounty` (`0x0049E39B`).
    ///
    /// ```c
    /// if (county.merchantCount != 0 && realm.bankruptStage == 0) {
    ///     if (realm.want[wood] == 0) { if (pers.reserve84 < realm.wood)
    ///         Ai_SellGood(county, realm.wood - pers.reserve84, 8); }
    ///     else Ai_BuyGoodDownTo(county, realm.want[wood], 8);
    ///     /* iron against +0x8C and good 6, stone against +0x88 and good 7 */
    ///     good = weaponGood(county.industry);
    ///     if (pers.floor78 < realm.gold) Ai_BuyGoodDownTo(county, pers.qty7C, good);
    /// }
    /// ```
    ///
    /// The three resources pair a **realm** want (`+0x70`, `+0x74`, `+0x7C`)
    /// with a **personality** reserve,
    /// at these offsets — `docs/diplomacy.md` §8.4. A non-zero want buys that
    /// much; a zero want sells everything above the reserve.
    ///
    /// The weapon order is placed **once per county**,
    /// several counties re-tests the floor against a treasury the last order
    /// already emptied.
    fn trade_for_county(&mut self, id: usize, county: &mut County, map: &mut CampaignMap) {
        // The stall
        let Some(morale) = self.stall.get(id).copied().flatten() else {
            return;
        };
        let owner = county.owner as usize;
        let Some(realm) = self.realms.get(owner) else { return };
        if realm.bankrupt_stage != 0 {
            return;
        }
        let (lord, wood, iron, stone) = (realm.lord, realm.wood, realm.iron, realm.stone);
        let want = realm.want;
        let Some(&p) = self.t.ai_personality(lord) else { return };

        for (want_index, held, reserve, good) in [
            (crate::ai_army::WANT_WOOD, wood, p.reserve_wood, crate::trade::Good::Timber),
            (crate::ai_army::WANT_IRON, iron, p.reserve_iron, crate::trade::Good::Iron),
            (crate::ai_army::WANT_STONE, stone, p.reserve_stone, crate::trade::Good::Stone),
        ] {
            if want[want_index] == 0 {
                if reserve < held {
                    self.sell_good(county, map, morale, held - reserve, good);
                }
            } else {
                self.buy_good_down_to(county, map, morale, want[want_index], good);
            }
        }

        let gold = self.realms[owner].gold;
        if p.trade_gold_floor < gold {
            let good = weapon_good(county.weapon_type);
            self.buy_good_down_to(county, map, morale, p.weapon_buy_qty, good);
        }
    }
}

