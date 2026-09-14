//! Raising an army — the levy, the armoury basket, and `Army_Create`.
//! `docs/armies.md` §6.
//!
//! **Recruitment is a headcount.** You take a percentage of a county's people
//!;
//! recruitment price anywhere on this path. What the men are *carrying* is
//! decided separately, out of the realm's weapon stockpiles, and a man with no
//! weapon is a peasant.
//!
//! # The three numbers, and which one is charged
//!
//! Worth saying once, because the original says it three different ways: the
//! raise-army screen prints `men / 2` as the seasonal wage, `g_mercWage` holds
//! `price / 10` and is never read by anything, and
//! [`crate::realm::Realm::wage_for_unit`] charges `men / 4`. **Only the last is
//! spent.**
//!
//! # No oracle exists for any of this
//!
//! The shipped `lastturn.sav` holds six units and **all six are merchants**.
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
/// slot 0 is the unequipped men, slots 1…6 are the six weapon types, and slot 7
/// is the levy total.
pub const BASKET_SLOTS: usize = TROOP_TYPES + 1;

/// The slot holding the total, which the `+`/`−` buttons never touch and which
/// is what `Army_Create` writes into the army's `men`.
pub const BASKET_TOTAL: usize = TROOP_TYPES;

/// One slot of the armoury basket.
///
/// Three fields: `available` is the number the
/// screen prints, `remaining` is what the `+` button compares against. Both are
/// seeded from the same stockpile and they only diverge on the AI's auto-equip
/// path, which decrements `remaining` and leaves `available` alone.
///
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

/// The armoury scratch buffer for one realm: who is being equipped with what.
///
/// > **`docs/armies.md` §6.2 attributes this to `Levy_Init` (`0x004AAA80`).
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
    ///
    /// ```c
    /// for t in 0..8: slot[t].chosen = slot[t].available = 0;
    /// for t in 1..7: slot[t].available = slot[t].remaining = realm.weapons[t - 1];
    /// slot[0].chosen = slot[0].remaining = men;
    /// slot[7].chosen = slot[7].remaining = men;
    /// ```
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
/// What a levy percentage would cost and yield — the two globals
/// `Levy_SetPercent` writes, and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Levy {
    /// `g_levyMen` (`0x00543FD8`).
    pub men: i32,
    /// `g_levyHappinessCost` (`0x00565400`).
    pub happiness_cost: i32,
    /// The percentage the walk-back settled on.
    ///
    /// **The original does not write this back.** `pct` is a by-value parameter
    /// and `g_levyPercent` stays where the player put the slider while the two
    /// globals hold the reduced figures — so the slider can read 80% while the
    /// county is only giving up 59%. Carried here because a caller needs to know
    /// what
    /// cannot be mistaken for the slider.
    pub settled: i32,
}

/// Why a raise-army order was refused.
///
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
    /// road tile and no free open tile in the county to put the army on.
    NowhereToStand,
}

/// How far from the county's anchor a new army may be put — the `3` both
/// `County_FindFree*Tile` loops stop at.
const MUSTER_RADIUS: i32 = 3;

/// Everything `create_army` needs that is not a county, a realm or a basket.
#[derive(Debug, Clone, Copy)]
pub struct Muster {
    /// The realm raising it. **0 raises an ownerless unit** — the unit's owner
    /// byte becomes 6 and its shield 0 — which is how a neutral county's
    /// militia is created. `docs/armies.md` §1.1 records the 6 and §6.3 does not
    /// connect it to this branch.
    pub realm: u8,
    pub county: u8,
    /// The happiness the county is charged. **A parameter, not a global**: the
    /// AI paths pass an unclamped, surcharge-free figure straight out of the
    /// table, which is what makes the clamp in [`create_army`] reachable.
    pub happiness_cost: i32,
    pub year: i32,
}

/// The owner byte `Army_Create` writes for `realm == 0`: an **ownerless** unit,
/// which is what a neutral county's militia is. `docs/armies.md` §1.1 records
/// the value without naming the branch that produces it.
pub const OWNERLESS: u8 = 6;

/// How a county's defence is equipped when it is raised to meet an invader —
/// `FUN_004A50AE`'s three modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Defence {
    /// Mode 0 — a **human**-owned county. Nothing is equipped at all: the
    /// defence is entirely peasants. `[D]`, and it is the harshest of the three.
    HumanCounty,
    /// Mode 1 — an **AI**-owned county. The auto-equip round-robin runs over
    /// the realm's real stockpiles.
    AiCounty,
    /// Mode 2 — a **neutral** county. It has no realm and no stockpile, so the
    /// function *grants* 500 each of pikes, bows and maces to realm 0 and then
    /// equips by a size ladder. The happiness cost is forced to zero: nobody
    /// owns the county to be angry at.
    Militia,
}

/// The size ladder a neutral county's militia is equipped by, as
/// `(men at least, archers, pikemen, macemen)`.
///
/// Read straight out of `FUN_004A50AE`'s nested `if`, which tests 480, 360, 240
/// and 120 in that order and equips nobody below 120. The three slots are
/// basket slots 5, 4 and 2 — archers, pikemen, macemen —
/// is decremented by the sum. `[D]`
pub const MILITIA_LADDER: [(i32, i32, i32, i32); 4] =
    [(480, 150, 100, 50), (360, 100, 70, 0), (240, 80, 40, 0), (120, 60, 0, 0)];

/// The population a county must have before it will raise a defence at all —
/// `FUN_004A50AE`'s `if (county.population < 40) return 0`.
pub const DEFENCE_MIN_POPULATION: i32 = 40;

/// The percentage of its population a **neutral** county levies to defend
/// itself, by difficulty 0…3.
pub const MILITIA_PERCENT_BY_DIFFICULTY: [i32; 4] = [25, 40, 50, 60];

/// The percentage an **owned** county levies. Flat, regardless of difficulty.
pub const DEFENCE_PERCENT: i32 = 40;

/// What `FUN_004A50AE` grants realm 0 of each of pikes, bows and maces before
/// equipping a neutral county's militia.
pub const MILITIA_GRANT: i32 = 500;

