//! The document traces the record, the levy, movement, supply, sieges and how a
//! battle *result* comes back, and there is a hole in the middle of it: what
//! makes an army standing on a county's castle *take* the county. That is
//! `FUN_004A6C68` (`0x004A6C68`, 1,264 bytes), called from the mover, and
//! nothing in the document mentions it, `FUN_004A50AE` which raises the
//! defence, or `FUN_004A72FE` which changes the owner. Three unnamed functions
//! between "armies move" and "there is a war". They are named here as
//! `Army_AttackCounty`, `County_RaiseDefence` (in [`crate::levy`]) and
//! `County_ChangeOwner`.
//!
//! **By stepping onto its castle tile.** `Unit_StepOnce` returns 5 for a
//! `plane0 & 0x40` tile, the mover then calls
//! `Transport_Deliver` and `Army_AttackCounty` with the tile's county. So the
//! castle site is expensive terrain the pathfinder routes around —
//! it is the objective, and `docs/armies.md` §2.2's castle row (*"5 →
//! `Transport_Deliver`, move ends"*) is half the story. `[D]`
//!
//! `[D]`

mod ownership;
pub use ownership::*;
mod attack;
pub use attack::*;
mod castle;
pub use castle::*;

#[cfg(test)]
use crate::county::{County, MAX_COUNTIES};
#[cfg(test)]
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Season;
#[cfg(test)]
use crate::tables::{Tables};
#[cfg(test)]
use crate::unit::{ArmyNames, UnitKind, Units};

/// **The four globals `County_MakeIndependent` (`0x004AC3C6`) reads** and
/// `County_ChangeOwner`'s own arguments do not carry: `g_optArmiesEat` for
/// `Ration_Apply`, `g_seasonNext` and `g_optAdvancedFarming` for
/// `County_RefreshEstimates`, and `g_countyCount` for the blacksmiths' share.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Restore {
    /// `g_season` and `g_optAdvancedFarming` as `Ration_Apply` (`0x0044DF5F`)
    /// wants them: entering Spring it holds back the seed corn.
    pub sowing: crate::ration::Sowing,
    pub season_next: Season,
    pub advanced_farming: bool,
    pub armies_eat: bool,
    pub county_count: usize,
}

impl Restore {
    pub const NEUTRAL: Restore = Restore {
        sowing: crate::ration::Sowing::NONE,
        season_next: Season::Spring,
        advanced_farming: false,
        armies_eat: false,
        county_count: 0,
    };
}

/// In the shipped save every neutral county sits at 77, so on the England map
/// **every neutral county is a battle and none is a walk-in** at turn one. The
/// rule only bites once a county has been taxed or starved into misery, which
/// is the *"the people are wretched, my liege"* county of
/// `docs/armies.md` §2.5's greeting table. `[D]`
pub const SURRENDER_HAPPINESS: i32 = 11;

/// **An AI conqueror always costs 30; a human's cost rises with the
/// difficulty** — 10 at Easy, 30 at Normal, 50 at Hard. So the two are equal at
/// Normal and the setting decides whether conquest is cheaper or dearer for the
/// player than for the AI. `[D]`
pub fn capture_happiness_penalty(conqueror_is_human: bool, difficulty: u8) -> i32 {
    if conqueror_is_human {
        difficulty as i32 * 20 + 10
    } else {
        30
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    NotAnArmy,
    Ownerless,
    AlreadyYours,
    Garrisoned,
}

pub const ATTACK_MOVE_COST: i32 = 8;

/// `+0x167 = 1` — a defence levied out of the county's own people the moment
/// the attacker arrived. Winning with it takes the county; it is dissolved
/// afterwards and its survivors go home.
pub const RAISED: u8 = 1;
/// `+0x167 = 2` — an army that was already standing at the town. Winning with
/// it takes the county; it keeps its men and only loses the mark.
pub const PRESSED: u8 = 2;
pub const UNMARKED: u8 = 0;

/// **What `County_ChangeOwner` (`0x004A72FE`) knew when it chose its letter**,
/// as a value the same on every peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capture {
    pub new_owner: u8,
    pub old_owner: u8,
    pub county: u8,
    pub held_before: u8,
    /// Realm `+0x2A` before this capture raised it —
    /// [`crate::realm::Realm::peak_counties`].
    pub peak_before: u8,
    pub governable: bool,
    pub penalty: i32,
    /// What became of the castle's garrison — `FUN_00437535`, reached from
    /// `County_MakeIndependent`'s tail. `None` when the castle was empty or
    /// the garrison already belonged to the taker. See [`change_owner`].
    pub garrison: Option<LeftCastle>,
}

impl Capture {
    pub fn held_after(&self) -> u8 {
        self.held_before.wrapping_add(1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastleArrival {
    Garrisoned(usize),
    GarrisonFull,
    Siege(Result<(), crate::siege::SiegeRefusal>),
    NotAnArmy,
}

pub const GARRISON_MOVE_COST: i32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftCastle {
    NotAGarrison,
    /// `FUN_00437535` hands that pair to `Battle_BeginFromCampaign` and sets
    /// `g_battleCounty` to the county. **The battle is not staged here** —
    /// this crate cannot fight one; `l2_game::turn::raise_sortie` is the
    /// caller that does. The field is the original's `besiegedBy` test.
    Marched { tile: (u8, u8), sortie: Option<usize> },
    Destroyed,
}

#[cfg(test)]
mod tests;

