//! ```text
//! Army_BeginSiege     0x004A7CA2  begin_siege     the guard
//! Siege_Link          0x004A7E0A  link            the two back-pointers
//! Siege_Prepare       0x004A7EB5  prepare         clear the records; the AI's order
//! Siege_RecomputeBuildTime 0x004A80DB recompute_build_time  percent and seasons
//! Siege_StartPhase    0x004A82B9  start_phase     break stale links, seed the cursor
//! Siege_ValidateLink  0x004A8426  validate_link
//! Siege_TickPhase     0x004A84BA  tick_phase      the resumable pump
//! Siege_BuildTick     0x004A8507  build_tick      one season of construction
//! Siege_LaunchAssault 0x004A8AAB  assault         the level, the gate, the battle
//! Siege_Break         0x0043B917  break_siege     three callers, all of them lifts
//! Army_PrepareForBattle 0x004AA6CA prepare_besieger / garrison_oil
//! ```
//!
//! Nothing is carried to a siege. [`ENGINE_WORK`] is three man-season costs and
//! [`build_tick`] spends the besieging army's whole strength on them once a
//! season, so **an army of 400 building two towers is ready next season and the
//! same army ordering three rams waits three**. The army does not move while it
//! builds — because *any* successful move
//! order calls [`break_siege`]. `L2.eng` 10/13 *"Lift the siege?"* is a warning,
//! not a veto. `[V]`
//!
//! * **Capturing Counties (pg76)**: *"If a garrisoned castle is present in the
//! county, it must be attacked instead of the county town to gain control of
//!   the county."* That is [`crate::conquest::can_be_entered`] verbatim, and it
//!   promotes the gate from a reading of one `if` to **[V]**.
//!
//! * **Besieged Castles (p.87)**: *"When one of your castles is under siege,
//!   you may only leave the castle to engage the sieging force, and you may not
//!   enter the castle or strengthen the garrison until the siege is lifted."*
//!   [`garrison_is_besieged`] is that rule, and `L2.eng` 289 —
//!   *"…As it is currently under siege !!"* — is the refusal the original
//!   prints.

mod lifecycle;
pub use lifecycle::*;
mod engine;
pub use engine::*;
mod assault;
pub use assault::*;

#[cfg(test)]
use crate::county::{County, MAX_COUNTIES};
#[cfg(test)]
use crate::realm::{Realm, MAX_REALMS};
#[cfg(test)]
use crate::tables::Tables;
#[cfg(test)]
use crate::unit::{UnitKind, Units};

/// The three engine types, in the order their records sit in the unit record
/// (`+0x182`, `+0x188`, `+0x18E` — stride 6) and in `g_siegeEngineWork`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Engine {
    Catapult = 0,
    SiegeTower = 1,
    BatteringRam = 2,
}

/// The three, in record order. A caller drawing the siege-preparation screen
/// walks this: `L2.eng` group 83 lists *"Catapults / Siege towers / Battering
/// rams"* in exactly this order.
pub const ENGINES: [Engine; 3] = [Engine::Catapult, Engine::SiegeTower, Engine::BatteringRam];

impl Engine {
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_index(i: usize) -> Option<Engine> {
        ENGINES.get(i).copied()
    }

    /// The battle troop type this engine becomes when the battle starts —
    /// 7 catapult, 8 siege tower, 9 battering ram. `Army_PrepareForBattle`
    /// copies the counts into `+0x17A`, `+0x17C` and `+0x17E`, which are
    /// `troops[7..=9]`.
    pub fn troop_type(self) -> usize {
        self.index() + 7
    }

    pub fn name(self) -> &'static str {
        match self {
            Engine::Catapult => "Catapults",
            Engine::SiegeTower => "Siege towers",
            Engine::BatteringRam => "Battering rams",
        }
    }
}

/// `g_siegeEngineWork` (`0x004DE440`) — man-seasons one engine of each type
/// costs. Read out of `Lords2.exe`, three `i32` followed by a zero and then
/// string data, which is what fixes the count at three. `[V]`
pub const ENGINE_WORK: [i32; 3] = [200, 200, 400];

/// `[V]`, and new here. The screen's increment handler (`0x0043B681`) picks its
/// ceiling from the hotspot id — `4` for hotspots 0 and 1, `2` for anything
/// else — and refuses to increment at it; the decrement handler
/// (`0x0043B741`) refuses at zero. Nobody had read those two functions, so
/// `docs/armies.md` §4 knew the button existed and not what it was bounded by.
pub const ENGINE_ORDER_CAP: [i16; 3] = [4, 4, 2];

/// The defender's boiling-oil count by **castle level** 0…4 — that is,
/// `castleType - 1`. `Army_PrepareForBattle`'s mode-0 branch, a plain switch on
/// `g_castleLevel`. `[V]`
pub const OIL_BY_CASTLE_LEVEL: [i32; 5] = [1, 2, 3, 4, 6];

/// `Siege_LaunchAssault`'s gate is `level < 3 || engines > 0`, and `L2.eng` 281
/// is the same sentence: *"Your captains advise that you must build some siege
/// engines to besiege this castle."* Level 3 is a stone castle (type 4)
/// palisade, a motte and bailey and a Norman keep can be stormed bare-handed
/// and nothing above them can. `[V]`
pub const ENGINES_REQUIRED_FROM_LEVEL: u8 = 3;

/// One engine type's build record — `+0x182 + e*6`, three `i16`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EngineBuild {
    pub ordered: i16,
    pub percent: i16,
    pub work_done: i16,
}

impl EngineBuild {
    pub fn remaining(&self, engine: Engine) -> i32 {
        if self.ordered < 1 {
            return 0;
        }
        (ENGINE_WORK[engine.index()] * self.ordered as i32 - self.work_done as i32).max(0)
    }

    pub fn is_complete(&self) -> bool {
        self.percent >= 100
    }
}

/// `Army_BeginSiege` is a single four-clause `if` with no else
/// silent in the original — the map click does nothing. Naming the four
/// clauses is what lets the map layer print `L2.eng` 284 / 285 / 289 / 275,
/// which `docs/armies.md` §9 pairs with exactly these conditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiegeRefusal {
    NotAnArmy,
    /// The county has no garrison. `L2.eng` 285 — *"This castle is deserted my
    /// liege. Your enemies await you in the county town."*
    NoGarrison,
    /// County `+0x1C2`, the castle-ruined flag. `L2.eng` 165 names it.
    CastleRuined,
    /// `castleDegraded == 1` with nothing being built. `L2.eng` 284 — *"This
    /// castle is under construction my liege."*
    CastleUnderConstruction,
    /// Somebody else is already besieging it. `L2.eng` 275 — *"…you must join
    /// with or dispose of the existing siegers."*
    AlreadyBesieged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SiegeCursor {
    pub at: usize,
    pub count: u32,
}

/// County `+0x1C3` when the castle is **under construction**. The byte holds
/// 0, 1 or 2 and [`assault_castle_level`] is what reads all three.
pub const CASTLE_DEGRADED_BUILDING: u8 = 1;
/// County `+0x1C3` when the castle has been **knocked down in a siege**, and
/// `+0x1F9` holds the level that is left standing.
///
/// > **Nothing in this workspace writes it, and that is a known hole rather
/// > than an oversight.** The other two values of the byte have reachable
/// > writers now — [`crate::industry::order_castle`] sets 1 and
/// > [`crate::industry::build_tick`] clears it — but 2 belongs to the
/// > end-of-siege bookkeeper, `FUN_004784CA` (`0x004784CA`), which is called
/// > from `Battle_ReturnToCampaign`'s siege arm and does this:
///
/// >
/// > ```c
/// > if (!g_battleIsSiege || (breachDamage == 0 && wallDamage == 0)) return;
/// > county[+0x1E4..+0x1F1] = the battle's breach and approach scores;
/// > if (g_castleLevel < 2) { woodOwed  = woodTotal  = wallDamage * 10; }
/// > else                   { stoneOwed = stoneTotal = wallDamage * 15; }
/// > workLeft = workTotal = breachDamage * 5 + wallDamage * 15;
/// > county.castleLevelLeft = g_castleLevel;
/// > county.castleDegraded  = 2;
/// > county.percent         = 0;
/// > ```
/// >
/// > — adding to the totals when a build was
/// > already under way, so **a wooden castle is repaired in wood and a stone
/// > one in stone**, and a siege on a half-built castle makes the job bigger.
pub const CASTLE_DEGRADED_DAMAGED: u8 = 2;

/// **What a siege left on a castle** — county `+0x1E4` … `+0x1F1`, the six
/// values `Siege_RecordCastleDamage` (`0x004784CA`) writes and `FUN_004787A4`
/// (`0x004787A4`) reads back into the battle when the *next* assault opens on
/// the same castle.
///
/// That pairing is the whole reason these are stored. A
/// besieger thrown off a half-wrecked castle comes back to a half-wrecked
/// castle: the moat it filled is still filled, the walls it opened are still
/// open, and the gate it broke is still broken. Without the round trip the six
/// numbers would be write-only, which is `docs/decisions.md` C27's shape and
/// the exact hole this type exists to avoid re-opening.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SiegeScars {
    /// `+0x1E4` — `DAT_0057A0D8`, **moat cells filled in**. Five man-seasons
    /// of digging each and not a stick of wood; see [`record_castle_damage`].
    pub moat_filled: u16,
    /// `+0x1E6` — `DAT_0056D648`, **rampart cells left hanging by a
    /// collapse**. This is the number the repair is billed in wood or stone.
    pub wall_damage: u16,
    /// `+0x1E8` — `g_siegeBreachScore` as the battle ended.
    pub breach_score: i32,
    /// `+0x1EC` — `g_siegeApproachScore` as the battle ended.
    pub approach_score: i32,
    /// `+0x1F0` — `_DAT_0055307C`, rampart patches down.
    pub ramparts_breached: u8,
    /// `+0x1F1` — `_DAT_00569588`, the gate is open. Set by the
    /// twenty-thousandth hit and by the garrison's own drawbridge.
    pub gate_open: bool,
}

#[cfg(test)]
mod tests;

