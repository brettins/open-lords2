//! `Industry_Produce` (`0x0044EA92`) runs four times per county per season:
//!
//! | commodity | job (`L2.eng` group 74) | divisor | base | credited to |
//! |---|---|---:|---:|---|
//! | wood | 6 Wood cutting | 1 | **20%** | realm `+0x130` |
//! | iron | 4 Iron mining | 1 | 15% | realm `+0x120` |
//! | weapons | 7 Blacksmith | **4** | 15% | realm `+0x140 + type*4` |
//! | stone | 5 Stone quarrying | **2** | 15% | realm `+0x128` |
//!
//! `Wages_ForUnit` (`0x004AD52B`) is the whole army upkeep rule, and troop type
//! does not enter it: a knight and a peasant cost the same. See
//! [`crate::realm::Realm::wage_for_unit`].

mod production;
pub use production::*;
mod wages;
pub use wages::*;
mod castles;
pub use castles::*;
mod ui;
pub use ui::*;

#[cfg(test)]
use crate::county::County;
#[cfg(test)]
use crate::realm::Realm;
#[cfg(test)]
use crate::report::Message;
use crate::tables::{Commodity, Tables, WEAPON_TYPE_COUNT};
#[cfg(test)]
use l2_net::Quirks;

/// The two denominators `FUN_0044F15B` (`0x0044F15B`) computes before every
/// weapons `resourceLimit`,
///
/// Two counties forging *different* weapons therefore split the stockpile in
/// proportion to what each weapon costs, not evenly. **`[V]`**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeaponShare {
    pub wood: i32,
    pub iron: i32,
}

impl WeaponShare {
    pub const UNSHARED: WeaponShare = WeaponShare { wood: 1, iron: 1 };

    pub fn single_smith(t: &Tables, weapon_type: usize) -> WeaponShare {
        let weapon = weapon_type.min(WEAPON_TYPE_COUNT - 1);
        WeaponShare {
            wood: t.weapon[weapon].wood.max(1),
            iron: t.weapon[weapon].iron.max(1),
        }
    }

    pub const SINGLE_SMITH: WeaponShare = WeaponShare::UNSHARED;
}

/// What an unpaid season does to a realm's armies — the escalation
/// `docs/kingdom.md` §2 records as `+0x158`, *"0 … 5"*, with no idea what the
/// stages do.
///
/// They are traced now, and each one is confirmed by the `L2.eng` group its
/// handler raises. That is the same cross-check the event table gets: the
/// message the player is shown says, in prose, what the code does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BankruptcyAction {
    None,
    /// **Stage 0 → 1.** Every mercenary in every army walks off, and any army
    /// left with no men at all is destroyed. Message `0xA0`, `L2.eng` group 160
    /// — *"Mercenaries desert! You have insufficient crowns in your treasury to
    /// pay for the services of all of your troops. The mercenaries promptly
    /// deserted, in search of a more reliable employer."*
    MercenariesDesert,
    /// **Stage 0 → 1 when the realm had no mercenaries to lose.** A warning and
    /// nothing more. Message `0x10E`, group 270 — *"Unpaid troops. … Be warned,
    /// your men will not tolerate this situation for long!"*
    Warned,
    /// **Stages 1, 2 and 3 → 2, 3, 4.** Every army loses men. Message `0x11F`,
    /// group 287 — *"Angry troops. You still have not payed your soldiers. Your
    /// captains report that your armies are losing men."*
    Desertion,
    /// **Stage 4 → 5.** The same desertion, with a final warning. Message
    /// `0x10F`, group 271 — *"Mutinous troops. … it has been over a year since
    /// your men received any wages. They say that the men will revolt next
    /// season."* **"Over a year" is a check on the stage count**: stage 4 is
    /// reached on the fourth unpaid season, which is four seasons, which is a
    /// year.
    LastWarning,
    Mutiny,
}

impl BankruptcyAction {
    /// The original's message id, which is also its `L2.eng` group.
    pub fn message_id(self) -> Option<u16> {
        match self {
            BankruptcyAction::None => None,
            BankruptcyAction::MercenariesDesert => Some(0xA0),
            BankruptcyAction::Warned => Some(0x10E),
            BankruptcyAction::Desertion => Some(0x11F),
            BankruptcyAction::LastWarning => Some(0x10F),
            BankruptcyAction::Mutiny => Some(0x110),
        }
    }

    pub fn rival_message_id(self) -> Option<u16> {
        match self {
            BankruptcyAction::Mutiny => Some(0x111),
            _ => None,
        }
    }
}

/// Why the castle chooser's OK button refused — `FUN_00436B59`'s two guards,
/// and they are the **only** two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastleRefusal {
    NoSuchType,
    /// The type picked is the one already standing. Message `0x93` = `L2.eng`
    /// **147**.
    AlreadyBuilt,
    /// The type picked is **smaller** than the one standing. Message `0x122` =
    /// `L2.eng` **290**. You may never knock a castle down to build a lesser
    /// one.
    Downgrade,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CastleComplete {
    pub free_archers: i32,
    pub repaired: bool,
}

/// 13…20 nothing at all, 21 and above the castle.** `[D]` — `Map_Click`'s own
/// `if`/`else if` chain, and it is the only way to reach
/// [`toggle_from_map`]: **nothing on any county panel switches an industry**.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MapToggle {
    /// One of the four commodities: its enable byte, county `+0x297 + c*0x18`.
    Industry(Commodity),
    /// County `+0x1B0` — the switch a player throws by dragging builders onto
    /// the castle,
    /// building. Until now this crate had no such field and treated it as
    /// permanently thrown ([`crate::labour::ceilings`]).
    Castle,
}

#[cfg(test)]
mod tests;

