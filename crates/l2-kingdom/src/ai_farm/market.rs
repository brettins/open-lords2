use super::*;

/// The ids are `L2.eng` group 6
/// live in the county. See `docs/symbols.md` on `0x004284CE` for the full list
/// — 3 *Sheep* and 5 *Wool* have no branch in either direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Good {
    Grain = 1,
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
pub trait Market {
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
    fn trade_for_county(&mut self, _id: usize, _county: &mut County, _map: &mut CampaignMap) {}
}

/// keeping at the type. Its comment used to read *"there is
/// no stall yet, so every style's opening shopping cascade is refused
/// county farms what it already has"*, and that stated reason was checked and
/// was **false**: `Ai_BuyGood`'s stall gate is county `+0x1A4`, which the six
/// one-turn-apart fixtures carry non-zero on every county holding a merchant,
///
/// was there, the money was there, and refusing the cascade cost the unowned
/// counties **50 sacks of grain a season each**. `docs/decisions.md`
/// C149; [`CountyStall`] is what phase 1 passes now.
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
/// 186 and 195. `docs/decisions.md` C149.
pub struct CountyStall<'a> {
    t: &'a Tables,
    stall: [Option<i32>; crate::county::MAX_COUNTIES],
    realms: &'a mut [Realm],
    season_next: Season,
    sowing: crate::ration::Sowing,
    armies_eat: bool,
    pub bought: i32,
}

impl<'a> CountyStall<'a> {
    /// Build the stall from the counties' own `+0x1A4` / `+0x1A5`
    /// array — exactly the two reads `Ai_BuyGood` makes.
    pub fn new(
        t: &'a Tables,
        counties: &[County; crate::county::MAX_COUNTIES],
        units: &crate::unit::Units,
        realms: &'a mut [Realm],
        season_next: Season,
        armies_eat: bool,
        sowing: crate::ration::Sowing,
    ) -> CountyStall<'a> {
        let mut stall = [None; crate::county::MAX_COUNTIES];
        for (id, slot) in stall.iter_mut().enumerate() {
            let county = &counties[id];
            if county.merchant_count == 0 {
                continue;
            }
            *slot = units.get(county.merchant_unit as usize).map(|u| u.morale);
        }
        CountyStall { t, stall, realms, season_next, armies_eat, sowing, bought: 0 }
    }

    pub fn price(&self, good: Good, morale: i32) -> i32 {
        let g = match good {
            Good::Grain => crate::trade::Good::Grain,
            Good::Cattle => crate::trade::Good::Cattle,
        };
        crate::trade::quote(self.t, g, morale).buy
    }

    /// `Ai_SellGood` (`0x004A4A3F`) — `Merchant_Trade(1, -qty, …)`, the stall's
    /// **base** price, no purse test of its own.
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
            self.sowing,
        );
    }

    /// `Ai_BuyGoodDownTo` (`0x004A4C41`) — `Ai_BuyGood` that **halves the lot**
    /// until the treasury covers it
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
                    self.sowing,
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
        let Some(morale) = self.stall.get(id).copied().flatten() else {
            return false;
        };
        let price = self.price(good, morale);
        let bill = price * qty;
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
            let realm = &mut self.realms[owner];
            realm.gold -= bill;
            realm.trade_spent_b += bill;
            realm.trade_spent_a += bill;
        }
        crate::trade::settle_county(
            self.t,
            county,
            map,
            self.armies_eat,
            self.season_next.index(),
            self.sowing,
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
    fn trade_for_county(&mut self, id: usize, county: &mut County, map: &mut CampaignMap) {
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

