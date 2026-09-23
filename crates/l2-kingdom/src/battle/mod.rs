//! ```text
//! Army_AttackCounty                 crate::conquest — a defender is settled
//! FUN_004A6A30       §1  choose     autocalc, prompt, or straight to a report
//! FUN_004AAD07       §2  autocalc   strength, a ratio, a survival percentage
//! …or l2-sim runs a real battle and writes the survivors back…
//! Battle_ReturnToCampaign §3        the county, the moves, the loser
//! Defence_Disband    §4             the levy goes home
//! ```

mod settlement;
pub use settlement::*;
mod campaign;
pub use campaign::*;

#[cfg(test)]
use crate::county::{County, MAX_COUNTIES};
#[cfg(test)]
use crate::realm::{Realm, MAX_REALMS};
#[cfg(test)]
use crate::tables::Tables;
#[cfg(test)]
use crate::unit::{ArmyNames, Unit, Units, TROOP_TYPES};


/// How a battle between two campaign units is going to be settled —
/// `FUN_004A6A30` (`0x004A6A30`) and the one `if` at every one of its three
/// callers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Settlement {
    Silently,
    Reported,
    /// Screen `0x12`: *"A Battle is to be fought. Will you take the field?"*
    /// Taking the field is the real simulation;
    /// [`Settlement::Reported`] by another route, because `FUN_0043B622` runs
    /// the same autocalc.
    Prompt,
}

/// `g_optFightHumansOnly` (`0x0053F284`) — the advanced option *"Fight humans
/// only?"*, **stored inverted**: the byte is 0 when the option displays *Yes*.
pub const FIGHT_HUMANS_ONLY_DEFAULT: u8 = 1;

/// `Table_Lookup(ratio, DAT_004DE710, 10, 100)` — **how much of the winner
/// survives**, as a percentage, by how far ahead of the loser it was.
///
/// Read out of `Lords2.exe` at `0x004DE710`, ten `(threshold, value)` pairs.
///
/// `[V]` — the thresholds ascend strictly and the eleventh pair in memory is
/// `(100, 200)`, a different table starting, which is what fixes the count at
/// ten.
pub const SURVIVAL_LADDER: [(i32, i32); 10] = [
    (110, 10),
    (130, 20),
    (160, 30),
    (180, 40),
    (220, 50),
    (270, 65),
    (360, 80),
    (500, 90),
    (700, 95),
    (900, 98),
];

pub const SURVIVAL_DEFAULT: i32 = 100;

pub fn survival_percent(ratio: i32) -> i32 {
    SURVIVAL_LADDER
        .iter()
        .find(|&&(threshold, _)| ratio < threshold)
        .map_or(SURVIVAL_DEFAULT, |&(_, value)| value)
}

pub const CASTLE_STRENGTH_PERCENT: [i32; 5] = [160, 200, 250, 320, 400];

/// `g_battleOutcome` (`0x00553FB8`) — which of `L2.eng` group 82's **seven**
/// heading/body pairs `Screen_BattleOutcome` (screen `0x2B`) shows.
///
/// A battle does not end two ways. `FUN_00478419` (`0x00478419`) sorts it into
/// six by two questions — was it a siege, and is the local player the winner —
/// and the seventh is the pair for a battle the local player was in neither
/// side of.
///
/// It is also a **fourth independent site confirming that `g_battleLoser`
/// holds the winner**: `DAT_005440CC` is `g_units[g_battleLoser].owner`, and
/// this function maps `g_localPlayer == DAT_005440CC` onto the *won* pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Outcome {
    Won = 0,
    Lost = 1,
    SiegeWon = 2,
    SiegeLost = 3,
    SiegeLifted = 4,
    CastleLost = 5,
    Bystander = 6,
}

impl Outcome {
    /// The index into `L2.eng` group 82: the heading is at `2n` and the body at
    /// `2n + 1`.
    pub fn pair(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Aftermath {
    pub county_taken_by: Option<u8>,
    pub loser_destroyed: bool,
    pub loser_siege_lifted: bool,
    pub offence: Option<(u8, u8)>,
    pub winner_moves_used: i32,
    pub withdrawal_casualties: Option<i32>,
    /// The realm that lost, **captured before its record was emptied** — the
    /// argument `Realm_RecountStrength` (`0x0049B42B`) takes at the bottom of
    /// both of `Battle_ReturnToCampaign`'s branches.
    pub loser_owner: u8,
    pub captures: [Option<crate::conquest::Capture>; 2],
}

pub const BATTLE_OFFENCE: i32 = 20;

pub const WINNER_MOVE_COST: i32 = 8;

pub const AI_DEFENDER_MOVE_COST: i32 = 7;

/// The men a **withdrawing** army needs to survive its withdrawal — the
/// Readme's *Retreats (pg82)* number, and `Battle_ReturnToCampaign`'s
/// `menTotal < 0x32`. Under it, message `0x120` (`L2.eng` group 288) and the
/// army is destroyed.
pub const WITHDRAWAL_SURVIVAL_MEN: i32 = 50;

pub const WITHDRAWAL_WIPE_BELOW: i32 = 11;

/// `Armies_ReturnDefences` (`0x004AD39A`) — the turn-end backstop.
pub const STRAY_COUNTY_LIMIT: u8 = 16;

#[cfg(test)]
mod tests;

