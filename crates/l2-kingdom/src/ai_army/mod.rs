//! | step | address | what it is | here |
//! |---:|---|---|---|
//! | 4 | `0x0049E1BF` | what the realm wants to buy | [`resource_wants`] |
//! | 7 | `0x0049F93D` | three passes: pick the muster county, top up the castle garrisons, hold or abandon the frontier | [`Kingdom::run_ai_armies`] |
//! | 9 | `0x0049F977` | raise the main army and aim it | [`Kingdom::run_ai_raise_army`] |
//! | 10 | `0x004A0015` | send a raiding party at somebody's crops | [`Kingdom::run_ai_raid`] |
//! | 11 | `0x004A5667` | run every army's mission and re-path it | [`Kingdom::run_ai_move_armies`] |
//!
//! `FUN_004A0015` calls `FUN_004A5003`, which calls `Army_Create`, which
//! spawns a **type-1 army**; the 7 goes into `+0x1A`, the *mission*. `[D]`
//!
//! `docs/armies.md` §1.5 and §8b.2 close unit `+0x1A` as *"the garrison
//! feedback code"*, on the evidence that `Army_GarrisonApply` writes 5 on
//! success and 2 on refusal and that `battle-before.sav`'s garrison carries a
//! 5. Every one of those observations is right and the conclusion is too
//! narrow: `+0x1A` is a **six-value mission enum** that the AI's whole army
//! driver dispatches on, and garrisoning is two of the six values.
//!
//! [`Mission`] is the enum. `[D]`
//!
//! `docs/diplomacy.md` §8.4 lists eleven personality fields that *"hold
//! plausible per-lord values and were not traced"*. Reading these five steps
//! closes three more of them — `+0x70`, `+0x74` and `+0x9C` — and turns two
//! that were named on a guess (`+0x28`, `+0x68`) into claims with a reader.
//!
//! See [`crate::tables::AI_PERSONALITY_GARRISON_MIN_POPULATION`] and its
//! neighbours. It also corrects §8.4's *"a threshold on realm `+0x38`"*: the
//! field is `+0x138`, the realm's **total weapon stock**, and `+0x38` is
//! inside the army-name counters.
//!
//! * **The garrison eviction's battle.** `FUN_00437535` starts a battle when
//!   the evicted garrison had a besieger. [`Kingdom::run_ai_move_armies`]
//! reports the pair instead of starting one, for the same reason
//!   [`crate::ai::taunt`] returns its letters.

mod wants;
pub use wants::*;
mod muster;
pub use muster::*;
mod raid;
pub use raid::*;
mod aim;
pub use aim::*;
mod army;
pub use army::*;

use crate::county::{County, MAX_COUNTIES};
use crate::kingdom::Kingdom;
use crate::levy::{self, LevyBasket, Muster};
use crate::map::{flags, CampaignMap, MAP_DIM};
use crate::math::pct;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use crate::unit::{TroopType, UnitKind, Units};

/// Unit `+0x1A` — **what an army is currently trying to do.**
///
/// `FUN_004A57AC` dispatches on it and rewrites **any** value it does not
/// recognise to [`Mission::SEEK_ENEMY`], returning 0 that turn — so a unit
/// with mission 0 or 1 loses one turn and then behaves as an attacker. That
/// normalisation is why [`crate::unit::Unit::mission`] is a raw byte and not a
/// Rust enum.
pub struct Mission;

impl Mission {
    /// **2 — seek the enemy.** The default and the one every unrecognised
    /// value becomes: intercept an enemy army within 5 tiles, else march on a
    /// hostile county. `FUN_004A5B1F`.
    pub const SEEK_ENEMY: u8 = 2;
    /// `FUN_004A5F0A`.
    pub const HOLD_HOME: u8 = 3;
    /// **4 — go and join a garrison.** Walk to a friendly castle with room and
    /// step onto its tile. `FUN_004A6270`.
    pub const JOIN_GARRISON: u8 = 4;
    /// **5 — in garrison.** `Army_GarrisonApply` writes this on success and
    /// [`Mission::SEEK_ENEMY`] on refusal, which is the pair `docs/armies.md`
    /// §8b.2 read as the whole meaning of the byte. `FUN_004A60B9`.
    pub const GARRISON: u8 = 5;
    /// `FUN_004A599D`.
    pub const ASSIST_ALLY: u8 = 6;
    /// `FUN_004A58F4`.
    pub const RAID: u8 = 7;
}

pub const SEEK_ENEMY_RADIUS: i32 = 6;
pub const HOLD_HOME_RADIUS: i32 = 31;
pub const ASSIST_ALLY_RADIUS: i32 = 21;

pub const TARGET_SCORE_CEILING: i32 = 0x80;
pub const DEFENDED_CASTLE_PENALTY: i32 = 40;

/// The smallest army [`Kingdom::run_ai_raise_army`] will divert instead of
/// raising a new one — `FUN_004A0917`'s third argument.
pub const DIVERT_MIN_MEN: i32 = 200;
/// What standing in `FUN_004A0917`'s score being in the wanted county is worth.
pub const DIVERT_IN_COUNTY: i32 = 300;
pub const DIVERT_NEXT_DOOR: i32 = 100;

pub const RAID_MIN_POPULATION: i32 = 399;
pub const RAID_MIN_HAPPINESS: i32 = 24;
/// How many men a raiding party is — `PctOf(50, population)` men, which is
/// **fifty men expressed as a percentage of the county** and then taken back
/// out of it, so integer truncation makes it fifty-ish whatever the county's
/// size. `FUN_004A5003`.
pub const RAID_MEN: i32 = 50;

/// A rival must be **strictly below** this standing before the AI will raid
/// it, and the lowest such standing wins. `FUN_004A0309` seeds its running
/// best with the threshold itself, which is what makes it strict.
pub const RAID_STANDING_THRESHOLD: i8 = -10;

/// The share of the map the leading realm must hold before the others treat it
/// as *the* threat — `FUN_004A01EA`'s `0x28`.
pub const THREAT_SHARE_PCT: i32 = 40;

pub const GARRISON_GAP_MIN: i32 = 19;
pub const GARRISON_GAP_ABANDON_PCT: i32 = 80;
pub const GARRISON_MIN_MISSILE_STOCK: i32 = 19;

/// `FUN_0049F431`'s two thresholds: an enemy force in one of the realm's
/// counties is only *noticed* above this…
pub const FRONTIER_ENEMY_MIN: i32 = 99;
pub const FRONTIER_DEFICIT: i32 = 100;
pub const FRONTIER_MIN_POPULATION: i32 = 200;
pub const FRONTIER_LEVY_PCT: i32 = 50;
/// The population floor `FUN_0049F431` passes `FUN_004A50AE`. **Not
/// [`crate::levy::DEFENCE_MIN_POPULATION`]**, which is the 40 the *invasion*
/// path passes: the same function, a different argument, and hardcoding it was
/// this crate's simplification.
pub const FRONTIER_LEVY_MIN_POPULATION: i32 = 100;

/// The order `FUN_004A5389` hands weapons out in: bows, crossbows, swords,
/// maces, mail — **and never pikes.**
///
/// each type in turn arms as many still-unarmed levies as the stock allows, so
/// a realm with a thousand bows sends a garrison of pure archers. Pikemen are
/// the one equipped type the pass skips, which reads as doctrine — a garrison
/// wants missiles and a pike is a field weapon against cavalry — and is stated
/// as an observation. `[D]`
pub const GARRISON_EQUIP_ORDER: [TroopType; 5] = [
    TroopType::Archer,
    TroopType::Crossbowman,
    TroopType::Swordsman,
    TroopType::Maceman,
    TroopType::Knight,
];
