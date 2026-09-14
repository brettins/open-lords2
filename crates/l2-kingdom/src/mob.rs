//! **A mob of revolting peasants crossing a border** — `FUN_004ABD0F`
//! (`0x004ABD0F`), the only thing `PeasantMob_Tick` (`0x00465486`) does with a
//! county that is not a troop recount.
//!
//! [`crate::arrival`] built `Unit_EnterCounty`'s two letters and named this
//! function as *"a different rule"*, out of scope then. This is it. The mob
//! carries its revolution across the border: it posts one of `L2.eng` 154…156
//! to the county's **owner**, takes ten off the county's happiness, and can
//! raise that county too. `[V]`, the whole function:
//!
//! ```c
//! void FUN_004abd0f(int unit, uint county) {
//!     if (g_counties[county].owner == 0) { g_units[unit].county = county; }
//!     else {
//!         if (g_units[unit].county != county) {
//!             g_units[unit].county = county;
//!             if (g_counties[county].happiness < 10) {
//!                 Msg_Enqueue(0, county.owner, 0x9a, 0, 3, county, 0, 0);
//!                 if (County_RaiseRevolt(county)) {
//!                     county.unrest = 0;
//!                     county.happiness += 0x1e;  county.shownEvents += 0x1e;
//!                 }
//!             } else if (g_counties[county].happiness < 0x1e) {
//!                 Msg_Enqueue(0, county.owner, 0x9b, 0, 3, county, 0, 0);
//!                 if (county.unrest == 0) county.unrest = 1;
//!             } else Msg_Enqueue(0, county.owner, 0x9c, 0, 3, county, 0, 0);
//!             if (g_counties[county].happiness < 10) {
//!                 county.shownEvents -= county.happiness;  county.happiness = 0;
//!             } else { county.happiness += -10; county.shownEvents += -10; }
//!         }
//!         if (g_counties[county].happiness < 0) g_counties[county].happiness = 0;
//!     }
//! }
//! ```
//!
//! # Four things the shape decides
//!
//! * **An unowned county is silent.** The mob's county byte is written and
//!   nothing else happens — no letter (there is nobody to write to), no toll,
//!   no revolt. A mob wandering neutral land is free.
//! * **The letter is chosen on the happiness the mob found**, before the ten is
//!   taken: a county at exactly 10 gets 155, at exactly 30 gets 156.
//! * **A revolt *raises* happiness.** The `+0x1e` after `County_RaiseRevolt`
//! is thirty back onto a county below ten, and the toll below then takes the
//! `-10` arm
//!   `happiness + 20`. The peasants who were angry have left the population;
//!   this is the same accounting as the levy.
//! * **The toll is re-tested, not remembered.** The second `happiness < 10` is
//!   a fresh read, which is how the revolt branch escapes the floor.
//!
//! The trailing `happiness < 0` clamp is the original's last statement and it
//! runs on **every** mob tick, crossing or not. It is a no-op unless happiness
//! is already negative, and nothing in this crate writes a negative happiness
//! — [`crate::conquest`]'s penalty and [`crate::happiness`] both floor at 0 —
//! so it is folded into [`settle`]
//! is unreachable; `[V]` that the original tests it every tick.
//!
//! # Determinism
//!
//! Two integer comparisons and integer deltas; the revolt itself is
//! [`crate::unrest::raise_revolt`]'s tile search, which is already in the
//! digest. No float, no map iteration. `docs/netcode.md`.

use crate::county::County;
use crate::diplomacy::Letter;

/// `L2.eng` 154 — *"Revolution in your lands."* Message id `0x9A`, posted when
/// the mob walks into a county already below 10 happiness.
pub const GROUP_REVOLUTION_SPREAD: u16 = 0x9A;
/// 155 — *"Trouble spreading."* `0x9B`, happiness 10…29.
pub const GROUP_TROUBLE_SPREADING: u16 = 0x9B;
/// 156 — *"People are troubled."* `0x9C`, happiness 30 and up.
pub const GROUP_PEOPLE_TROUBLED: u16 = 0x9C;

/// `Msg_DrawWindow`'s category 3 — `l2_game::message::category::COUNTY_NOTICE`,
/// the county-titled notice box. All three letters use it.
pub const CATEGORY_COUNTY_NOTICE: u8 = 3;

/// What the mob costs the county it walks into, whatever it says.
pub const HAPPINESS_TOLL: i32 = 10;

/// What a successful `County_RaiseRevolt` puts back — `+0x1e`.
pub const REVOLT_RELIEF: i32 = 30;

/// The first happiness threshold: below this the mob's arrival may raise the
/// county.
pub const WRETCHED: i32 = 10;

/// The second: below this the county is put on unrest 1.
pub const TROUBLED: i32 = 30;

/// What a crossing asks of its caller: the letter to post, and whether
/// `County_RaiseRevolt` is to be attempted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Crossing {
    /// The letter, addressed to the county's owner.
    pub letter: Letter,
    /// True on the `happiness < 10` arm — the caller runs
    /// [`crate::unrest::raise_revolt`] and tells [`settle`] what happened.
    pub raise_revolt: bool,
}

/// **The letter half.** `None` for an unowned county — the whole `owner == 0`
/// branch is one assignment the stepper has already made.
///
/// Read on the happiness as found, before [`settle`] takes its ten.
pub fn crossing(c: &County, county: u8) -> Option<Crossing> {
    if c.owner == 0 {
        return None;
    }
    let (group, raise_revolt) = if c.happiness < WRETCHED {
        (GROUP_REVOLUTION_SPREAD, true)
    } else if c.happiness < TROUBLED {
        (GROUP_TROUBLE_SPREADING, false)
    } else {
        (GROUP_PEOPLE_TROUBLED, false)
    };
    Some(Crossing {
        letter: Letter {
            from: 0,
            to: c.owner,
            group,
            variant: 0,
            category: CATEGORY_COUNTY_NOTICE,
            county,
            payload: 0,
        },
        raise_revolt,
    })
}

/// **The writing half**, in the original's order: the arm's own write, then the
/// toll, then the clamp.
///
/// `revolted` is `County_RaiseRevolt`'s return — true only when a mob was
/// placed. It is read on the `Revolution` arm and nowhere else.
pub fn settle(c: &mut County, crossing: &Crossing, revolted: bool) {
    match crossing.group_arm() {
        Arm::Revolution => {
            if revolted {
                c.unrest = 0;
                c.happiness += REVOLT_RELIEF;
                c.shown_events += REVOLT_RELIEF;
            }
        }
        Arm::Trouble => {
            if c.unrest == 0 {
                c.unrest = 1;
            }
        }
        Arm::Troubled => {}
    }
    // Re-read, not the value `crossing` was chosen on: thirty back off a
    // revolt lifts the county out of this arm.
    if c.happiness < WRETCHED {
        c.shown_events -= c.happiness;
        c.happiness = 0;
    } else {
        c.happiness -= HAPPINESS_TOLL;
        c.shown_events -= HAPPINESS_TOLL;
    }
    if c.happiness < 0 {
        c.happiness = 0;
    }
}

/// Which of `FUN_004ABD0F`'s three arms a crossing took.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arm {
    /// Letter 154, happiness below 10.
    Revolution,
    /// 155, below 30.
    Trouble,
    /// 156, the rest.
    Troubled,
}

impl Crossing {
    /// The arm, off the group the letter carries.
    pub fn group_arm(&self) -> Arm {
        match self.letter.group {
            GROUP_REVOLUTION_SPREAD => Arm::Revolution,
            GROUP_TROUBLE_SPREADING => Arm::Trouble,
            _ => Arm::Troubled,
        }
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tick_tests;
