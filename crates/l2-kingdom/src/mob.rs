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
//! * **A revolt *raises* happiness.** The `+0x1e` after `County_RaiseRevolt`
//! is thirty back onto a county below ten, and the toll below then takes the
//! `-10` arm
//!   `happiness + 20`. The peasants who were angry have left the population;
//!   this is the same accounting as the levy.
//!
//! The trailing `happiness < 0` clamp is the original's last statement and it
//! runs on **every** mob tick, crossing or not. It is a no-op unless happiness
//! is already negative, and nothing in this crate writes a negative happiness
//! — [`crate::conquest`]'s penalty and [`crate::happiness`] both floor at 0 —
//! so it is folded into [`settle`]
//! is unreachable; `[V]` that the original tests it every tick.

use crate::county::County;
use crate::diplomacy::Letter;

/// `L2.eng` 154 — *"Revolution in your lands."* Message id `0x9A`, posted when
/// the mob walks into a county already below 10 happiness.
pub const GROUP_REVOLUTION_SPREAD: u16 = 0x9A;
pub const GROUP_TROUBLE_SPREADING: u16 = 0x9B;
pub const GROUP_PEOPLE_TROUBLED: u16 = 0x9C;

pub const CATEGORY_COUNTY_NOTICE: u8 = 3;

pub const HAPPINESS_TOLL: i32 = 10;

/// What a successful `County_RaiseRevolt` puts back — `+0x1e`.
pub const REVOLT_RELIEF: i32 = 30;

pub const WRETCHED: i32 = 10;

pub const TROUBLED: i32 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Crossing {
    pub letter: Letter,
    pub raise_revolt: bool,
}

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
    Revolution,
    Trouble,
    Troubled,
}

impl Crossing {
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
