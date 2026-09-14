//! The kingdom — the whole state
//! [`SEASON_PIPELINE`] over it.
//!
//! `Season_Advance` (`0x00448440`) calls 29 functions in a fixed order, and
//! `docs/kingdom.md` §3.4 is explicit that **the order is the rule**. So the
//! driver here is a loop over an array:
//! [`Kingdom::advance_season`] walks [`SEASON_PIPELINE`] and records what it
//! ran
//! head.
//!
//! # The clock
//!
//! ```c
//! int ended = g_seasonNext;
//! g_turnCount++;
//! g_seasonPrev = g_season;
//! g_season     = g_seasonNext;      /* the season now beginning */
//! g_seasonNext = ended + 1; if (g_seasonNext > 4) g_seasonNext = 1;
//! if (ended == 4) { g_year = g_yearNext; g_yearNext++; }
//! ```
//!
//! The subtlety that makes every seasonal rule read correctly: `g_season` is
//! set to the season *about to begin* **before** the economy runs. So
//! guarded by `g_season == 1` fires at the **end of Winter** — which is exactly
//! what the game's own help says about sowing grain.

mod history;
pub use history::*;
mod pipeline;
pub use pipeline::*;
mod economy;
pub use economy::*;
mod state;
pub use state::*;
mod tests;
pub use tests::*;

use crate::ai;
use crate::county::{County, MAX_COUNTIES, MAX_COUNTY_ID};
use crate::event;
use crate::happiness;
use crate::health;
use crate::industry;
use crate::land;
use crate::phase::{Pass, Phase, PhaseTick, TurnMachine, SEASON_PIPELINE};
use crate::population;
use crate::ration;
use crate::realm::{Realm, MAX_REALMS};
use crate::report::{Message, SeasonReport};
use crate::tables::{Commodity, Season, Tables};
use crate::tax;
use crate::unrest;
use crate::weather;
use l2_net::Pcg32;

/// **The game options a new game is started with**, as the setup screen's twelve
/// drop-downs leave them.
///
/// Six of the twelve are rules that live for the length of a game and so live
/// here; the other six are *starting conditions* — how much gold, which castle,
/// what garrison — which are spent once when the world is built and are
/// `l2_game::setup`'s, not this crate's. `docs/kingdom.md` §10 has the whole
/// table and which global each one is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// `g_optDifficulty`, 0..=2 in the England turn-one fixture (0), and 0..=3 in the
    /// AI grant tables. Only the AI's wages and grants read it.
    pub difficulty: u8,
/// `g_optAdvancedFarming`. Off in the England turn-one fixture, so every
    /// county there is Cloudy with zero fertility.
    pub advanced_farming: bool,
    /// `g_optArmiesEat`. Off in the England turn-one fixture.
    pub armies_eat: bool,
    /// `g_optFightHumansOnly` (`0x0053F284`) **as the option byte**, which the
    /// original stores *inverted*: it is 0 when the option displays *Yes*. Kept
/// as the byte so that the sense cannot drift from
    /// the binary's — [`crate::battle::settlement`] tests it against 0 exactly
    /// as `FUN_004A6A30` does. [`crate::battle::FIGHT_HUMANS_ONLY_DEFAULT`] is
    /// the game's default.
    pub fight_humans_only_byte: u8,
    /// `g_optExploration` (`0x0053F264`), the *Exploration* drop-down — **the
    /// fog of war**.
    ///
    /// **Read by no rule in this crate, and that is the original's shape rather
    /// than a gap.** `L2.eng` group 218 index 3 says what it does: *"When
    /// Exploration is turned on, the world outside your county is blacked out.
    /// It is gradually revealed as your armies move through and conquer new
    /// counties."* Its readers in `Lords2.exe` are seven map painters and the
    /// options page, and nothing else — no AI step, no input arm, no pass — so
    /// here it is read by [`crate::explore::hides`] and the painters that call
    /// it. The *seen bits* are simulation-written and live in
    /// [`Campaign::explored`]; they are kept whether this is on or off, exactly
    /// as the original keeps them.
    pub exploration: bool,
    /// `g_optTimeLimit` (`0x0053F26C`) — **seconds**, 0 for no limit.
    ///
    /// Not the *Time limit* drop-down's index: `Setup_CommitOptions`
    /// (`0x00499DC3`) puts the index through `g_timeLimitSeconds`
    /// (`0x004DBBF8`) = `30, 60, 120, 240, 480, 600, 0`, so what a game runs on
    /// is a duration and the seven strings are its labels. Carried for the same
    /// reason as [`Options::exploration`]:
    /// is a setting the game was started with. `Setup_StartGame` copies it into
    /// the turn timer as the game begins.
    pub time_limit: i32,
    /// **Which of the original's defects this game reproduces — ours.**
    ///
    /// There is **no `g_optQuirks`**; the original has no such setting and
    /// could not, because to it these are not settings at all. This field is a
/// declared divergence, and it is here on [`Options`]
    /// [`Tables`] for the reason `docs/bugs.md` §6.3 works out and
    /// `docs/decisions.md` C62 records:
    ///
    /// * **not on [`Tables`]** — `save::ruleset_fingerprint` is hashed into the
    /// save *header* and `save::decode` refuses a mismatch, so
    ///   would invalidate every existing save on the day it was added, and
    ///   would frame a quirk as a *rule*, which it is not;
    /// * **on `Options`** — `Options` is already in the save *body* and
    ///   therefore already inside the per-tick lockstep digest, since
    ///   [`save::checksum`] is `Canonical::hash_of(kingdom)`. That is exactly
    ///   where something that changes what the simulation computes belongs: two
    ///   peers whose quirk sets differ disagree at the first tick a quirk
    /// touches
    ///
/// Sound, animations and the scroll
    /// speed are — they live in `l2_game::prefs`, are never encoded here and
    /// never reach the digest. These change the world.
    ///
    /// [`Tables`]: crate::tables::Tables
    /// [`save::checksum`]: crate::save::checksum
    pub quirks: l2_net::Quirks,
}

impl Default for Options {
    fn default() -> Self {
        // The shipped lastturn.sav's settings (docs/kingdom.md §9).
        Options {
            difficulty: 0,
            advanced_farming: false,
            armies_eat: false,
            fight_humans_only_byte: crate::battle::FIGHT_HUMANS_ONLY_DEFAULT,
            exploration: false,
            time_limit: 0,
            // **Faithful by default**
            // the original is the oracle, the bugs are load-bearing on a balance
            // nobody has measured
            // corpus of saves and replays is recorded under. It is also the
            // integer zero, so this line costs nothing.
            quirks: l2_net::Quirks::FAITHFUL,
        }
    }
}

/// A whole kingdom.
///
/// The two arrays are fixed size and 1-based, as the original's are:
/// county 0 is never a county and realm 0 is never a realm. Everything is
/// walked by index, so two peers running the same commands reach bit-identical
/// state (`docs/netcode.md` §3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kingdom {
    pub counties: [County; MAX_COUNTIES],
    pub realms: [Realm; MAX_REALMS],
    /// `g_countyCount` — 14 on the England map. Records above it are all zero.
    pub county_count: usize,
    /// `g_season` — the season now under way, 1..=4.
    pub season: u8,
    /// `g_seasonNext`.
    pub season_next: u8,
    /// `g_seasonPrev`.
    pub season_prev: u8,
    /// `g_year` and `g_yearNext`.
    pub year: i32,
    pub year_next: i32,
    /// `g_turnCount`.
    pub turn_count: u32,
    /// `g_turnPhase` and `g_turnPhaseStep`.
    pub turn: TurnMachine,
    pub options: Options,
    /// The simulation's generator. Frozen in-tree in `l2-net`; three draws a
    /// season — one for the event deck's starting slot and two for the weather.
    pub rng: Pcg32,
    /// The history ring `Season_Advance`'s second-to-last pass writes.
    /// See [`History`].
    pub history: History,
    /// `g_weatherCounty` (`0x00554020`) — the county that got last season's
    /// local weather swing, which is the fallback when this season's masked
    /// draw lands outside the map. See [`weather::chosen_county`].
    pub weather_county: usize,
    /// **Everything that moves on the map** — the units, the ground they stand
    /// on, the mercenary bands and the realms' army-name counters.
    ///
/// It is *inside* the kingdom because it is
    /// simulation state in exactly the sense `docs/netcode.md` §5 means: two
    /// lockstep peers have to agree about where an army stands and which tiles
    /// it has ruined, the tick checksum has to cover it, and a save that
    /// dropped it would keep the economy and lose the war. See [`Campaign`].
    pub campaign: Campaign,
    /// **The diplomatic state that does not live in a realm record** — the
    /// five-slot inbox per realm, the outstanding pay-for-help price
    /// dice the three bargaining replies roll.
    ///
    /// It is inside the kingdom for the reason [`Campaign`] is: two lockstep
    /// peers have to agree about who has written to whom, and a save that
    /// dropped an unanswered letter would resume a different game. See
    /// [`crate::diplomacy`].
    pub diplomacy: crate::diplomacy::Diplomacy,
    /// **The ruleset this kingdom runs on.**
    ///
    /// Fixed for the life of the kingdom, like `l2_sim::Battle`'s troop
    /// table: every rule function in this crate takes `&Tables` and reads it
/// so whatever table built this kingdom is
    /// the table its whole economy runs on.
    ///
    /// [`Tables::DEFAULT`] is what the original ships with. Anything else
    /// arrives from `l2-mods`, which this crate knows nothing about — it takes
    /// plain data and never learns where the data came from.
    pub tables: Tables,
}

/// `FUN_004AE7DD` — the history ring `docs/kingdom.md` §3.4 names and nothing
/// more.
///
/// **400 seasons x 16 counties x `{population, happiness}`.** The length is not
/// inferred: save block 10 is `0x0056D8C0` for 51,200 bytes and
/// `400 * 16 * 8` is exactly 51,200. Three globals go with it — the write head
/// (`0x0055300C`), the oldest entry (`0x00568DA8`) and the number of entries
/// held (`0x00553F30`, saturating at 400) — and each is its own four-byte save
/// block, so the whole structure is confirmed by the save layout
/// only by the code.
///
/// Once the ring is full the head keeps advancing and the tail follows it
/// game longer than 400 seasons — a hundred years — silently forgets its
/// beginning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct History {
    /// `[season][county - 1]`, oldest at the tail.
    ///
/// Crate-visible so [`crate::save`] can write the ring
    /// out and read it back. Still not `pub`: the invariant tying `head`,
    /// `tail` and `len` together belongs to this module, and
    /// `save::LoadError::CorruptHistory` is what enforces it on the way in.
    pub(crate) entries: Vec<[HistoryEntry; crate::tables::HISTORY_COUNTIES]>,
    pub(crate) head: usize,
    pub(crate) tail: usize,
    pub(crate) len: usize,
}

/// One county's line in one season of the ring: `{i32 population, i8
/// happiness}` in eight bytes, four of them padding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HistoryEntry {
    pub population: i32,
    pub happiness: i8,
}

impl Default for History {
    fn default() -> Self {
        History::new()
    }
}

/// The campaign-map half of the state — `docs/armies.md`.
///
/// Four things that only make sense together: the unit array, the map they
/// stand on, the mercenary bands walking it
/// counters. They are one struct because every rule in [`crate::movement`],
/// [`crate::levy`] and [`crate::conquest`] needs two or three of them at once,
/// and because grouping them keeps [`Kingdom`] readable as *economy plus war*
///
///
/// A default [`Campaign`] is an empty map with no units and no bands, which is
/// what a kingdom built without a scenario has. Nothing here is optional: an
/// empty map is a real value, not a missing one, and a rule that has to ask
/// whether the map exists is a rule with two behaviours.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Campaign {
    /// `g_units` — 151 slots, 1..=150 usable.
    pub units: crate::unit::Units,
    /// The three tile planes the simulation reads and writes.
    pub map: crate::map::CampaignMap,
    /// `g_mercBands` and how many of them this map supports.
    pub mercenaries: crate::mercenary::MercenaryBands,
    /// Realm `+0x2D` — twenty-four name counters a lord.
    pub names: crate::unit::ArmyNames,
    /// `g_merchantRoutes` — the six trade routes the merchants walk.
    /// See [`crate::merchant`].
    pub routes: crate::merchant::MerchantRoutes,
    /// `DAT_004E59DC` — the peasant mobs' **shared** destination cursor.
    ///
    /// One counter for the whole map, not one per mob: `FUN_004AC5BA` bumps it
    /// and hands the result to whichever mob asked. Simulation state, and a
    /// small one that would be easy to leave out of a save and never notice
    /// until two lockstep peers sent their mobs to different counties.
    pub mob_cursor: usize,
    /// **What each realm has seen of the map** — the fog of war, tile record
    /// `+2` bit `0x20` in the original. See [`crate::explore`].
    ///
    /// Here and not beside the map's planes because nothing on the map reads
    /// it: no cost, no step and no AI decision looks at a seen bit. It is in
    /// the campaign because the campaign's rules *write* it — an army walking,
    /// an army raised, a county taken — and because nothing can rebuild it, so
    /// the save has to carry it.
    pub explored: crate::explore::Explored,
}

impl Default for Campaign {
    fn default() -> Self {
        Campaign::new()
    }
}

impl Campaign {
    pub fn new() -> Campaign {
        Campaign {
            units: crate::unit::Units::new(),
            map: crate::map::CampaignMap::empty(),
            mercenaries: crate::mercenary::MercenaryBands::none(),
            names: crate::unit::ArmyNames::new(),
            routes: crate::merchant::MerchantRoutes::none(),
            mob_cursor: 0,
            // `Map_InitScenario` clears every seen bit (`FUN_0046DF51`) straight
            // after the planes are loaded.
            explored: crate::explore::Explored::new(),
        }
    }
}

