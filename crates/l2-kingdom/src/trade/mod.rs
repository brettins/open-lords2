//! **The merchant's transaction** — `Merchant_Trade` (`0x004284CE`), the price
//! the screen quotes, and the two limits the arrows clamp to.
//!
//! Where each good lands is the interesting part, and it is `L2.eng` group 6's
//! id order:
//!
//! `FUN_00428DAF`, which runs when a good is picked, is the whole of it:
//!
//! The AI uses the identical markup through `Ai_BuyGood` (`0x004A4B12`) and
//! `Ai_SellGood` (`0x004A4A3F`), reading the merchant off the county's own
//! stall slot. So [`quote`] is the price for everybody.
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
//! `Merchant_Trade`'s tail runs seven more calls on the county. Six are here
//! now — `FUN_0046921D`, **buying cattle into a county with no pasture converts
//! a field into one**, is [`crate::field::ensure_pasture`] and is called from
//! [`trade`] on `good == Cattle && qty >= 0`. One is still not:
//!
//! * `FUN_00450CCD` — a degraded castle draws its owed wood and stone out of
//!   the realm's stockpile, so it is called here: it is what makes
//!   stone bought at the merchant reach the castle. `Readme.txt` says the
//!   opposite (*"don't expect to see it deducted right away"*), and settling
//!   which is right needs `castleDegraded` set in a live game.

mod engine;
pub use engine::*;
mod tests_part;
pub use tests_part::*;

use crate::county::County;
use crate::kingdom::Kingdom;
use crate::math::pct;
use crate::realm::Realm;
use crate::tables::{Tables, WEAPON_TYPE_COUNT};
use l2_net::{Quirk, Quirks};

/// `L2.eng` group 6 has fifteen strings and `g_goodsPrice` fifteen entries;
/// index 0 is a placeholder in both.
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

/// A fallback name. **`L2.eng` group 6** and the view layer
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

    pub fn tradeable(self) -> bool {
        !matches!(self, Good::Sheep | Good::Wool)
    }

    pub fn is_county_store(self) -> bool {
        matches!(self, Good::Grain | Good::Cattle)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quote {
    pub sell: i32,
    pub buy: i32,
}

impl Quote {
    pub fn markup(self) -> i32 {
        self.buy - self.sell
    }
}

/// `FUN_00428DAF`'s price arithmetic.
///
/// `merchant_morale` is `g_units[DAT_00553C64].morale` — the morale of the
/// merchant *being clicked*, which is 100 for every merchant the shipped game
/// creates. The floor of one crown is applied **before** ale's exemption, which
/// is the order that makes ale free of markup.
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

pub fn max_buy(gold: i32, buy_price: i32) -> i32 {
    if buy_price == 0 {
        0
    } else {
        gold / buy_price
    }
}

pub fn min_qty(counties: &[County], realms: &[Realm], good: Good, realm: usize, county: usize) -> i32 {
    -stock(counties, realms, good, realm, county)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    NotEnoughStock,
    /// `g_realms[realm].gold < buyPrice * qty`. `L2.eng` 68/20 says it in the
    /// game's own words: *"You do not have enough crowns to buy, my Lord."*
    NotEnoughGold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Receipt {
    pub crowns: i32,
    pub moved: i32,
    pub ale_happiness: i32,
}

/// `buy_price` and `sell_price` are carried because the
/// original carries them: the screen quotes a price, the player agrees to it,
/// and the confirmed trade runs on the quoted number. In multiplayer they go
/// on the wire (`Net_WriteField(&DAT_0055CD70, 4)`), which is the same
/// statement — the price is part of the command, not something the far end
/// works out again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Order {
    pub qty: i32,
    pub good: Good,
    pub buy_price: i32,
    pub sell_price: i32,
    pub realm: usize,
    pub county: usize,
}

impl Order {
    pub fn buy(good: Good, qty: i32, q: Quote, realm: usize, county: usize) -> Order {
        Order { qty, good, buy_price: q.buy, sell_price: q.sell, realm, county }
    }

    pub fn sell(good: Good, qty: i32, q: Quote, realm: usize, county: usize) -> Order {
        Order { qty: -qty, good, buy_price: q.buy, sell_price: q.sell, realm, county }
    }
}

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

