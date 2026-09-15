//!;
//! There is not one army in it, so every army-only field — the troop counts,
//! the wage, the morale, the mercenary triple, the move allowance — has no
//! data-side confirmation available at all. Everything here is `[D]` from the
//! instruction stream or `[V]` where a *static table* in the executable or an
//! `L2.eng` string pins it, and nothing in this module is `[V]` on the strength
//! of a save.

mod basket;
pub use basket::*;
mod raise;
pub use raise::*;
mod tests;
pub use tests::*;

use crate::county::{County, MAX_COUNTIES};
use crate::map::{flags, CampaignMap};
use crate::math::pct;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::{Tables, WEAPON_TYPE_COUNT};
use crate::unit::{ArmyNames, TroopType, Unit, UnitKind, Units, TROOP_TYPES};

/// `g_levyBasket` (`0x0053F6A0`) has **eight slots of `0x10` bytes** per realm:
pub const BASKET_SLOTS: usize = TROOP_TYPES + 1;

pub const BASKET_TOTAL: usize = TROOP_TYPES;

/// The original has a fourth field at `+0x0C` that neither seeding function
/// clears — a "chosen count when this slot was last selected" latch that fires
/// the troop-portrait animation. It is presentation and it is not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BasketSlot {
    /// `+0x00` — how many men are in this slot.
    pub chosen: i32,
    /// `+0x04` — the stockpile, as drawn.
    pub available: i32,
    /// `+0x08` — the stockpile, as spent.
    pub remaining: i32,
}

/// > **`docs/armies.md` §6.2 attributes this to `Levy_Init` (`0x004AAA80`).
///
/// > That is the AI's function.** Its only callers are the three AI
/// > army-raising helpers. The **player's** raise-army screen goes through
/// > `FUN_004AA90A`, which is the same seeding plus three UI resets. The
/// > distinction matters for modding — a rule attached to the wrong one of the
/// > two would apply to only half the armies in the game —
/// > itself is identical, so [`LevyBasket::seed`] is both. Corrected in the
/// > document. `[V]`
///
/// The realm index is **the county's owner**, not a realm handed in: both
/// seeding functions read `county[+0x05]`. They agree in play and would not for
/// an ownerless county, which is exactly the case a neutral county's militia
/// hits — see [`raise_defence`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LevyBasket {
    pub slots: [BasketSlot; BASKET_SLOTS],
}

impl LevyBasket {
    /// `Levy_Init` / `FUN_004AA90A` — clear the basket, seed slots 1…6 from the
    /// realm's weapon stockpiles, and put every man in slot 0 and slot 7.
    pub fn seed(realm: &Realm, men: i32) -> LevyBasket {
        let mut basket = LevyBasket::default();
        for t in 1..=WEAPON_TYPE_COUNT {
            let stock = realm.weapons[t - 1];
            basket.slots[t].available = stock;
            basket.slots[t].remaining = stock;
        }
        basket.slots[0].chosen = men;
        basket.slots[0].remaining = men;
        basket.slots[BASKET_TOTAL].chosen = men;
        basket.slots[BASKET_TOTAL].remaining = men;
        basket
    }

}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Levy {
    /// `g_levyMen` (`0x00543FD8`).
    pub men: i32,
    /// `g_levyHappinessCost` (`0x00565400`).
    pub happiness_cost: i32,
    pub settled: i32,
}

/// **All three live in the confirm handler `FUN_00435B4D`, not in
/// `Army_Create`.** `docs/armies.md` §6.3 attributes the first two to
/// `Army_Create`; the third it does not mention at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevyRefusal {
    /// Message `0xA8` = `L2.eng` group 168, *"Your proposed army of zero men
    /// fails to fulfill certain principles of medieval troop management"*.
    NoMen,
    /// Message `0x94` = group 148, *"impractical to create an army of less than
    /// 50 men"*.
    TooFew,
    /// Message `0xDD` = group 221. `Army_Create` returned 0:
    NowhereToStand,
}

const MUSTER_RADIUS: i32 = 3;

#[derive(Debug, Clone, Copy)]
pub struct Muster {
    pub realm: u8,
    pub county: u8,
    pub happiness_cost: i32,
    pub year: i32,
}

pub const OWNERLESS: u8 = 6;

/// How a county's defence is equipped when it is raised to meet an invader —
/// `FUN_004A50AE`'s three modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Defence {
    /// Mode 0 — a **human**-owned county. Nothing is equipped at all: the
    /// defence is entirely peasants. `[D]`, and it is the harshest of the three.
    HumanCounty,
    AiCounty,
    Militia,
}

/// Read straight out of `FUN_004A50AE`'s nested `if`, which tests 480, 360, 240
/// and 120 in that order and equips nobody below 120. The three slots are
/// basket slots 5, 4 and 2 — archers, pikemen, macemen —
/// is decremented by the sum. `[D]`
pub const MILITIA_LADDER: [(i32, i32, i32, i32); 4] =
    [(480, 150, 100, 50), (360, 100, 70, 0), (240, 80, 40, 0), (120, 60, 0, 0)];

/// The population a county must have before it will raise a defence at all —
/// `FUN_004A50AE`'s `if (county.population < 40) return 0`.
pub const DEFENCE_MIN_POPULATION: i32 = 40;

pub const MILITIA_PERCENT_BY_DIFFICULTY: [i32; 4] = [25, 40, 50, 60];

pub const DEFENCE_PERCENT: i32 = 40;

/// What `FUN_004A50AE` grants realm 0 of each of pikes, bows and maces before
/// equipping a neutral county's militia.
pub const MILITIA_GRANT: i32 = 500;

