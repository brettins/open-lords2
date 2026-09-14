//! **The AI's war** — turn steps 4, 7, 9, 10 and 11, and the six unit missions
//! step 11 dispatches on.
//!
//! [`crate::ai`] is the AI's *economy*: taxes, grants, castles, industry. This
//! is everything the AI does with an army, and until it existed no AI realm
//! had ever raised one.
//!
//! # The constraint that had expired
//!
//! `ai.rs` used to say of these steps that they *"drive armies, merchants,
//! diplomacy and map tiles, none of which `l2-kingdom` owns — reproducing them
//! here would mean inventing a unit model to hang them on"*. The crate owns
//! [`crate::unit`], [`crate::movement`], [`crate::levy`], [`crate::map`] and
//! [`crate::merchant`], so there is nothing left to invent; the sentence was a
//! decision that had become a description of a world that had gone.
//! `docs/plan.md` §2.4.
//!
//! # The five steps
//!
//! | step | address | what it is | here |
//! |---:|---|---|---|
//! | 4 | `0x0049E1BF` | what the realm wants to buy | [`resource_wants`] |
//! | 7 | `0x0049F93D` | three passes: pick the muster county, top up the castle garrisons, hold or abandon the frontier | [`Kingdom::run_ai_armies`] |
//! | 9 | `0x0049F977` | raise the main army and aim it | [`Kingdom::run_ai_raise_army`] |
//! | 10 | `0x004A0015` | send a raiding party at somebody's crops | [`Kingdom::run_ai_raid`] |
//! | 11 | `0x004A5667` | run every army's mission and re-path it | [`Kingdom::run_ai_move_armies`] |
//!
//! # `docs/kingdom.md` §3.2 is wrong about step 10, and it is one digit
//!
//! §3.2 calls step 10 *"create a **type-7 unit** and send it somewhere"*, and
//! `ai.rs` copied that as *"needs the unit **mission** byte"* without noticing
//! the two were the same claim. —
//! [`crate::unit::UnitKind`] has four and `g_unitTickTable` has four handlers.
//! `FUN_004A0015` calls `FUN_004A5003`, which calls `Army_Create`, which
//! spawns a **type-1 army**; the 7 goes into `+0x1A`, the *mission*. `[D]`
//!
//! # The mission byte, and what `docs/armies.md` had closed
//!
//! `docs/armies.md` §1.5 and §8b.2 close unit `+0x1A` as *"the garrison
//! feedback code"*, on the evidence that `Army_GarrisonApply` writes 5 on
//! success and 2 on refusal and that `battle-before.sav`'s garrison carries a
//! 5. Every one of those observations is right and the conclusion is too
//! narrow: `+0x1A` is a **six-value mission enum** that the AI's whole army
//! driver dispatches on, and garrisoning is two of the six values.
//! [`Mission`] is the enum. `[D]`
//!
//! # Five personality fields traced
//!
//! `docs/diplomacy.md` §8.4 lists eleven personality fields that *"hold
//! plausible per-lord values and were not traced"*. Reading these five steps
//! closes three more of them — `+0x70`, `+0x74` and `+0x9C` — and turns two
//! that were named on a guess (`+0x28`, `+0x68`) into claims with a reader.
//! See [`crate::tables::AI_PERSONALITY_GARRISON_MIN_POPULATION`] and its
//! neighbours. It also corrects §8.4's *"a threshold on realm `+0x38`"*: the
//! field is `+0x138`, the realm's **total weapon stock**, and `+0x38` is
//! inside the army-name counters.
//!
//! # Two seams, named
//!
//! * **The evacuation.** Step 7's third pass ships a written-off county's
//!   grain and herd to the muster county with `Transport_Spawn`, or sells them
//!   where the county has a merchant stall. This crate has neither
//!   `Transport_Deliver` nor a county stall field, and a transport that can
//!   never unload would **delete** the county's food from the game — a worse
//!   reproduction than not evacuating. So the pass returns what it wanted to
//!   move, as [`Evacuation`], and moves nothing. Everything else in the pass —
//!   the levy, the punitive tax rate, the industry share — happens.
//! * **The garrison eviction's battle.** `FUN_00437535` starts a battle when
//!   the evicted garrison had a besieger. [`Kingdom::run_ai_move_armies`]
//! reports the pair instead of starting one, for the same reason
//!   [`crate::ai::taunt`] returns its letters.
//!
//! # Determinism
//!
//! Every scan here is an ascending index or an ascending tile offset with a
//! **strict** `<` comparison, so a tie goes to the lowest id — which is the
//! original's own tie-break and is what `docs/netcode.md` §5 requires anyway.
//! Nothing draws a random number: the AI's war is entirely deterministic in
//! the shipped game, which is worth knowing before anyone reaches for the
//! generator to make it less predictable.

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
    /// **3 — hold the home county.** Intercept anything hostile standing in
    /// [`crate::unit::Unit::home_county`] within 30 tiles, else walk back to it.
    /// `FUN_004A5F0A`.
    pub const HOLD_HOME: u8 = 3;
    /// **4 — go and join a garrison.** Walk to a friendly castle with room and
    /// step onto its tile. `FUN_004A6270`.
    pub const JOIN_GARRISON: u8 = 4;
    /// **5 — in garrison.** `Army_GarrisonApply` writes this on success and
    /// [`Mission::SEEK_ENEMY`] on refusal, which is the pair `docs/armies.md`
    /// §8b.2 read as the whole meaning of the byte. `FUN_004A60B9`.
    pub const GARRISON: u8 = 5;
    /// **6 — relieve an ally.** March on the county the ally asked about,
    /// while the ally still owns it and there are still enemies in it.
    /// `FUN_004A599D`.
    pub const ASSIST_ALLY: u8 = 6;
    /// **7 — raid.** Walk at the nearest **standing crop** in the target
    /// county, which is what makes this a raid: `Unit_TrampleTile` fires on
    /// every field an army crosses in a county it does not own.
    /// `FUN_004A58F4`.
    pub const RAID: u8 = 7;
}

/// The intercept radius of [`Mission::SEEK_ENEMY`] — Chebyshev `< 6`.
pub const SEEK_ENEMY_RADIUS: i32 = 6;
/// The intercept radius of [`Mission::HOLD_HOME`] — Chebyshev `< 31`. Five
/// times the attacker's, which is what makes a defender leave its post for
/// something it would otherwise ignore.
pub const HOLD_HOME_RADIUS: i32 = 31;
/// The radius [`Mission::ASSIST_ALLY`] closes on the enemy in the ally's
/// county — Chebyshev `< 21`.
pub const ASSIST_ALLY_RADIUS: i32 = 21;

/// The distance both county choosers start from, so a county scoring `0x80` or
/// worse is never chosen at all.
pub const TARGET_SCORE_CEILING: i32 = 0x80;
/// What a **defended** castle adds to a county's target score. It is 40 on a
/// Chebyshev distance across a 64-tile map, so it is very nearly a veto.
pub const DEFENDED_CASTLE_PENALTY: i32 = 40;

/// The smallest army [`Kingdom::run_ai_raise_army`] will divert instead of
/// raising a new one — `FUN_004A0917`'s third argument.
pub const DIVERT_MIN_MEN: i32 = 200;
/// What standing in `FUN_004A0917`'s score being in the wanted county is worth.
pub const DIVERT_IN_COUNTY: i32 = 300;
/// …and being next door to it.
pub const DIVERT_NEXT_DOOR: i32 = 100;

/// The muster county must have more than this many people, and more than
/// [`RAID_MIN_HAPPINESS`] happiness, before a raid is sent from it.
pub const RAID_MIN_POPULATION: i32 = 399;
/// See [`RAID_MIN_POPULATION`].
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

/// A castle's garrison must be short by more than this before the AI raises
/// men for it, and the shortfall must be under
/// [`GARRISON_GAP_ABANDON_PCT`] of the county's population.
pub const GARRISON_GAP_MIN: i32 = 19;
/// A shortfall this large a share of the county is not attempted at all.
pub const GARRISON_GAP_ABANDON_PCT: i32 = 80;
/// The realm needs more than this many bows *plus* crossbows before it will
/// garrison anything — `weapons[4] + weapons[0] > 0x13`.
pub const GARRISON_MIN_MISSILE_STOCK: i32 = 19;

/// `FUN_0049F431`'s two thresholds: an enemy force in one of the realm's
/// counties is only *noticed* above this…
pub const FRONTIER_ENEMY_MIN: i32 = 99;
/// …and only acted on when the realm's own men there fall this far behind it.
pub const FRONTIER_DEFICIT: i32 = 100;
/// A county below this population is written off outright — the branch that
/// forces the comparison threshold to zero.
pub const FRONTIER_MIN_POPULATION: i32 = 200;
/// The share of the county's people either branch of the frontier pass levies.
pub const FRONTIER_LEVY_PCT: i32 = 50;
/// The population floor `FUN_0049F431` passes `FUN_004A50AE`. **Not
/// [`crate::levy::DEFENCE_MIN_POPULATION`]**, which is the 40 the *invasion*
/// path passes: the same function, a different argument, and hardcoding it was
/// this crate's simplification.
pub const FRONTIER_LEVY_MIN_POPULATION: i32 = 100;

/// The order `FUN_004A5389` hands weapons out in: bows, crossbows, swords,
/// maces, mail — **and never pikes.**
///
/// This is a *greedy* fill and not [`LevyBasket::auto_equip`]'s round-robin:
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
