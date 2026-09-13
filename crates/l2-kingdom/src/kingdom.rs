//! The kingdom — the whole state, and the driver that walks
//! [`SEASON_PIPELINE`] over it.
//!
//! `Season_Advance` (`0x00448440`) calls 29 functions in a fixed order, and
//! `docs/kingdom.md` §3.4 is explicit that **the order is the rule**. So the
//! driver here is a loop over an array:
//! [`Kingdom::advance_season`] walks [`SEASON_PIPELINE`] and records what it
//! ran, and the ordering assertions live in tests
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
    /// touches, and the desync detector names the `options` section.
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
            // **Faithful by default**, and the argument is `docs/bugs.md` §6.5's:
            // the original is the oracle, the bugs are load-bearing on a balance
            // nobody has measured, and the default becomes the value the whole
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
    /// five-slot inbox per realm, the outstanding pay-for-help price, and the
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
/// Once the ring is full the head keeps advancing and the tail follows it, so a
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

impl History {
    pub fn new() -> History {
        History {
            entries: vec![
                [HistoryEntry::default(); crate::tables::HISTORY_COUNTIES];
                crate::tables::HISTORY_SEASONS
            ],
            head: 0,
            tail: 0,
            len: 0,
        }
    }

    /// How many seasons are held, up to [`crate::tables::HISTORY_SEASONS`].
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Write this season's line for counties 1..=16 and advance the ring.
    ///
    /// The original's loop is `for (c = 1; c < 0x11; c++)` — **counties 1..=16
    /// unconditionally**, not `1..=g_countyCount` — so the slots above the map's
    /// county count are filled with whatever the unused records hold, which is
    /// zero. Reproduced, because it is what a reader of the ring has to expect.
    pub fn record(&mut self, counties: &[County]) {
        let slot = &mut self.entries[self.head];
        for c in 1..=crate::tables::HISTORY_COUNTIES {
            let county = &counties[c];
            slot[c - 1] = HistoryEntry {
                population: county.population,
                // The original stores a signed byte. Happiness is clamped
                // 0..=100 by `Happiness_UpdateAll`, so the narrowing is safe
// here, but it is done so a caller cannot
                // come to depend on a range the original does not have.
                happiness: county.happiness as i8,
            };
        }
        self.len += 1;
        if self.len > crate::tables::HISTORY_SEASONS {
            self.len = crate::tables::HISTORY_SEASONS;
            self.tail += 1;
            if self.tail >= crate::tables::HISTORY_SEASONS {
                self.tail = 0;
            }
        }
        self.head += 1;
        if self.head >= crate::tables::HISTORY_SEASONS {
            self.head = 0;
        }
    }

    /// One county's history, oldest season first. `county` is 1-based.
    pub fn county(&self, county: usize) -> Vec<HistoryEntry> {
        if county < 1 || county > crate::tables::HISTORY_COUNTIES {
            return Vec::new();
        }
        (0..self.len)
            .map(|i| {
                let slot = (self.tail + i) % crate::tables::HISTORY_SEASONS;
                self.entries[slot][county - 1]
            })
            .collect()
    }

    /// The most recent season recorded, or `None` before the first.
    pub fn latest(&self) -> Option<&[HistoryEntry; crate::tables::HISTORY_COUNTIES]> {
        if self.len == 0 {
            return None;
        }
        let slot =
            (self.head + crate::tables::HISTORY_SEASONS - 1) % crate::tables::HISTORY_SEASONS;
        Some(&self.entries[slot])
    }
}

/// The campaign-map half of the state — `docs/armies.md`.
///
/// Four things that only make sense together: the unit array, the map they
/// stand on, the mercenary bands walking it, and the per-realm army-name
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

impl Kingdom {
    /// An empty kingdom on the stock ruleset, before any map is loaded.
    pub fn new(seed: u64) -> Kingdom {
        Kingdom::with_tables(seed, Tables::DEFAULT)
    }

    /// An empty kingdom on a supplied ruleset — how a mod reaches the economy.
    ///
    /// The same shape `l2_sim::Battle::with_troops` uses for the battle side,
    /// and for the same reason: the rules are a *value* the simulation is
    /// handed, so nothing in this crate can tell whether they came from
    /// [`Tables::DEFAULT`] or out of somebody's `kingdom.toml`.
    ///
    /// ```
    /// # use l2_kingdom::{Kingdom, tables::Tables};
    /// let mut rules = Tables::DEFAULT;
    /// rules.grain.yield_per_sack = 24;   // twice the harvest
    /// let kingdom = Kingdom::with_tables(1, rules);
    /// assert_eq!(kingdom.tables.grain.yield_per_sack, 24);
    /// assert_eq!(Kingdom::new(1).tables, Tables::DEFAULT);
    /// ```
    pub fn with_tables(seed: u64, tables: Tables) -> Kingdom {
        Kingdom {
            tables,
            counties: core::array::from_fn(|_| County::new()),
            realms: core::array::from_fn(|_| Realm::new()),
            county_count: 0,
            season: 0,
            season_next: 1,
            season_prev: 0,
            year: 0,
            year_next: 0,
            turn_count: 0,
            turn: TurnMachine::new(),
            options: Options::default(),
            rng: Pcg32::from_seed(seed),
            history: History::new(),
            weather_county: 1,
            campaign: Campaign::new(),
            diplomacy: crate::diplomacy::Diplomacy::new(seed),
        }
    }

    /// `Game_NewGame` (`0x00497CED`)'s clock constants, then one
    /// [`Kingdom::advance_season`].
    ///
    /// **A new game starts in Winter 1268.** The shipped `lastturn.sav` reads
    /// back `g_season = 4`, `g_year = 1268`, `g_turnCount = 1`, and this is the
    /// arithmetic that produces them.
    ///
    /// **Every pass but phase 7's three.** `Game_NewGame` (`0x00497CED`)
    /// runs `Mercenary_Init(); … Season_Advance();` and never
    /// `Mercenary_AdvanceAll`, which is `Turn_Tick`'s phase-7 call and not
    /// `Season_Advance`'s. It did not matter while no new game had bands in
    /// play; now that `Scenario::from_map` seeds them, running the walk here
    /// would have the Saxon band offering itself in county 14 on turn one. The
    /// save settles it `[V]`: in `england-turn1.sav` every one of the twelve
    /// bands still has its countdown equal to its reload and its walk on its
    /// start county, which is `Mercenary_Init`'s state with no walk after it.
    pub fn start_new_game(&mut self) -> SeasonReport {
        self.season = 3;
        self.season_next = 4;
        self.year = 1267;
        self.year_next = 1268;
        self.turn_count = 0;
        let mut report = SeasonReport::new();
        for pass in SEASON_PIPELINE {
            // **No phase-7 pass.** `Game_NewGame` (`0x00497CED`) calls
            // `Season_Advance` and `Score_RankRealms` and neither
            // `Mercenary_AdvanceAll`, `Units_ResetMoves` (`0x004651B9`) nor
            // `Diplo_ReconcileAlliances` (`0x004A1847`) — those three are
            // `Turn_Tick`'s phase-7 arm, which a new game has not reached.
            // `[V]` from its own call list, and `Season_Advance`'s
            // (`0x00448440`) does not hold them either. End Turn still runs
            // all three: they are skipped here, not removed from the pipeline,
            // so no pass index moves.
            if matches!(
                pass,
                Pass::MercenaryAdvance | Pass::UnitsResetMoves | Pass::ReconcileAlliances
            ) {
                continue;
            }
            self.run_pass(pass, &mut report);
            report.passes.push(pass);
        }
        report
    }

    /// **`FUN_0044BA35`** — realm `+0x15C`, the court's *"Tax revenues
    /// expected"* (`L2.eng` 70/8): the sum of [`County::tax_shown`] over the
    /// counties a realm owns.
    ///
    /// The original caches it in the realm record at the tail of
    /// `Tax_RecomputePreview`; this recomputes it, and `docs/stored-fields.json`
    /// holds the recomputation to the cached bytes in every realm of every save
    /// (twenty-three of them non-zero). **One window where the two can differ**,
    /// `[D]`: `County_ChangeOwner` does not re-run the preview, so after a
    /// mid-turn capture the original's court goes on showing the old sum until
    /// the season's refresh, and this shows the new one at once.
    ///
    /// The original skips a county with owner 0; this asks for a match on
    /// `realm`, and realm 0 is not a realm.
    pub fn tax_expected(&self, realm: u8) -> i32 {
        if realm == 0 {
            return 0;
        }
        self.county_ids()
            .filter(|&id| self.counties[id].owner == realm)
            .map(|id| self.counties[id].tax_shown)
            .sum()
    }

    /// How many counties this kingdom has, bounded by the array.
    ///
    /// Returns `false` and changes nothing if asked for more than the original
    /// can hold. The bound is real: `g_counties` is 17 records with index 0
    /// unused.
    pub fn set_county_count(&mut self, count: usize) -> bool {
        if count > MAX_COUNTY_ID as usize {
            return false;
        }
        self.county_count = count;
        true
    }

    /// `g_season` as an enum. `None` only before the first season has begun.
    pub fn season(&self) -> Option<Season> {
        Season::from_index(self.season)
    }

    /// Whether a realm index is a person. Realm 0 is not a realm, so an
    /// unowned county's "owner" is never human.
    pub fn owner_is_human(&self, owner: u8) -> bool {
        owner != 0 && (owner as usize) < MAX_REALMS && self.realms[owner as usize].is_human
    }

    /// The county ids, 1..=`county_count`. Always ascending, never sorted, and
    /// never derived from a hash.
    pub fn county_ids(&self) -> core::ops::RangeInclusive<usize> {
        1..=self.county_count
    }

    /// One `Turn_Tick`. When the machine enters phase 7 the season advances;
    /// otherwise nothing economic happens.
    ///
    /// `settled` answers the current phase's wait condition — see
    /// [`TurnMachine::tick`]. Phases 2, 3, 5 and 6 move units, which are not
    /// this crate's, so the caller answers for them.
    pub fn tick(&mut self, settled: bool) -> (PhaseTick, Option<SeasonReport>) {
        let settled = if self.turn.phase == Phase::PlayersTurn {
            ai::all_realms_done(&self.realms)
        } else {
            settled
        };
        let tick = self.turn.tick(settled);
        let report = if tick.phase == Phase::SeasonEnd && tick.started {
            Some(self.advance_season())
        } else {
            None
        };
        if tick.advanced_to == Some(Phase::PlayersTurn) {
            ai::begin_turn(&mut self.realms);
        }
        (tick, report)
    }

    /// `Season_Advance` — walk [`SEASON_PIPELINE`] in order.
    pub fn advance_season(&mut self) -> SeasonReport {
        let mut report = SeasonReport::new();
        for pass in SEASON_PIPELINE {
            self.run_pass(pass, &mut report);
            report.passes.push(pass);
        }
        report
    }

    /// One pass of the pipeline. Split out so a test can run a single pass
    /// against a hand-built kingdom.
    pub fn run_pass(&mut self, pass: Pass, report: &mut SeasonReport) {
        match pass {
            Pass::AiManageFarms => {
                self.ai_manage_farms_all();
            }
            Pass::Clock => self.clock(),
            Pass::EventRoll => self.event_roll(report),
            Pass::Weather => self.weather(),
            Pass::TaxCollect => self.tax_collect(),
            Pass::WagesPay => self.wages_pay(report),
            Pass::RationApply => self.ration_apply(false),
            Pass::HealthUpdate => self.health_update(),
            Pass::HappinessUpdate => self.happiness_update(),
            Pass::UnrestUpdate => self.unrest_update(report),
            Pass::SecedeIsolatedCounties => self.secede_isolated_counties(report),
            Pass::CountyRecountFields => crate::field::recount_all(
                &mut self.counties,
                self.county_count,
                &self.campaign.map,
            ),
            Pass::FertilityUpdate => self.fertility_update(),
            Pass::FieldReclaim => self.field_reclaim(),
            Pass::GrainSeasonTick => self.grain_season_tick(),
            Pass::HerdSeasonTick => self.herd_season_tick(),
            Pass::Industry(c) => self.industry(c),
            Pass::CastleBuildTick => self.castle_build_tick(report),
            Pass::LabourAllocate | Pass::LabourAllocateAgain => self.labour_allocate_all(),
            Pass::MigrationUpdate => self.migration_update(),
            Pass::PopulationUpdate => self.population_update(),
            Pass::CountyRecountMerchants => crate::merchant::recount_all(
                &mut self.counties,
                self.county_count,
                &self.campaign.units,
            ),
            Pass::ScoreRank => ai::rank_realms(&self.tables, &mut self.realms),
            Pass::History => self.history(),
            Pass::RationPreview => self.ration_apply(true),
            Pass::RefreshEstimates => self.refresh_estimates_all(),
            Pass::MercenaryAdvance => self.mercenary_advance(),
            Pass::UnitsResetMoves => self.units_reset_moves(),
            Pass::ReconcileAlliances => self.reconcile_alliances(),
        }
    }

    // --- the passes --------------------------------------------------------

    fn clock(&mut self) {
        let ended = self.season_next;
        self.turn_count += 1;
        self.season_prev = self.season;
        self.season = self.season_next;
        self.season_next = if ended + 1 > 4 { 1 } else { ended + 1 };
        if ended == 4 {
            self.year = self.year_next;
            self.year_next += 1;
        }
    }

    fn event_roll(&mut self, report: &mut SeasonReport) {
        let quirks = self.options.quirks;
        let Some(season) = self.season() else { return };
        let humans: [bool; MAX_REALMS] = core::array::from_fn(|i| self.realms[i].is_human);
        let owner_is_human = move |owner: u8| {
            owner != 0 && (owner as usize) < MAX_REALMS && humans[owner as usize]
        };
        // The seven events that touch a treasury or a stockpile read and write
        // the realm record, which `event` deliberately does not depend on.
        let mut purses: Vec<event::RealmPurse> = self
            .realms
            .iter()
            .map(|r| event::RealmPurse {
                gold: r.gold,
                wood: r.wood,
                stone: r.stone,
                weapons: r.weapons,
            })
            .collect();
        event::roll_all(
            &self.tables,
            &mut self.counties,
            self.county_count,
            &owner_is_human,
            &mut purses,
            self.year,
            season,
            &mut self.rng,
            quirks,
            &mut report.messages,
        );
        for (realm, purse) in self.realms.iter_mut().zip(purses) {
            realm.gold = purse.gold;
            realm.wood = purse.wood;
            realm.stone = purse.stone;
            realm.weapons = purse.weapons;
        }
    }

    fn weather(&mut self) {
        let Some(season) = self.season() else { return };
        weather::update_all(
            &self.tables,
            &mut self.counties,
            self.county_count,
            season,
            self.options.advanced_farming,
            &mut self.rng,
            &mut self.weather_county,
        );
    }

    fn tax_collect(&mut self) {
        tax::sum_empire_happiness(
            &self.tables,
            &mut self.counties,
            &mut self.realms,
            self.county_count,
            self.options.quirks,
        );
        for id in 1..=self.county_count {
            let owner = self.counties[id].owner as usize;
            let empire = if owner != 0 && owner < MAX_REALMS {
                self.realms[owner].tax_hap_empire as i32
            } else {
                0
            };
            let take = tax::collect(&self.tables, &mut self.counties[id], empire);
            // **Both limbs of the branch**, which is the point: an unowned
            // county's take is banked in its own purse and is the only money it
            // ever has to shop with. See [`tax::bank`].
            tax::bank(&mut self.counties[id], &mut self.realms, take);
        }
    }

    /// `Wages_PayAll` — starve the armies, rebuild the bill, then pay it.
    ///
    /// This is the hook `docs/armies.md` §6.4 calls *"the missing half"*, and
    /// until now it had nothing to attach to: the pass paid a `realm.wages` a
    /// caller had to set, and told [`industry::pay`] that nobody had
    /// mercenaries because there were no units to ask. Three things happen here
    /// now, in the original's order:
    ///
    /// 1. **`Army_Starve` runs first**, before anyone is charged — so an army
    ///    can desert from hunger and be billed the reduced wage in the same
    ///    season;
    /// 2. the bill is rebuilt from the realm's surviving armies by
    ///    [`crate::unit::refresh_wages`], which also writes each army's own
    ///    share into its record for the panel;
    /// 3. the realm pays, and bankruptcy escalates against real armies —
    ///    stage 1 walks the mercenaries out, stages 2 to 4 desert a tenth of
    ///    every troop count, and stage 5 destroys every army the realm has.
    fn wages_pay(&mut self, report: &mut SeasonReport) {
        crate::unit::starve(
            &self.tables,
            &mut self.campaign.units,
            &self.counties,
            self.options.armies_eat,
            &mut report.messages,
        );

        for id in 1..MAX_REALMS {
            if !self.realms[id].in_play {
                continue;
            }
            crate::unit::refresh_wages(
                &self.tables,
                &mut self.campaign.units,
                &mut self.realms,
                id as u8,
                self.options.difficulty,
            );
            let had_mercenaries = self
                .campaign
                .units
                .iter()
                .any(|(_, u)| u.owner == id as u8 && u.mercenaries.is_some());
            industry::pay(&mut self.realms[id], id as u8, had_mercenaries, &mut report.messages);

            let stage = self.realms[id].bankrupt_stage;
            self.apply_bankruptcy(id as u8, stage);
        }
    }

    /// The half of `docs/kingdom.md` §7.4's bankruptcy ladder that acts on
/// armies.
    ///
    /// [`industry::pay`] advances the stage and reports it; what the stage
    /// *does* needs the unit array, so it happens here. The mapping is the one
    /// [`industry::BankruptcyAction`] already documents — stage 1 is
    /// `Mercenary_Release` over the whole realm (`L2.eng` 160, *"Mercenaries
    /// desert!"*), stages 2..=4 are `Army_Desert` per army, and stage 5 is the
    /// mutiny that destroys every army the realm holds.
    fn apply_bankruptcy(&mut self, realm: u8, stage: u8) {
        match stage {
            1 => {
                self.campaign.mercenaries.release_realm(&mut self.campaign.units, realm);
            }
            2..=4 => {
                for (_, u) in self.campaign.units.iter_mut() {
                    if u.owner == realm && u.kind == crate::unit::UnitKind::Army {
                        u.desert();
                    }
                }
            }
            // `industry::pay` resets the counter to 0 after the mutiny, so the
            // mutiny is reported as stage 0 — see `Message::Bankrupt`.
            0 => {}
            _ => {}
        }
    }

    /// `Units_ResetMoves` (`0x004651B9`) — turn phase 7. Every slot, every
    /// type: `moveState = 0` and `movesUsed = 0`.
    fn units_reset_moves(&mut self) {
        self.campaign.units.reset_moves();
    }

    /// `Mercenary_AdvanceAll` (`0x004ACA2B`) — turn phase 7, and **before**
    /// [`Kingdom::units_reset_moves`].
    ///
    /// > `docs/armies.md` §2.1 says phase 7 *"calls `Units_ResetMoves` … then
    /// > `Move_BuildCostMap` and `Mercenary_AdvanceAll`"*. The order is the
    /// > other way round: the three calls are literally consecutive as
    /// > `Mercenary_AdvanceAll(); Units_ResetMoves(); Move_BuildCostMap();`.
    /// > Corrected in the document. It matters because the order two peers run
    /// > the season's passes in is the specification (`docs/kingdom.md` §3.4),
    /// > and [`SEASON_PIPELINE`] is where that is written down. `[D]`
    fn mercenary_advance(&mut self) {
        let count = self.county_count;
        let quirks = self.options.quirks;
        self.campaign.mercenaries.advance(&mut self.counties, count, quirks);
    }

    fn ration_apply(&mut self, preview: bool) {
        for id in 1..=self.county_count {
            if preview {
                ration::preview(&self.tables, &mut self.counties[id], self.options.armies_eat);
            } else {
                ration::apply(&self.tables, &mut self.counties[id], self.options.armies_eat);
            }
        }
    }

    fn health_update(&mut self) {
        for id in 1..=self.county_count {
            health::update(&self.tables, &mut self.counties[id]);
        }
    }

    fn happiness_update(&mut self) {
        for id in 1..=self.county_count {
            let human = self.owner_is_human(self.counties[id].owner);
            happiness::update(&mut self.counties[id], human, self.turn_count);
        }
    }

    /// `Unrest_UpdateAll` (`0x0044AA41`), including the call it makes at the
    /// bottom that this pass used to skip.
    ///
/// The counter is reset **only when a mob was placed** —
    /// `if (3 < unrest && County_RaiseRevolt(c)) unrest = 0;`. A county with no
    /// free tile within three of its anchor therefore sits at 4 for ever, and
    /// that is the original's behaviour, not a shortcut. `crate::unrest`'s
    /// module docs have the three corrections this call site came with.
    fn unrest_update(&mut self, report: &mut SeasonReport) {
        let quirks = self.options.quirks;
        let year = self.year;
        for id in 1..=self.county_count {
            let human = self.owner_is_human(self.counties[id].owner);
            let mut messages = Vec::new();
            let wants_revolt =
                unrest::update(&mut self.counties[id], id as u8, human, quirks, &mut messages);
            for m in messages {
                report.message(m);
            }
            if !wants_revolt {
                continue;
            }
            let Some((_slot, men)) = unrest::raise_revolt(
                &self.campaign.map,
                &self.counties[id],
                &mut self.campaign.units,
                id,
                year,
            ) else {
                continue;
            };
            // `County_MakeIndependent` first — its `Labour_Allocate` deals the
            // population the mob has not yet been taken out of — then the debit.
            self.make_county_independent(id);
            self.counties[id].population -= men;
            self.counties[id].unrest = 0;
            report.message(Message::Revolt { county: id as u8 });
        }
    }

    fn fertility_update(&mut self) {
        for id in 1..=self.county_count {
            land::update_fertility(&mut self.counties[id], self.options.advanced_farming);
        }
    }

    /// `Field_ReclaimTick` and `Field_ReclaimEstimate`, per county — the second
    /// and third statements of `Fields_SeasonTick`.
    fn field_reclaim(&mut self) {
        for id in 1..=self.county_count {
            land::reclaim_fields(&self.tables, &mut self.counties[id], &mut self.campaign.map);
            // The estimate is the pass's own tail call, and the tick has just
            // moved every input it has.
            self.counties[id].labour_wanted[crate::tables::JOB_FIELD_RECLAMATION] =
                crate::county::LABOUR_NO_FLOOR;
            self.counties[id].labour_useful[crate::tables::JOB_FIELD_RECLAMATION] =
                land::reclaim_labour_estimate(&self.tables, &self.counties[id], &self.campaign.map);
        }
    }

    /// `Grain_SeasonTick`, with `Grain_LabourEstimate` as its last line.
    fn grain_season_tick(&mut self) {
        let Some(season) = self.season() else { return };
        let season_next = Season::from_index(self.season_next).unwrap_or(Season::Spring);
        let advanced = self.options.advanced_farming;
        let quirks = self.options.quirks;
        for id in 1..=self.county_count {
            land::grain_season_tick(&self.tables, &mut self.counties[id], season, advanced, quirks);
            // **The repaint `Grain_SeasonTick` ends with**, which had no
            // counterpart until a player said the wheat never grows. It writes
            // the crop-density band onto every grain tile of the county, and
            // that byte is what `l2_view::campaign::field_variant` reads back.
            // It was called twice here, the second under a copy of this
            // comment with its two names stripped; the repaint is idempotent,
            // so the copy changed no pixel and hid nothing but itself.
            land::grain_repaint_fields(id, &self.counties[id], season, &mut self.campaign.map);
            if let Some(grain) = land::grain_labour_estimate(
                &self.tables,
                &self.counties[id],
                season_next,
                advanced,
            ) {
                self.counties[id].labour_wanted[crate::tables::JOB_GRAIN_FARMING] = grain.wanted;
                self.counties[id].labour_useful[crate::tables::JOB_GRAIN_FARMING] = grain.useful;
            }
        }
    }

    /// `Herd_SeasonTick`, with `Herd_LabourEstimate` as its last line — which
    /// is [`land::herd_preview`]'s forecast *and* the cattle ceiling, one
    /// function in the original.
    fn herd_season_tick(&mut self) {
        for id in 1..=self.county_count {
            land::herd_season_tick(
                &self.tables,
                &mut self.counties[id],
                self.season,
                self.season_next,
            );
            // `Herd_SeasonTick`'s last act is `Herd_UpdateCrowding`, which
            // writes the crowding level **and repaints the county's pasture**.
            // `land::herd_season_tick` has no map and does only the first half;
            // this is the second. Without it the animals on the ground never
            // change however the herd grows, which is what the map looked like
            // until somebody asked why the pastures were empty.
            crate::field::herd_update_crowding(
                &self.tables,
                &mut self.counties[id],
                &mut self.campaign.map,
            );
            if self.counties[id].pop_band != 0 {
                // Both words of the record — the break-even floor as well as
                // the ceiling. See `land::herd_labour_estimate`.
                let herd =
                    land::herd_labour_estimate(&self.tables, &self.counties[id], self.season_next);
                self.counties[id].labour_wanted[crate::tables::JOB_CATTLE_FARMING] = herd.wanted;
                self.counties[id].labour_useful[crate::tables::JOB_CATTLE_FARMING] = herd.useful;
            }
        }
    }

    /// `Labour_AllocateAll` (`0x0044F699`) — [`crate::labour::allocate`] for
    /// counties 1..=`county_count`, in index order.
    fn labour_allocate_all(&mut self) {
        for id in 1..=self.county_count {
            crate::labour::allocate(&mut self.counties[id]);
        }
    }

    /// `Panels_RefreshAll`'s middle statement, and `Tax_RecomputePreview` with
    /// it — the third.
    fn refresh_estimates_all(&mut self) {
        for id in 1..=self.county_count {
            self.refresh_estimates(id);
            crate::tax::recompute_preview(&self.tables, &mut self.counties[id]);
        }
    }

    /// `Realm_SecedeIsolatedCounties` (`0x0044AE3C`) — `docs/kingdom.md` §6.1.
    fn secede_isolated_counties(&mut self, report: &mut SeasonReport) {
        let blocks = crate::territory::build_blocks(&self.counties, self.county_count);
        let strength: Vec<u8> = self.realms.iter().map(|r| r.strength).collect();
        for cut in crate::territory::minor_blocks(&blocks, &strength) {
            for county in &cut.counties {
                self.make_county_independent(*county as usize);
            }
            // The original tells only the local player, and only when the realm
            // held more than one block. Both messages are raised here whatever
            // realm they belong to, because this crate has no local player: the
            // realm is on the message and the UI filters. An **AI loses its
            // outlying counties in silence**, which is the same rule seen from
            // the other side.
            if cut.blocks > 1 {
                report.message(if cut.counties.len() == 1 {
                    Message::CountySeceded { realm: cut.realm, county: cut.counties[0] }
                } else {
                    Message::LandsDivide { realm: cut.realm, counties: cut.counties.len() as u8 }
                });
            }
        }
        if !self.counties.is_empty() {
            crate::conquest::recount_realm_counties(&self.counties, &mut self.realms);
        }
    }

    /// **What [`crate::conquest::make_independent`] and a capture need** that
    /// neither takes a kingdom to read: `g_seasonNext`, `g_optAdvancedFarming`,
    /// `g_optArmiesEat` and `g_countyCount`.
    pub fn restore(&self) -> crate::conquest::Restore {
        crate::conquest::Restore {
            season_next: Season::from_index(self.season_next).unwrap_or(Season::Spring),
            advanced_farming: self.options.advanced_farming,
            armies_eat: self.options.armies_eat,
            county_count: self.county_count,
        }
    }

    /// `County_MakeIndependent` (`0x004AC3C6`) — secession, revolt, the
    /// elimination of a realm and a conquest the taker cannot reach all end
    /// here. The function is [`crate::conquest::make_independent`]; this is the
    /// kingdom's arguments for it.
    ///
    /// The idle-townsfolk effect the industry switches produce is the
    /// difference between the shipped save's `[0, 218, 0, 0, 0, 0, 217, 0, 0]`
    /// and its `[0, 323, 0, 0, 0, 0, 0, 0, 133]`.
    pub fn make_county_independent(&mut self, county: usize) {
        if county == 0 || county > self.county_count {
            return;
        }
        let r = self.restore();
        let Kingdom { counties, realms, campaign, tables, .. } = self;
        crate::conquest::make_independent(tables, counties, realms, county as u8, &campaign.map, r);
    }

    /// One `Industry_Produce` run over every county.
    ///
    /// The blacksmith's stockpile share ([`industry::weapon_shares`],
    /// `FUN_0044F15B`) is computed **per realm, before the loop**. The original
    /// recomputes it inside every `resourceLimit` call, which is the same
    /// answer: its inputs are which smithies are switched on and staffed, and
    /// the pass changes neither.
    ///
    /// # `Industry_ProduceAll`'s estimates
    ///
    /// ```c
    /// for (c = 1..) { Industry_Produce(c, weapons); Industry_UpdateSiteTile(c, 2);
    ///                 Industry_LabourEstimate(c, weapons); capacity[2] = workers[7]; }
    /// for (c = 1..) { Industry_Produce(c, iron); …(c, stone); …(c, wood); …three site tiles…
    ///                 Industry_LabourEstimate(c, iron / stone / wood / weapons); …capacities… }
    /// ```
    ///
    /// **The estimates are why a new game has foresters.** `Season_Advance`
    /// runs `Labour_AllocateAll` straight after this, and on a new game nothing
    /// before it has computed an industry ceiling: `Game_SetupRealmsAndCounties`
    /// refreshes the start county *before* it switches the forest on. So the
    /// allocation's wood ceiling is the one written here. Without them a new
    /// England opened with **no foresters, 155 idle and all four sidebar
    /// forecasts at zero in every start county**, where `england-turn1.sav`'s
    /// human county has 108 foresters and forecasts 86. A loaded game hid it,
    /// because the importer carries the last season's ceilings.
    ///
    /// They run in the pass that ends each of the original's loops — weapons'
    /// own, and wood's for the other four — so the order they see the stock in
    /// is the original's **except in one place, `[D]`**: this crate runs iron
    /// and stone over *every* county before wood runs over any, where the
    /// original's second loop does iron, stone and wood county by county. So
    /// the weapons estimate in county `c` sees the iron every county of its realm
    /// mined this season, not only counties `1 ..= c`. It moves a blacksmith's
    /// ceiling in a realm where a later county mines iron, for the season's two
    /// allocations only — `Panels_RefreshAll` recomputes it from the final stock
    /// before anything is drawn. Folding the passes together would move
    /// `l2_game::save`'s pass indices.
    ///
    /// **Not here, as C136 says:** the estimate's efficiency write-back and the
    /// `capacity = workers` writes.
    fn industry(&mut self, commodity: Commodity) {
        let shares: [industry::WeaponShare; MAX_REALMS] = core::array::from_fn(|realm| {
            industry::weapon_shares(&self.tables, &self.counties, self.county_count, realm as u8)
        });
        let advanced = self.options.advanced_farming;
        let count = self.county_count;
        let neutral = Realm::new();
        let estimated: &[Commodity] = match commodity {
            Commodity::Weapons => &[Commodity::Weapons],
            Commodity::Wood => &crate::tables::INDUSTRY_ESTIMATE_ORDER,
            Commodity::Iron | Commodity::Stone => &[],
        };
        let (counties, realms, tables) = (&mut self.counties, &mut self.realms, &self.tables);
        for id in 1..=count {
            let owner = counties[id].owner as usize;
            if owner >= MAX_REALMS {
                continue;
            }
            // An unowned county has no realm to credit, so it produces nothing.
            if owner != 0 {
                industry::produce_with_share(
                    tables,
                    &mut counties[id],
                    &mut realms[owner],
                    commodity,
                    advanced,
                    shares[owner],
                );
            }
            // …but it is still estimated: `Industry_ProduceAll` has no owner
            // test, and the estimate's own is what zeroes a neutral county.
            let realm = realms.get(owner).unwrap_or(&neutral);
            for &c in estimated {
                industry::refresh(tables, &mut counties[id], c, realm, shares[owner], advanced);
            }
        }
        // `Industry_ProduceAll` follows **every** `Industry_Produce` with an
        // `Industry_UpdateSiteTile` for the same commodity, in the same county
        // loop. It is what turns a mine's picture back off when its last worker
        // is taken away, and what un-wrecks a trampled one the season its
        // countdown expires — the branch inside `Industry_Produce` calls the
        // same function.
        //
        // **`Industry_ProduceAll` (`0x0044E852`) has no owner test here, and
        // this used to.** Its loop is `for (c = 1; c <= g_countyCount; c++)`
        // with the update unconditional; the owner test is one level down, in
        // `Industry_Produce` (`0x0044EA92`), where `if (realm != 0)` guards the
        // *production* and nothing else. A county that leaves a realm still has
        // its site tiles repainted the next season.
        //
        // The visible defect was a seceded county's mine going on turning for
        // the rest of the game. `County_MakeIndependent` (`0x004AC3C6`) clears
        // all four `enabled` flags and repaints nothing — `Industry_Produce`
        // and `Industry_ProduceAll` hold every call site of
        // `Industry_UpdateSiteTile` there is — so the terrain byte stays on
        // `base + 1`, the *working* value `Sprite_TopIt` (`0x004071A0`)
        // animates, and this pass is the only thing that ever writes
        // `base + 0` over it.
        for id in 1..=self.county_count {
            self.update_industry_site(id, commodity);
        }
    }

    /// `Castle_BuildTick` (`0x004508DE`) over every county, plus the two things
    /// it does that [`industry::build_tick`] cannot reach: the free garrison a
    /// finished castle comes with, and the tile the castle is drawn on.
    fn castle_build_tick(&mut self, report: &mut SeasonReport) {
        // **`Castle_BuildTick`'s first loop, and the sixth score input.**
        // `for (r = 1; r < 6; r++) { realm[r][0x4C] = 0; realm[r][0x4D] = 0; }`
        // — realms 1..=5 only, so realm 0's counters are cleared by
        // `Game_SetupRealmsAndCounties` and never again.
        //
        // `crate::tables::SCORE_INPUT_OFFSETS` has read `+0x4C` as *castles
        // held* for as long as it has existed, with the C for it in the doc
        // comment — and **nothing in this workspace ever wrote it**, so
        // `score_inputs[5]` was zero for every realm for the whole game and the
        // heaviest-weighted of the six terms (x50, more than the other five
        // combined) contributed nothing to anybody's score. That is
        // `docs/agents.md`'s *a correct explanation sitting directly above the
        // omission it describes*, and `crates/l2-game/tests/differential.rs` is
        // what found it: with the score's other five inputs repaired, every
        // realm came in exactly 50 short, once per castle.
        for realm in self.realms.iter_mut().take(MAX_REALMS).skip(1) {
            realm.score_inputs[crate::tables::SCORE_INPUT_CASTLES] = 0;
        }
        for id in 1..=self.county_count {
            let owner = self.counties[id].owner as usize;
            // **Counted before the work, not after**, which is the original's
            // order and decides a real case: the season a castle tops out, the
            // county is still `castle_degraded != 0` here, so it is counted into
            // `+0x4D` (the concurrency count, derived in `ai::build_castles`)
            // and does not score its 50 until the *next* season.
            //
            // The increment is not guarded on the owner, so an unowned county
            // holding a castle increments realm 0's counter — which nothing
            // clears and nothing reads. Reproduced as the original writes it;
            // `Score_RankRealms` loops 1..=5, so the slot is inert either way.
            if self.counties[id].castle_degraded == 0 && self.counties[id].castle_type != 0 {
                if let Some(r) = self.realms.get_mut(owner) {
                    r.score_inputs[crate::tables::SCORE_INPUT_CASTLES] += 1;
                }
            }
            let mut messages = Vec::new();
            let (mut realm, has_realm) = match self.realms.get(owner) {
                Some(r) => (r.clone(), true),
                None => (Realm::new(), false),
            };
            let done = industry::build_tick(
                &self.tables,
                &mut self.counties[id],
                &mut realm,
                id as u8,
                &mut messages,
            );
            if has_realm {
                self.realms[owner] = realm;
            }
            for m in messages {
                report.message(m);
            }
            // `Castle_StampTile` runs on **every** season the castle is under
            // way, not only on the one it finishes: the scaffolding at 49% and
            // the half-built walls at 50% are different pictures, and so is the
            // finished castle. The terrain byte is the half this crate owns.
            if self.counties[id].castle_degraded != 0 || done.is_some() {
                crate::map::stamp_castle_terrain(
                    &mut self.campaign.map,
                    id as u8,
                    self.counties[id].castle_type,
                );
            }
            let Some(done) = done else { continue };
            if done.free_archers > 0 {
                self.raise_free_garrison(id as u8, done.free_archers);
            }
        }
    }

    /// `Castle_RaiseFreeGarrison` (`0x004A551B`) — the archers a new castle
    /// comes with, mustered and marched straight inside.
    ///
    /// The original tops the realm's **bow** stock up by exactly the number it
    /// is about to hand out, so `Levy_ConsumeWeapons` takes them back and the
/// men cost nothing. Reproduced, because the
    /// order matters if the realm is short of bows: the top-up happens first,
    /// so it never is.
    fn raise_free_garrison(&mut self, county: u8, archers: i32) {
        let owner = self.counties[county as usize].owner;
        let Some(realm) = self.realms.get_mut(owner as usize) else { return };
        let bow = crate::unit::TroopType::Archer.weapon_slot().unwrap_or(4);
        realm.weapons[bow] += archers;
        let mut basket = crate::levy::LevyBasket::seed(realm, archers);
        basket.equip(crate::unit::TroopType::Archer, archers);
        let map = self.campaign.map.clone();
        let muster = crate::levy::Muster {
            realm: owner,
            county,
            year: self.year,
            happiness_cost: 0,
        };
        let Ok(unit) = crate::levy::create_army(
            &self.tables,
            &map,
            &mut self.counties,
            &mut self.realms,
            &mut self.campaign.units,
            &mut self.campaign.names,
            &basket,
            muster,
            &mut self.campaign.explored,
        ) else {
            return;
        };
        let realms = self.realms.clone();
        crate::conquest::garrison_apply(
            &self.tables,
            &map,
            &mut self.counties,
            &realms,
            &mut self.campaign.units,
            unit,
            county,
        );
    }

    fn migration_update(&mut self) {
        population::migrate_all(&mut self.counties, self.county_count, self.options.quirks);
    }

    fn population_update(&mut self) {
        let Some(season) = self.season() else { return };
        population::update_all(
            &self.tables,
            &mut self.counties,
            self.county_count,
            season,
            self.options.quirks,
        );
    }

    /// The history ring — `FUN_004AE7DD`. See [`History`].
    fn history(&mut self) {
        self.history.record(&self.counties);
    }

    /// `AI_SetTaxRates`' first half — set every county `realm` owns to the
    /// rate its happiness earns on that realm's ladder.
    ///
/// Like [`Kingdom::run_ai_grants`] this runs in the AI's turn
/// in `Season_Advance`, so it is exposed.
    /// **Realm 0 is the unowned counties**, which the original taxes once a
    /// turn in phase 1 on the neutral ladder. An out-of-range realm index does
/// nothing, because the caller is a turn machine and
    /// not a rule.
    /// **Paint one field.** `Field_SetType` (`0x00438BEC`) with this kingdom's
    /// own map, ruleset and clock supplied — the whole of what a click on the
    /// campaign map does to the simulation.
    ///
    /// This is the only writer of the field counts a player can reach, and
    /// until it existed there was none: every county of the England position
    /// starts with `fieldsGrain = 0` and nothing but the AI's own farming
    /// styles ever changed one. See [`crate::field`].
    ///
    /// A refusal is a refusal — the tile is not one of the county's twenty
    /// fields, it is blighted this season, or the brush is not one that tile's
    /// menu offers — and never a silent no-op.
    pub fn paint_field(
        &mut self,
        county: usize,
        tile: usize,
        brush: crate::field::FieldType,
    ) -> Result<(), crate::field::BrushRefusal> {
        let season_next = Season::from_index(self.season_next).unwrap_or(Season::Spring);
        crate::field::set_type(
            &mut self.counties,
            self.county_count,
            &mut self.campaign.map,
            county,
            tile,
            brush,
            season_next,
            &self.tables,
            self.options.advanced_farming,
            &self.realms,
        )
    }

    /// `County_RefreshEstimates` (`0x004485A5`) for one county, with the owning
    /// realm and its blacksmiths' share of the stockpile looked up.
    ///
/// Every caller in this crate goes through here
    /// arguments itself, because getting the *owner* wrong is the failure that
    /// leaves a county full of idle townsfolk.
    pub fn refresh_estimates(&mut self, county: usize) {
        if county == 0 || county >= self.counties.len() {
            return;
        }
        let season_next = Season::from_index(self.season_next).unwrap_or(Season::Spring);
        let advanced = self.options.advanced_farming;
        let owner = self.counties[county].owner;
        let share =
            crate::industry::weapon_shares(&self.tables, &self.counties, self.county_count, owner);
        let neutral = Realm::new();
        let realm = self.realms.get(owner as usize).unwrap_or(&neutral);
        crate::field::refresh_estimates(
            &mut self.counties[county],
            &self.campaign.map,
            season_next,
            &self.tables,
            advanced,
            realm,
            share,
        );
    }

    /// **Switch one industry, or castle building, on or off.**
    /// `Industry_ToggleFromMap` (`0x0043D309`) assembled — the enable byte, the
    /// industry share, and the allocation the original runs afterwards.
    ///
    /// Like [`Kingdom::paint_field`] this is only reachable from a click on the
    /// map, because in the original it is only reachable from a click on the
    /// map: no county panel has an industry switch. Returns the state the
    /// switch ends in.
    ///
    /// ```c
    /// enabled ^= 1;                                  /* or the castle switch */
    /// County_RefreshEstimates(county, seasonNext);
    /// Labour_ToggleIndustryShare(county, job, enabled);
    /// County_RefreshEstimates(county, seasonNext);
    /// Labour_Allocate(county); Ration_Apply(county, season); Labour_Allocate(county);
    /// County_RefreshEstimates(county, seasonNext);
    /// Industry_UpdateSiteTile(county, industry);
    /// FUN_00448648(owner);                           /* every blacksmith of the realm */
    /// ```
    ///
    /// [`crate::industry::toggle_from_map`] does the flip and the share toggle
    /// together, so the first two refreshes are one here: an estimate reads no
    /// labour share, and it is idempotent while the efficiency write-back is
    /// not ported (C136), so a refresh on either side of the share toggle is the
    /// same refresh. **The last line was missing, and it is a visible one.**
    /// The Readme: *"turning a blacksmith on will reduce the resources
    /// available to other blacksmiths"* — every other smithy of the realm has
    /// its ceiling and its sidebar forecast recomputed on the click, and ours
    /// kept the old numbers until the season. So was `Ration_Apply`, which is
    /// [`crate::ration::preview`] for the reason
    /// [`Kingdom::set_ration_wanted`] gives.
    pub fn toggle_industry(&mut self, county: usize, what: crate::industry::MapToggle) -> bool {
        if county == 0 || county > self.county_count {
            return false;
        }
        let quirks = self.options.quirks;
        let armies_eat = self.options.armies_eat;
        let on = crate::industry::toggle_from_map(&mut self.counties[county], what, quirks);
        self.refresh_estimates(county);
        crate::labour::allocate(&mut self.counties[county]);
        crate::ration::preview(&self.tables, &mut self.counties[county], armies_eat);
        crate::labour::allocate(&mut self.counties[county]);
        self.refresh_estimates(county);
        // `Industry_UpdateSiteTile(county, industry)` — the *visible* half of
        // the switch. A player: *"there's no message saying or visually
        // showing mining on / mining off."* The message is the caller's; this
        // is the picture.
        if let crate::industry::MapToggle::Industry(c) = what {
            self.update_industry_site(county, c);
        }
        let owner = self.counties[county].owner;
        self.refresh_blacksmiths(owner);
        on
    }

    /// **`Game_SetupRealmsAndCounties` (`0x0049BD99`)'s two rounds for one start
    /// county**, which run before it switches anything on.
    ///
    /// ```c
    /// Labour_Allocate(county); Ration_Apply(county, g_season); County_RefreshEstimates(county, g_seasonNext);
    /// Labour_Allocate(county); Ration_Apply(county, g_season); County_RefreshEstimates(county, g_seasonNext);
    /// castleType = g_startCastle; …materials, armoury…
    /// for (i = 0; i < 4; i++) if (i != 2 && industry[i].hasResource) { industry[i].enabled = 1; break; }
    /// castleSwitch = 1;
    /// ```
    ///
    /// **This is the only allocation a person's county gets before its first
    /// season.** An AI county is allocated again by `Ai_ManageFarmsAll` at the
    /// head of `Season_Advance`; the human one is not, so without these rounds
    /// it went into `Herd_SeasonTick` with nobody minding the cattle and into
    /// `Industry_ProduceAll` with nobody in the forest. Measured on a new
    /// England: the human county opened on 47 head and 47 idle, forecasting
    /// −10 cattle, where every AI county already matched `england-turn1.sav`.
    ///
    /// **The switches are off while the rounds run**, because the original sets
    /// them after. `l2_scenario::Scenario::from_map` has already set them, so
    /// they are put aside and put back. It is visible in the file: the human
    /// realm is the one whose wood stock did not move in the opening season —
    /// `Industry_ProduceAll`'s `symbols.json` note, *"(0, 66, 66, 132, 166)"* —
    /// because its foresters were dealt with the forest switched off.
    ///
    /// `Ration_Apply` is [`crate::ration::preview`]: it records and does not
    /// spend, for the reason [`Kingdom::set_ration_wanted`] gives.
    pub fn settle_start_county(&mut self, county: usize) {
        if county == 0 || county > self.county_count {
            return;
        }
        let armies_eat = self.options.armies_eat;
        let switches: [bool; 4] = core::array::from_fn(|i| self.counties[county].industry[i].enabled);
        let castle = self.counties[county].castle_switch;
        for industry in self.counties[county].industry.iter_mut() {
            industry.enabled = false;
        }
        self.counties[county].castle_switch = false;
        for _ in 0..2 {
            crate::labour::allocate(&mut self.counties[county]);
            crate::ration::preview(&self.tables, &mut self.counties[county], armies_eat);
            self.refresh_estimates(county);
        }
        for (industry, on) in self.counties[county].industry.iter_mut().zip(switches) {
            industry.enabled = on;
        }
        self.counties[county].castle_switch = castle;
    }

    /// **`FUN_00448648`** — `Industry_LabourEstimate(c, weapons)` for every
    /// county `owner` holds.
    ///
    /// ```c
    /// for (c = 1; c <= g_countyCount; c++)
    ///     if (g_counties[c].owner == owner) Industry_LabourEstimate(c, 2, 7, 15, 4);
    /// ```
    ///
    /// A blacksmith's ceiling is a share of the realm's stockpile split across
    /// every staffed smithy it owns ([`industry::weapon_shares`]), so anything
    /// that staffs or switches one moves every other smithy's number. Its two
    /// callers are `Labour_Move` (twice) and `Industry_ToggleFromMap`. The
    /// share cannot move inside the loop — an estimate staffs nothing — so it
    /// is taken once.
    pub fn refresh_blacksmiths(&mut self, owner: u8) {
        let share = industry::weapon_shares(&self.tables, &self.counties, self.county_count, owner);
        let advanced = self.options.advanced_farming;
        let count = self.county_count;
        let neutral = Realm::new();
        let (counties, realms, tables) = (&mut self.counties, &self.realms, &self.tables);
        let realm = realms.get(owner as usize).unwrap_or(&neutral);
        for id in 1..=count {
            if counties[id].owner == owner {
                industry::refresh(tables, &mut counties[id], Commodity::Weapons, realm, share, advanced);
            }
        }
    }

    /// **`FUN_0043A997(county, weaponType)` (`0x0043A997`) — what the blacksmith
    /// page's six hotspots do**, and the one thing a player could not tell this
    /// simulation: *"I can't choose what type of weapon my blacksmiths are
    /// making."*
    ///
    /// ```c
    /// county[+0x290] = weaponType;                 /* County::weapon_type */
    /// Industry_LabourEstimate(county, 2, 7, 0xF, 4);   /* weapons, the blacksmith */
    /// Labour_Allocate(county);
    /// County_RefreshEstimates(county, g_seasonNext);
    /// FUN_00448648(owner);                         /* every blacksmith of the realm */
    /// DAT_005530D0 = 1;                            /* a redraw */
    /// if (g_localPlayer == owner) { Panel_JobBlacksmith(); Sound_RestartSlot(8); }
    /// ```
    ///
    /// **Five statements and four of them are the recompute**, which is the
/// whole reason this is a method here
    /// screen. The type is a divisor in three places at once:
    /// [`industry::weapon_shares`] sums `g_weaponCost[type]` over the realm's
    /// staffed smithies, so **changing one county's weapon moves every other
    /// county's ceiling in the same realm** — the Readme's *"turning a
    /// blacksmith on will reduce the resources available to other
    /// blacksmiths"*, reached from the other side. That is what the closing
    /// [`Kingdom::refresh_blacksmiths`] is for, and it is the original's own
    /// last line.
    ///
    /// **The estimate comes before the allocation**, which is the opposite way
    /// round from [`Kingdom::toggle_industry`] and is the original's order:
    /// `Industry_LabourEstimate` writes `labour_useful[7]`, the ceiling the
    /// allocator then deals against, so
    /// on the same click. There is **no `Ration_Apply` and no second
    /// allocation** here; `docs/decisions.md` C177 and
    /// [`Kingdom::set_ration_wanted`] on why that asymmetry is not tidied.
    ///
    /// **No owner guard, and that is the original's too.** `Screen_HandleInput`'s
    /// `0x0F` arm hit-tests the six hotspots on `g_jobPanelJob == 8` alone; the
    /// only `g_localPlayer` test in the function is the one that decides whether
    /// to repaint and play the hammer. What keeps a player out of an AI's smithy
    /// is that the job popup opens on `g_selectedCounty`.
    ///
    /// Returns `false` for a county out of range or a weapon out of
    /// [`crate::tables::WEAPON_TYPE_COUNT`], and does nothing in that case. The
    /// original indexes `g_weaponCost` with the byte unchecked; the hotspot
    /// table can only ever publish 0…5, so
    /// screen and is here because this is a public method.
    pub fn set_weapon_type(&mut self, county: usize, weapon: usize) -> bool {
        if county == 0 || county > self.county_count || self.counties.len() <= county {
            return false;
        }
        if weapon >= crate::tables::WEAPON_TYPE_COUNT {
            return false;
        }
        self.counties[county].weapon_type = weapon;
        // `Industry_LabourEstimate(county, 2, 7, 0xF, 4)` — the weapons row
        // alone, with the realm share as it stands *before* the county-wide
        // refresh below. `industry::refresh` is that function whole: the search
        // loop's two words into `labour[7]` and the tail's forecast.
        let advanced = self.options.advanced_farming;
        let owner = self.counties[county].owner;
        let share =
            industry::weapon_shares(&self.tables, &self.counties, self.county_count, owner);
        let neutral = Realm::new();
        {
            let realm = self.realms.get(owner as usize).unwrap_or(&neutral);
            industry::refresh(
                &self.tables,
                &mut self.counties[county],
                Commodity::Weapons,
                realm,
                share,
                advanced,
            );
        }
        crate::labour::allocate(&mut self.counties[county]);
        self.refresh_estimates(county);
        self.refresh_blacksmiths(owner);
        true
    }

    /// **`Labour_Move` (`0x00439B52`) — the village's drag and its double
    /// click, whole.**
    ///
    /// ```c
    /// labour[to] += workers; labour[from] -= workers;
    /// FUN_00439CC2(county, from, to);                /* switch the destination on */
    /// Ration_Apply(county, g_season);
    /// County_RefreshEstimates(county, g_seasonNext);
    /// FUN_00448648(owner);
    /// Labour_RecomputeIndustryShare(county);
    /// Labour_RecomputeShares(county);
    /// Ration_Apply(county, g_season);
    /// County_RefreshEstimates(county, g_seasonNext);
    /// FUN_00448648(owner);
    /// ```
    ///
    /// allocator deals a county out from its shares, and the season runs it
    /// twice; `Labour_RecomputeShares` rewrites the shares from where people
    /// now stand, so the season deals the player's own split back to him. Ours
    /// moved the counts and nothing else, so every drag lasted until the
    /// season re-dealt the old split — *"the labor slider seems to reset each
    /// turn so that I have to reassign peasants to wheat each turn"* — and every
    /// forecast on the sidebar went on describing the staffing the player had
    /// just changed: *"industry values don't seem to update"*.
    ///
    /// `workers` is already clamped: that is `Village_Drop`'s job, and
    /// `Village_BalanceJob`'s arithmetic never overshoots, so `Labour_Move`
    /// does not repeat it. Returns `workers`. `Ration_Apply` is
    /// [`crate::ration::preview`], for the reason
    /// [`Kingdom::set_ration_wanted`] gives.
    pub fn move_labour(&mut self, county: usize, from: usize, to: usize, workers: i32) -> i32 {
        if county == 0 || county > self.county_count || from == to {
            return 0;
        }
        let jobs = self.counties[county].labour.len();
        if from >= jobs || to >= jobs {
            return 0;
        }
        let owner = self.counties[county].owner;
        let armies_eat = self.options.armies_eat;
        self.counties[county].labour[to] += workers;
        self.counties[county].labour[from] -= workers;
        self.switch_on_by_drop(county, to);
        for pass in 0..2 {
            if pass == 1 {
                crate::labour::recompute_industry_share(&mut self.counties[county]);
                crate::labour::recompute_shares(&mut self.counties[county]);
            }
            crate::ration::preview(&self.tables, &mut self.counties[county], armies_eat);
            self.refresh_estimates(county);
            self.refresh_blacksmiths(owner);
        }
        workers
    }

    /// **`FUN_00439CC2`** — the first thing `Labour_Move` does after the
    /// arithmetic, and it reads only the destination.
    ///
    /// ```c
    /// if      (to == 6 && industry[0].hasResource) rec = 0;   /* wood   */
    /// else if (to == 5 && industry[3].hasResource) rec = 3;   /* stone  */
    /// else if (to == 4 && industry[1].hasResource) rec = 1;   /* iron   */
    /// else if (to == 7 && industry[2].hasResource) rec = 2;   /* smithy */
    /// if (rec != 999) { industry[rec].enabled = 1; Industry_UpdateSiteTile(county, rec); }
    /// if (to == 3) castleSwitch = 1;
    /// ```
    ///
    /// **Putting men on a site is how a player switches it on**, and the only
    /// way besides the map. A new game opens with one industry on per start
    /// county — the first of wood, iron and stone it has, so never the mine in
    /// a county that also has a forest (`Game_SetupRealmsAndCounties`,
    /// `0x0049BD99`) — and without this a player who staffed that mine saw his
    /// men drawn as idle, no iron row on the sidebar, and the season send them
    /// home. `docs/decisions.md` C121 found it; nothing had built it. The share
    /// is not toggled here: `Labour_RecomputeShares` writes it from the workers.
    fn switch_on_by_drop(&mut self, county: usize, to: usize) {
        use crate::tables::{
            JOB_BLACKSMITH, JOB_CASTLE_BUILDING, JOB_IRON_MINING, JOB_STONE_QUARRYING,
            JOB_WOOD_CUTTING,
        };
        let record = match to {
            JOB_WOOD_CUTTING => Some(Commodity::Wood),
            JOB_STONE_QUARRYING => Some(Commodity::Stone),
            JOB_IRON_MINING => Some(Commodity::Iron),
            JOB_BLACKSMITH => Some(Commodity::Weapons),
            _ => None,
        };
        if let Some(c) = record {
            if self.counties[county].industry[c.index()].has_resource {
                self.counties[county].industry[c.index()].enabled = true;
                self.update_industry_site(county, c);
            }
        }
        if to == JOB_CASTLE_BUILDING {
            self.counties[county].castle_switch = true;
        }
    }

    /// **`Industry_UpdateSiteTile` (`0x0044EDC2`)**, the half of it that is
    /// terrain.
    ///
    /// ```c
    /// if (hasResource == 0 || disabledSeasons != 0) return;
    /// g_tiles[site].content = base + (enabled != 0);      /* 1 iron, 4 stone, 7 weapons, 10 wood */
    /// if (total - totalSnapshot < 1)
    ///     g_tiles[site].frame = idleFrame;                /* 30 iron, 0 stone, 10 weapons, 20 wood */
    /// ```
    ///
    /// **The `content` write is the on/off appearance**, and it is a whole
/// terrain value: an enabled site is `base + 1` and
    /// `Sprite_TopIt` animates exactly that value. So *"on"* is **motion**, not
    /// a different picture — see
    /// [`l2_view::campaign::industry_frames`](../../l2_view/campaign/fn.industry_frames.html).
    ///
    /// The frame reset is the other half and lives in the view, because this
    /// crate holds no graphics: a site with no output this season shows its
    /// idle frame, which is where the wheel starts from again.
    ///
    /// The guard matters and is easy to miss. A **wrecked** site — three
/// seasons on the countdown — is left as `Unit_TrampleTile` wrote
    /// it, so switching a trampled mine on and off changes nothing on the map
    /// until the countdown expires and `Industry_Produce`'s own call here
    /// repaints it.
    pub fn update_industry_site(&mut self, county: usize, c: crate::tables::Commodity) {
        let Some(record) = self.counties.get(county).map(|k| k.industry[c.index()]) else {
            return;
        };
        if !record.has_resource || record.disabled_seasons != 0 {
            return;
        }
        let Some(tile) = crate::map::industry_site(&self.campaign.map, county as u8, c) else {
            return;
        };
        let base = crate::map::terrain::INDUSTRY_IDLE[c.index()];
        self.campaign.map.terrain[tile] = base + u8::from(record.enabled);
    }

    /// **The farm/industry labour split** — `Labour_SetIndustryShare`
    /// (`0x0043933B`), which is what `Labour_SplitSliderDrag` (`0x00439122`,
    /// the *drag*, not the writer) calls once the track position has become a
    /// percentage:
    ///
    /// ```c
    /// county[+0x08] = share;
    /// Labour_Allocate(county);
    /// Ration_Apply(county, g_season);
    /// County_RefreshEstimates(county, g_seasonNext);
    /// ```
    ///
    /// **One pass, not two, and with `Ration_Apply` between them.** Ours ran
    /// `Labour_Allocate; County_RefreshEstimates` *twice* and never re-applied
    /// the ration at all — copied from [`toggle_industry`](Self::toggle_industry)
    /// on the reasoning that a control which moves the labour must re-allocate,
    /// which is true and is not the same as running the same pair twice. The
    /// omitted `Ration_Apply` is the one that matters: `herd_eaten` sizes the
    /// herd the estimate that follows searches over, and the forecast subtracts
    /// it twice. Two passes and a bare `Ration_SetSplit` is a different
    /// control — see [`set_ration_wanted`](Self::set_ration_wanted), which
    /// explains why that asymmetry is deliberate and must not be tidied.
    ///
    /// The doc this replaces named `FUN_00439122` and county `+0x2C`; the
    /// writer is `0x0043933B` and the field is `+0x08`.
    ///
    /// [`crate::labour::allocate`] reads
    /// [`crate::county::County::industry_share`] to size the industry pool, so
    /// moving the split without re-running it would leave every job's headcount
    /// describing the split the player just left.
    ///
    /// **What this does *not* fix, and cannot:** a click here also re-runs an
    /// allocation the season left owing, so on a county whose cattle ceiling is
    /// bounded by its population the milkmaid count *rises* when the player
    /// drags towards industry. That is the original's, and `docs/bugs.md` has
    /// it — the pipeline refreshes the ceilings after the last
    /// `Labour_AllocateAll`, so
    pub fn set_industry_share(&mut self, county: usize, share: i32) -> bool {
        if county == 0 || county > self.county_count {
            return false;
        }
        let share = share.clamp(0, 100);
        if self.counties[county].industry_share == share {
            return false;
        }
        self.counties[county].industry_share = share;
        crate::labour::allocate(&mut self.counties[county]);
        let armies_eat = self.options.armies_eat;
        // `Ration_Apply` records and does not spend — [`set_ration_wanted`] and
        // [`toggle_army_foraging`] make the same substitution for the same
        // reason. Calling the spending twin here would charge the county for a
        // meal every time the player nudged the slider.
        crate::ration::preview(&self.tables, &mut self.counties[county], armies_eat);
        self.refresh_estimates(county);
        true
    }

    /// **The wanted ration level — the third control on the ration panel, and
    /// the third with the same omission.** `Ration_IncreaseCounty`
    /// (`0x0043A23F`) and its twin:
    ///
    /// ```c
    /// if (rationWanted < 5) rationWanted++;      /* the cap is the table's length */
    /// Ration_Apply(county, g_season);            /* ONCE, not twice */
    /// County_RefreshEstimates(county, g_seasonNext);
    /// Panel_Ration();
    /// ```
    ///
    /// **One pass each, where `Ration_SetSplit` runs two. Do not tidy this into
    /// symmetry.** It is the original's asymmetry, it is deliberate, and the
    /// reason is legible: the split's search walks the value up to a hundred
    /// times and can leave the county's labour describing a split it then
    /// walked away from, so that control re-runs the pair to settle it. A level
    /// change moves once and has nothing to settle.
    ///
/// Written here because the next reader
    /// of these two functions will see `for _ in 0..2` beside a bare call and
    /// reach for the loop.
    ///
    /// The recompute is unconditional — the guard is only on the increment — so
    /// a click at the cap still re-applies and repaints. Ours wrote
    /// `ration_wanted` and returned, exactly like the other two, and this one
/// was found by *enumerating the class*
    /// it: `docs/agents.md`, **the correction that identifies a class must
    /// enumerate the class**.
    ///
    /// It writes `rationWanted` (`+0x15E`) and never `rationAchieved`
/// (`+0x15D`): what the player asks for and what the stores could
    /// feed are different fields, and only the pass decides the second.
    ///
    /// Returns whether the level moved.
    pub fn set_ration_wanted(&mut self, county: usize, level: i32) -> bool {
        if county == 0 || county > self.county_count {
            return false;
        }
        let level = level.clamp(0, crate::tables::RATION_LEVEL_COUNT as i32 - 1);
        let moved = self.counties[county].ration_wanted != level;
        self.counties[county].ration_wanted = level;
        let armies_eat = self.options.armies_eat;
        // `Ration_Apply` computes and records; it does not spend. The spending
        // twin is `crate::ration::apply`, whose name matches the original's and
        // whose behaviour does not — see `set_ration_split`.
        crate::ration::preview(&self.tables, &mut self.counties[county], armies_eat);
        self.refresh_estimates(county);
        moved
    }

/// **`Opt_ToggleArmyForaging` (`0x004345D0`).**
    ///
    /// ```c
    /// g_optArmiesEat = (g_optArmiesEat != 1);
    /// for (i = 1; i <= g_countyCount; i++) {
    ///     Ration_Apply(i, g_season);
    ///     County_RefreshEstimates(i, g_seasonNext);
    /// }
    /// ```
    ///
    /// The switch changes who a county feeds — [`crate::ration::people_to_feed`]
    /// adds the armies standing in it — so the original re-runs the ration
    /// pass and the forecasts over every county *on the flip*, and the ration
    /// panel is right the moment the options panel closes. Ours flipped the
    /// flag and left every county's ration fields describing the old rule until
    /// the next season. `Ration_Apply` is [`crate::ration::preview`] here for the
    /// reason [`Kingdom::set_ration_wanted`] gives: it records and does not
    /// spend. `[V]` against the decompilation.
    ///
    /// The multiplayer branch — `Net_SendCommand(0x32, 0)` instead of the flip —
    /// is not here: `docs/netcode.md`, the original is not the authority there.
    pub fn toggle_army_foraging(&mut self) {
        self.options.armies_eat = !self.options.armies_eat;
        let armies_eat = self.options.armies_eat;
        for id in 1..=self.county_count {
            crate::ration::preview(&self.tables, &mut self.counties[id], armies_eat);
            self.refresh_estimates(id);
        }
    }

/// **The tax rate.**
    /// `Tax_IncreaseCounty` (`0x0043AA83`) and its twin, whole:
    ///
    /// ```c
    /// if (taxRate < 0x32) taxRate++;      /* 50 is the player's ceiling */
    /// Tax_RecomputePreview(county);       /* taxShown, both happiness terms */
    /// Panel_Tax();                        /* repaint */
    /// ```
    ///
    /// and `Tax_RecomputePreview` itself ends with `Tax_SumEmpireHappiness(owner)`
    /// and `FUN_0044BA35`, the empire-wide sum of `taxShown` that the court
    /// prints. **Every tax control in the original recomputes and repaints**,
    /// exactly like the ration slider — and this is the second panel found with
/// the same omission, which is the finding.
    ///
    /// Ours wrote `taxRate` and stopped, so `tax_shown` kept whatever the last
    /// season's [`crate::tax::collect`] left in it (zero, before the first
    /// collection) and both happiness terms went stale the moment the rate
    /// moved. A player reported both halves in one sentence.
    ///
    /// **Watch which term is expected to move.** `d_hap_tax_local` is `5 - rate`
    /// and moves on every click; `tax_hap_other` is `g_taxHappinessOther[rate]`,
    /// which is **flat zero from rate 0 to 19**, so the *Other counties* line
/// does not budge over most of the range a player uses. That is
    /// the panel being right, and `docs/rules.md` says so.
    ///
    /// Returns whether the rate moved.
    pub fn set_tax_rate(&mut self, county: usize, rate: i32) -> bool {
        if county == 0 || county > self.county_count {
            return false;
        }
        let rate = rate.clamp(0, crate::tables::MAX_TAX_RATE);
        let moved = self.counties[county].tax_rate != rate;
        self.counties[county].tax_rate = rate;
        crate::tax::recompute_preview(&self.tables, &mut self.counties[county]);
        // `Tax_SumEmpireHappiness(owner)` — the realm's own term is a sum over
        // its counties, so one county's rate moves every county's *This county*
// line. Recomputed here for the same
        // reason the rest of this function exists.
        let quirks = self.options.quirks;
        crate::tax::sum_empire_happiness(
            &self.tables,
            &mut self.counties,
            &mut self.realms,
            self.county_count,
            quirks,
        );
        moved
    }

/// **The grain-to-livestock split.**
    /// `Ration_SetSplit` (`0x0043A5A9`), whole.
    ///
    /// A player reported the ration panel's slider as *"moves but is
    /// inoperable"*. It was writing the field and stopping, and every number on
    /// the panel stayed where it was until the turn ended — so the thumb
    /// travelled and nothing else did. The original does four more things, and
    /// three of them are visible:
    ///
    /// ```c
    /// old = rationSplit;  dir = sign(split - old);
    /// rationSplit = split;
    /// was = herdEaten;
    /// Ration_Apply(county, g_season);                       /* the food pass, NOW */
    /// if (herd && herdEaten && dir && split != 0 && split != 100 && herdEaten == was) {
    ///     rationSplit = old;                                /* ... the search ... */
    ///     do {
    ///         if (++n > 100) goto done;
    ///         rationSplit = clamp(rationSplit + dir, 0, 100);
    ///         Ration_Apply(county, g_season);
    ///         if (herdEaten != was) goto done;
    ///     } while (rationSplit != split || sweep == 0);
    ///     rationSplit = old; Ration_Apply(county, g_season); /* give up: spring back */
    /// }
    /// done:
    /// Labour_Allocate(county); County_RefreshEstimates(county, g_seasonNext);   /* twice */
    /// Labour_Allocate(county); County_RefreshEstimates(county, g_seasonNext);
    /// if (g_selectedCounty == county) Panel_Ration();
    /// ```
    ///
    /// **The slider refuses to sit on a value that changes nothing.** If the
    /// county has a herd, the herd is being eaten, the value moved, the
    /// *requested* split is strictly inside 0…100, and `herdEaten` came out
    /// unchanged, it puts the old value back and walks one point at a time
    /// towards the request, re-running the food pass at every step, and stops at
/// the first split that moves `herdEaten`.
    ///
    /// **`sweep` is what tells a track jump from an arrow**, and it changes the
    /// ending. `Ration_SliderClick` passes 1 for a click on the track and 0 for
    /// an arrow:
    ///
    /// * **track jump** (`sweep`): the loop stops when it reaches the requested
    ///   value, and if nothing changed on the way the split is **restored** —
    ///   the thumb springs back to where it was.
    /// * **arrow step** (`!sweep`): `(rationSplit != split) || (sweep == 0)` is
    ///   *always* true, so the walk does not stop at the requested value. It
    ///   keeps going in the same direction until `herdEaten` moves or a hundred
    ///   steps are spent — so **one click of an arrow can move the split by far
    ///   more than one**, and it does not spring back.
    ///
    /// **`Ration_Apply` does not spend.** It writes `rationAchieved`,
    /// `herdEaten`, `grainEaten`, the two `available` fields and the happiness
    /// delta, and the store is debited by the season. So
    /// to a hundred times is [`crate::ration::preview`] and **not**
    /// [`crate::ration::apply`], whose name matches the original's and whose
    /// behaviour does not — reaching for the same-named function would have had
    /// a drag eat the county's herd a hundred times over.
    ///
    /// Returns whether the split ended anywhere other than where it started,
    /// which is what a caller needs to know to decide whether to repaint.
    pub fn set_ration_split(&mut self, county: usize, split: i32, sweep: bool) -> bool {
        if county == 0 || county > self.county_count {
            return false;
        }
        let split = split.clamp(0, crate::county::MAX_RATION_SPLIT);
        let armies_eat = self.options.armies_eat;
        let old = self.counties[county].ration_split;
        let dir = match split.cmp(&old) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Greater => 1,
            std::cmp::Ordering::Equal => 0,
        };
        let was = self.counties[county].herd_eaten;

        {
            let c = &mut self.counties[county];
            c.ration_split = split;
        }
        crate::ration::preview(&self.tables, &mut self.counties[county], armies_eat);

        let stuck = {
            let c = &self.counties[county];
            c.herd != 0
                && c.herd_eaten != 0
                && dir != 0
                && split != 0
                && split != crate::county::MAX_RATION_SPLIT
                && c.herd_eaten == was
        };
        if stuck {
            self.counties[county].ration_split = old;
            let mut steps = 0;
            loop {
                steps += 1;
                if steps > 100 {
                    break;
                }
                {
                    let c = &mut self.counties[county];
                    c.ration_split = (c.ration_split + dir).clamp(0, crate::county::MAX_RATION_SPLIT);
                }
                crate::ration::preview(&self.tables, &mut self.counties[county], armies_eat);
                if self.counties[county].herd_eaten != was {
                    break;
                }
                // The `do … while` condition, and the whole of the difference
                // between the two gestures.
                if self.counties[county].ration_split == split && sweep {
                    self.counties[county].ration_split = old;
                    crate::ration::preview(
                        &self.tables,
                        &mut self.counties[county],
                        armies_eat,
                    );
                    break;
                }
            }
        }

        for _ in 0..2 {
            crate::labour::allocate(&mut self.counties[county]);
            self.refresh_estimates(county);
        }
        self.counties[county].ration_split != old
    }

    /// The map tiles that are one county's fields, with what each is being
    /// used for — what a screen needs to draw the brush's targets.
    pub fn field_tiles(&self, county: usize) -> Vec<(usize, crate::field::FieldType)> {
        let Some(c) = self.counties.get(county) else { return Vec::new() };
        (0..crate::county::MAX_FIELDS)
            .filter_map(|slot| c.field_tile(slot))
            .map(|tile| (tile, crate::field::classify(self.campaign.map.terrain[tile])))
            .collect()
    }

    pub fn run_ai_tax_rates(&mut self, realm: u8) {
        let Some(r) = self.realms.get(realm as usize) else { return };
        let lord = r.lord;
        ai::set_tax_rates(&self.tables, &mut self.counties, self.county_count, realm, lord);
    }

    /// `AI_SetTaxRates`' resource grants, which run in the AI's turn rather
    /// than in `Season_Advance`. Exposed separately for that reason.
    pub fn run_ai_grants(&mut self) {
        ai::grant_resources(
            &self.tables,
            &mut self.counties,
            &mut self.realms,
            self.county_count,
            self.options.difficulty,
        );
    }

    /// What the AI's farming passes read besides the counties and the map.
    fn farm_env(&self) -> crate::ai_farm::FarmEnv {
        crate::ai_farm::FarmEnv {
            season: self.season().unwrap_or(Season::Spring),
            season_next: Season::from_index(self.season_next).unwrap_or(Season::Spring),
            advanced_farming: self.options.advanced_farming,
            armies_eat: self.options.armies_eat,
        }
    }

    /// AI step 5 — `Ai_ManageCountyFarms`. Orders fields and runs the lord's
    /// farming style over every county the realm holds.
    ///
    /// `market` is the merchant seam. **The game's own callers pass the stall**
    /// — [`Kingdom::run_ai_farms_at_the_stall`] — and this form stays for a
    /// hand-built kingdom with no merchant model, which passes
    /// [`crate::ai_farm::NoMarket`] and still gets every style's layout,
    /// rations and labour split.
    pub fn run_ai_farms(&mut self, realm: u8, market: &mut dyn crate::ai_farm::Market) -> i32 {
        let Some(r) = self.realms.get(realm as usize) else { return 0 };
        let lord = r.lord;
        let env = self.farm_env();
        crate::ai_farm::manage_county_farms(
            &self.tables,
            &mut self.counties,
            self.county_count,
            &mut self.campaign.map,
            &self.realms,
            realm,
            lord,
            market,
            &env,
        )
    }

    /// Turn phase 1, step 2 — `AI_ManageFields(0)`, the unowned counties.
    pub fn run_neutral_farms(&mut self, market: &mut dyn crate::ai_farm::Market) -> i32 {
        let env = self.farm_env();
        crate::ai_farm::manage_neutral_fields(
            &self.tables,
            &mut self.counties,
            self.county_count,
            &mut self.campaign.map,
            &self.realms,
            market,
            &env,
        )
    }

    /// Turn phase 1, step 2, **with the county's own merchant stall attached**
    /// — which is what the original runs and what makes a lordless county able
    /// to feed itself.
    ///
    /// [`Kingdom::run_neutral_farms`] takes any [`crate::ai_farm::Market`] and
    /// stays the seam; this is the one call that supplies the real one, built
    /// from `County::merchant_count` / `merchant_unit` and the unit array
/// as `Ai_BuyGood` reads them. Returns the number of fields
    /// ordered, as [`Kingdom::run_neutral_farms`] does.
    pub fn run_neutral_farms_at_the_stall(&mut self) -> i32 {
        let env = self.farm_env();
        // The stall holds the realms mutably; the pass reads a copy. Exact for
        // the reason given at [`Kingdom::run_ai_farms_at_the_stall`], and here
        // trivially so: an unowned county's trade never touches a realm.
        let view = self.realms.clone();
        let mut market = crate::ai_farm::CountyStall::new(
            &self.tables,
            &self.counties,
            &self.campaign.units,
            &mut self.realms,
            env.season_next,
            env.armies_eat,
        );
        crate::ai_farm::manage_neutral_fields(
            &self.tables,
            &mut self.counties,
            self.county_count,
            &mut self.campaign.map,
            &view,
            &mut market,
            &env,
        )
    }

    /// AI step 5, **with the county's own merchant stall attached** —
    /// `Ai_ManageCountyFarms` (`0x0049DD01`) as the original runs it, where
    /// each realm style's opening `Ai_BuyGood` lines (`0x004A4B12`) pay out of
    /// the owning realm's treasury.
    ///
    /// Returns the number of fields ordered, as [`Kingdom::run_ai_farms`] does.
    ///
    /// # Why the pass may read a copy of the realms
    ///
    /// The stall needs the realm array mutably, to test and debit the treasury
    /// and book the spend; the pass needs it immutably, because
    /// `County_RefreshEstimates` reads the owner's record for the blacksmith's
    /// ceiling. The pass is handed a copy taken before it starts, and the copy
/// is **exact**: a grain or cattle purchase
    /// writes `gold`, `trade_spent_a` and `trade_spent_b` and nothing else on
    /// the realm, and no estimate reads any of the three (`crate::field` and
    /// `crate::industry`'s estimates read no treasury — the one `gold` in
    /// `crate::industry` is `Wages_PayAll`'s, which is a season pass).
    /// `Ai_TradeForCounty`, which *would* move wood and iron under the
    /// estimates, is not implemented; if it arrives, this reasoning has to be
/// redone.
    pub fn run_ai_farms_at_the_stall(&mut self, realm: u8) -> i32 {
        let Some(r) = self.realms.get(realm as usize) else { return 0 };
        let lord = r.lord;
        let env = self.farm_env();
        let view = self.realms.clone();
        let mut market = crate::ai_farm::CountyStall::new(
            &self.tables,
            &self.counties,
            &self.campaign.units,
            &mut self.realms,
            env.season_next,
            env.armies_eat,
        );
        crate::ai_farm::manage_county_farms(
            &self.tables,
            &mut self.counties,
            self.county_count,
            &mut self.campaign.map,
            &view,
            realm,
            lord,
            &mut market,
            &env,
        )
    }

    /// **`Ai_ManageFarmsAll` (`0x0049A990`)** — the first thing
    /// `Season_Advance` (`0x00448440`) does, ahead of `Rand_Advance` and the
    /// clock.
    ///
    /// ```c
    /// for (r = 1; r < 6; r++)
    ///     if (g_realms[r].strength != 0 && g_realms[r].isHuman == 0)
    ///         Ai_ManageCountyFarms(r);
    /// ```
    ///
    /// So an AI lord's counties are farmed **twice** a turn — once by his own
    /// step 5 in phase 4, and once here at the start of phase 7 — and the
    /// second time is ahead of tax, rations and industry, which then read the
    /// fields, the labour split and the larder it leaves. It shops at the
    /// stall like step 5 does. `g_season` is still the season that is ending
    /// when it runs, so the Winter re-sow fires here on the Winter turn.
    ///
    /// **The class is four callers of `Ai_ManageCountyFarms`, and three are
    /// reproduced.** AI step 5 and this are two. `FUN_0049DF48` is a
    /// byte-for-byte twin of this loop (tests in the other order) whose only
    /// caller is the tail of `Battle_ReturnToCampaign` (`0x004AB383`):
    /// `FUN_004AD426(); Panels_RefreshAll(); FUN_0049DF48();`. That tail is not
    /// in `crate::battle::return_to_campaign`, so an AI's farms are **not**
    /// re-managed after a battle here. Open, not cleared.
    pub fn ai_manage_farms_all(&mut self) -> i32 {
        let mut ordered = 0;
        for id in 1..MAX_REALMS {
            let realm = &self.realms[id];
            if realm.strength != 0 && !realm.is_human {
                ordered += self.run_ai_farms_at_the_stall(id as u8);
            }
        }
        ordered
    }

    /// AI step 6 — `AI_BuildCastles`.
    pub fn run_ai_castles(&mut self, realm: u8) -> Vec<u8> {
        ai::build_castles(
            &self.tables,
            &mut self.counties,
            self.county_count,
            &mut self.realms,
            realm,
        )
    }

    /// AI step 12 — the weapon rota and the industry switchboard, then the
    /// labour re-allocation and the estimate refresh the original's third loop
    /// does.
    pub fn run_ai_industry(&mut self, realm_id: u8) {
        let Some(realm) = self.realms.get(realm_id as usize) else { return };
        let mut realm = realm.clone();
        ai::choose_industry(&self.tables, &mut self.counties, self.county_count, &mut realm, realm_id);
        self.realms[realm_id as usize] = realm;
        let season_next = Season::from_index(self.season_next).unwrap_or(Season::Spring);
        for id in 1..=self.county_count {
            if self.counties[id].owner != realm_id {
                continue;
            }
            crate::labour::allocate(&mut self.counties[id]);
            // The blacksmith's ceiling is a share of the realm's stockpile
            // split across every *staffed* smithy it owns, and the allocation
            // on the line above is what staffs them — so the share is
// recomputed per county, inside the loop, as
            // `crate::field::set_type` recomputes it inside its own.
            let share = crate::industry::weapon_shares(
                &self.tables,
                &self.counties,
                self.county_count,
                realm_id,
            );
            crate::field::refresh_estimates(
                &mut self.counties[id],
                &self.campaign.map,
                season_next,
                &self.tables,
                self.options.advanced_farming,
                &self.realms[realm_id as usize],
                share,
            );
        }
    }

    /// `Diplo_Init` (`0x004A1C53`) — clear every inbox and open every realm's
    /// view of every other. **Call it once, after the realms are set up and
    /// before the first turn**, because the opening standing it writes depends
    /// on which realms are in play and which are people.
    ///
    /// `Game_NewGame` calls it in exactly that position, and this is the whole
    /// of the *"what writes it in a real game?"* answer for
    /// [`crate::realm::Pair::standing`]: nothing else puts an opening value in.
    pub fn init_diplomacy(&mut self) {
        crate::diplomacy::init(&mut self.realms, &mut self.diplomacy);
    }

    /// **AI step 1** — `Diplo_AnswerInbox`. Returns the replies.
    pub fn run_ai_inbox(&mut self, realm_id: u8) -> Vec<crate::diplomacy::Letter> {
        crate::diplomacy::answer_inbox(
            &mut self.realms,
            &mut self.diplomacy,
            &self.tables,
            realm_id,
        )
    }

    /// **AI step 2** — `AI_Diplomacy`. Returns the letters the realm sent.
    ///
    /// `g_rankLeader` is derived from the ranks [`ai::rank_realms`] wrote, the
    /// same way [`ai::rank_trailer`] is: the original keeps both as globals and
    /// deriving them is the same answer with one fewer thing to keep in step.
    pub fn run_ai_diplomacy(&mut self, realm_id: u8) -> Vec<crate::diplomacy::Letter> {
        let leader = ai::rank_leader(&self.realms);
        crate::diplomacy::ai_diplomacy(
            &mut self.realms,
            &self.tables,
            realm_id,
            self.year,
            leader,
        )
    }

    /// `Diplo_ReconcileAlliances` (`0x004A1847`) — called from `Turn_Tick`, and
    ///
    pub fn reconcile_alliances(&mut self) {
        crate::diplomacy::reconcile_alliances(&mut self.realms);
    }

    /// `Diplo_Post` — a person's letter into an AI's inbox. The player's whole
    /// outgoing side in single player, and the seam multiplayer would replace.
    pub fn post_letter(
        &mut self,
        from: u8,
        to: u8,
        kind: crate::diplomacy::Kind,
        gold: i32,
        county: u8,
    ) {
        crate::diplomacy::post(&mut self.realms, &mut self.diplomacy, from, to, kind, gold, county);
    }

    /// `Diplo_Offend` — apply one act's diplomatic damage. The four call sites
    /// are in [`crate::diplomacy::offence`]; this is where a caller holding a
    /// [`crate::movement::Offence`] or a [`crate::battle::Aftermath`] brings it.
    pub fn offend(&mut self, offended: u8, offender: u8, amount: i8) -> Vec<crate::diplomacy::Letter> {
        crate::diplomacy::offend(&mut self.realms, offended, offender, amount)
    }

    /// AI step 13 — `AI_Taunt`. Returns the letters the realm sent.
    pub fn run_ai_taunt(&mut self, realm_id: u8) -> Vec<ai::Taunt> {
        let trailer = ai::rank_trailer(&self.realms);
        let snapshot = self.realms.clone();
        let Some(realm) = self.realms.get_mut(realm_id as usize) else { return Vec::new() };
        ai::taunt(realm, realm_id, &snapshot, trailer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tables::Weather;

    #[test]
    fn a_new_kingdom_holds_seventeen_counties_and_six_realms_with_slot_zero_unused() {
        let k = Kingdom::new(1);
        assert_eq!(k.counties.len(), 17);
        assert_eq!(k.realms.len(), 6);
        assert_eq!(k.county_count, 0);
    }

    #[test]
    fn the_county_count_is_bounded_by_the_array() {
        let mut k = Kingdom::new(1);
        assert!(k.set_county_count(14), "the England map");
        assert!(k.set_county_count(16), "the last usable id");
        assert!(!k.set_county_count(17), "index 0 is not a county");
        assert_eq!(k.county_count, 16, "and a refused count changes nothing");
    }

    /// **`docs/kingdom.md` §9 point 1 and §3.3.** A new game starts in Winter
    /// 1268: `Game_NewGame` sets season 3, next 4, year 1267, next 1268, and
    /// one `Season_Advance` rolls the year.
    #[test]
    fn a_new_game_starts_in_winter_1268_on_turn_one() {
        let mut k = Kingdom::new(1);
        k.set_county_count(14);
        k.start_new_game();
        assert_eq!(k.season, 4);
        assert_eq!(k.season(), Some(Season::Winter));
        assert_eq!(k.year, 1268);
        assert_eq!(k.turn_count, 1);
        assert_eq!(k.season_next, 1, "Spring is next");
        assert_eq!(k.season_prev, 3);
    }

    /// The clock, walked for four years. The year rolls in the same call that
    /// begins Winter — see the errata note in the crate documentation, which is
    /// **not** what §3.3's prose says.
    #[test]
    fn the_year_rolls_in_the_call_that_begins_winter() {
        let mut k = Kingdom::new(1);
        k.start_new_game();
        let mut seen = vec![(k.season, k.year)];
        for _ in 0..8 {
            k.advance_season();
            seen.push((k.season, k.year));
        }
        assert_eq!(
            seen,
            vec![
                (4, 1268),
                (1, 1268),
                (2, 1268),
                (3, 1268),
                (4, 1269),
                (1, 1269),
                (2, 1269),
                (3, 1269),
                (4, 1270),
            ]
        );
    }

    #[test]
    fn the_season_cycles_one_two_three_four_forever() {
        let mut k = Kingdom::new(1);
        k.start_new_game();
        let mut last = k.season;
        for _ in 0..40 {
            k.advance_season();
            let expected = if last == 4 { 1 } else { last + 1 };
            assert_eq!(k.season, expected);
            last = k.season;
        }
    }

    #[test]
    fn a_season_runs_every_pass_in_the_documented_order() {
        let mut k = Kingdom::new(1);
        k.set_county_count(3);
        let report = k.advance_season();
        assert_eq!(report.passes, SEASON_PIPELINE.to_vec());
    }

    /// **A new game runs no phase-7 pass.** `Game_NewGame` (`0x00497CED`)
    /// calls `Season_Advance` and `Score_RankRealms`; `Mercenary_AdvanceAll`,
    /// `Units_ResetMoves` (`0x004651B9`) and `Diplo_ReconcileAlliances`
    /// (`0x004A1847`) are `Turn_Tick`'s phase-7 arm, which a game that has not
    /// started has not reached. End Turn still runs all three, and that is the
    /// second half here.
    ///
    /// Ablation: drop either name from the `matches!` in [`Kingdom::start_new_game`]
    /// — red, *"a new game ran a phase-7 pass"*.
    #[test]
    fn a_new_game_runs_no_phase_seven_pass_and_a_season_end_runs_all_three() {
        let phase7 =
            [Pass::MercenaryAdvance, Pass::UnitsResetMoves, Pass::ReconcileAlliances];
        let mut k = Kingdom::new(1);
        k.set_county_count(3);
        let new = k.start_new_game();
        for pass in phase7 {
            assert!(!new.passes.contains(&pass), "a new game ran a phase-7 pass: {pass:?}");
        }
        assert_eq!(
            new.passes.len(),
            SEASON_PIPELINE.len() - phase7.len(),
            "and it ran every other pass"
        );
        let ended = k.advance_season();
        for pass in phase7 {
            assert!(ended.passes.contains(&pass), "End Turn stopped running {pass:?}");
        }
    }

    /// The whole point of the phase machine: only phase 7 advances the season.
    #[test]
    fn only_the_seventh_phase_advances_the_season() {
        let mut k = Kingdom::new(1);
        k.set_county_count(2);
        let mut seasons = 0;
        for _ in 0..64 {
            let (tick, report) = k.tick(true);
            if report.is_some() {
                seasons += 1;
                assert_eq!(tick.phase, Phase::SeasonEnd);
            }
        }
        assert!(seasons >= 3, "several full cycles should have run");
        assert_eq!(k.turn_count as usize, seasons);
    }

    /// **`Ai_ManageFarmsAll` (`0x0049A990`) farms every realm with strength
    /// and no person behind it, at the stall, and nobody else.**
    ///
    /// Three realms each hold one county set up identically — a merchant
    /// standing in it, no grain, a herd above the grazing style's cattle floor,
    /// and 4,000 crowns in the treasury. Realm 2 is an AI lord (lord 1, style 1,
    /// whose only grain line is `if (grain < 100) buy 400`), realm 3 is a
    /// person, and realm 4 is an AI at strength 0. Only realm 2 buys.
    ///
    /// **The treasury is 4,000 because `Ai_TradeForCounty` (`0x0049E39B`) now
    /// shops first**: the Knight's floor of 1,000 is cleared, the county makes
    /// crossbows (weapon type 0, good 12) at 24 + `Pct(24, 100)` = 48, and
    /// `Ai_BuyGoodDownTo` halves 100 to 50 for 2,400. That leaves exactly the
    /// 1,600 the grain lot costs, so both orders land and the test still says
    /// what it said.
    ///
    /// *Ablation*: empty `ai_manage_farms_all`'s loop and the first assertion
    /// goes red; drop its `!realm.is_human` and the realm-3 one does; drop the
    /// `strength != 0` and the realm-4 one does.
    #[test]
    fn the_season_head_farms_ai_realms_with_strength_and_leaves_the_rest() {
        let mut k = Kingdom::new(1);
        assert!(k.set_county_count(3));
        for (id, realm, human, strength) in [(1usize, 2usize, false, 1u8), (2, 3, true, 1), (3, 4, false, 0)] {
            let r = &mut k.realms[realm];
            r.in_play = true;
            r.strength = strength;
            r.is_human = human;
            r.lord = 1;
            r.gold = 4_000;
            let c = &mut k.counties[id];
            c.owner = realm as u8;
            c.population = 500;
            c.grain = 0;
            c.herd = 50;
            c.merchant_count = 1;
            c.merchant_unit = id as u8;
            let mut m = crate::unit::Unit::new(crate::unit::UnitKind::Merchant, 6, 8, 8);
            m.morale = 100;
            m.county = id as u8;
            k.campaign.units.put(id, m);
        }
        assert_eq!(k.tables.ai_farm_style(1), Some(1), "lord 1 grazes");

        k.ai_manage_farms_all();

        assert_eq!((k.counties[1].grain, k.realms[2].gold), (400, 0), "the AI lord buys 50 crossbows for 2,400 and 400 sacks for 1,600");
        assert_eq!(k.realms[2].weapons[0], 50);
        assert_eq!(k.realms[2].trade_spent_a, 4_000);
        assert_eq!((k.counties[2].grain, k.realms[3].gold), (0, 4_000), "a persons realm is not farmed");
        assert_eq!((k.counties[3].grain, k.realms[4].gold), (0, 4_000), "a realm at strength 0 is not farmed");
    }

    /// The determinism property lockstep depends on. Two kingdoms built the
    /// same way and driven the same way must stay bit-identical, generator
    /// included.
    #[test]
    fn two_identical_kingdoms_stay_identical_for_forty_seasons() {
        let build = || {
            let mut k = Kingdom::new(0xA11CE);
            k.options = Options { difficulty: 2, advanced_farming: true, armies_eat: true, ..Options::default() };
            k.set_county_count(14);
            for id in 1..=5 {
                k.realms[id].in_play = true;
                k.realms[id].lord = (id - 1) as u8;
            }
            k.realms[1].is_human = true;
            for id in 1..=14 {
                let owner = if id <= 4 { 1 } else if id <= 8 { 2 } else { 0 };
                let c = &mut k.counties[id];
                c.owner = owner;
                c.population = 400 + id as i32 * 7;
                c.happiness = 60;
                c.health_meter = 65;
                c.health_band = crate::tables::health_band(65);
                c.herd = 60;
                c.grain = 300;
                c.fields_grain = 6;
                c.fields_fallow = 3;
                c.labour[crate::tables::JOB_GRAIN_FARMING] = 500;
                c.labour[crate::tables::JOB_WOOD_CUTTING] = 100;
                c.castle_type = crate::tables::CASTLE_STARTING_TYPE;
                c.tax_rate = 7;
                c.dryness = 40;
                for n in 1..=14u8 {
                    if n as usize != id {
                        c.add_neighbour(n);
                    }
                }
            }
            k.start_new_game();
            k
        };

        let (mut a, mut b) = (build(), build());
        for season in 0..40 {
            let ra = a.advance_season();
            let rb = b.advance_season();
            assert_eq!(ra, rb, "reports diverged at season {season}");
            assert_eq!(a, b, "state diverged at season {season}");
        }
    }

    /// A kingdom with no counties still keeps its clock, and never panics.
    #[test]
    fn an_empty_kingdom_advances_its_clock_and_nothing_else() {
        let mut k = Kingdom::new(1);
        k.start_new_game();
        for _ in 0..20 {
            let report = k.advance_season();
            assert!(report.messages.is_empty());
        }
        assert_eq!(k.turn_count, 21);
    }

    /// **`docs/kingdom.md` §9 point 3.** With Advanced Farming off, every
    /// county ends the season Cloudy with zero fertility — the two overrides
    /// §7.2 and §7.3 say the option forces.
    #[test]
    fn basic_farming_forces_cloudy_and_zero_fertility_across_the_map() {
        let mut k = Kingdom::new(1);
        k.set_county_count(14);
        for id in 1..=14 {
            k.counties[id].fields_fallow = 5;
            k.counties[id].fields_grain = 1;
            k.counties[id].dryness = 100;
        }
        k.start_new_game();
        for id in 1..=14 {
            assert_eq!(k.counties[id].weather, Weather::Cloudy, "county {id}");
            assert_eq!(k.counties[id].fertility, 0, "county {id}");
        }
    }

    #[test]
    fn tax_is_collected_into_the_owning_realm_and_nowhere_else() {
        let mut k = Kingdom::new(1);
        k.set_county_count(3);
        k.realms[1].in_play = true;
        k.counties[1].owner = 1;
        k.counties[1].population = 1000;
        k.counties[1].tax_rate = 10;
        k.counties[2].owner = 0; // unowned
        k.counties[2].population = 1000;
        k.counties[2].tax_rate = 10;

        let mut report = SeasonReport::new();
        k.run_pass(Pass::TaxCollect, &mut report);
        assert_eq!(k.realms[1].gold, 320);
        assert_eq!(k.counties[2].tax_collected, 320, "computed but banked nowhere");
        assert_eq!(k.realms[0].gold, 0, "realm 0 is not a realm");
    }

    // --- the history ring --------------------------------------------------

    /// The ring's shape, and the arithmetic that pins it: save block 10 is
    /// 51,200 bytes, and `400 * 16 * 8` is 51,200.
    #[test]
    fn the_history_ring_is_four_hundred_seasons_of_sixteen_counties() {
        assert_eq!(crate::tables::HISTORY_SEASONS, 400);
        assert_eq!(crate::tables::HISTORY_COUNTIES, 16);
        assert_eq!(
            crate::tables::HISTORY_SEASONS * crate::tables::HISTORY_COUNTIES * 8,
            51_200,
            "save block 10 at 0x0056D8C0"
        );
        assert_eq!(
            crate::tables::HISTORY_COUNTIES,
            crate::county::MAX_COUNTY_ID as usize,
            "one slot per addressable county"
        );
    }

    #[test]
    fn a_season_writes_one_line_per_county_and_the_ring_reads_back_in_order() {
        let mut k = Kingdom::new(1);
        k.set_county_count(3);
        assert!(k.history.is_empty());
        for season in 1..=5i32 {
            for id in 1..=3 {
                k.counties[id].population = 100 * season + id as i32;
                k.counties[id].happiness = 40 + season;
            }
            k.history.record(&k.counties.clone());
        }
        assert_eq!(k.history.len(), 5);

        let county_two = k.history.county(2);
        assert_eq!(county_two.len(), 5);
        assert_eq!(
            county_two.iter().map(|e| e.population).collect::<Vec<i32>>(),
            vec![102, 202, 302, 402, 502],
            "oldest first"
        );
        assert_eq!(county_two[4].happiness, 45);

        let latest = k.history.latest().expect("five seasons recorded");
        assert_eq!(latest[1].population, 502, "county 2 is slot 1");
    }

    /// **Counties 1..=16 unconditionally**, not `1..=g_countyCount`. A map with
    /// four counties still writes sixteen lines, twelve of them zero.
    #[test]
    fn the_ring_writes_every_slot_whatever_the_map_holds() {
        let mut k = Kingdom::new(2);
        k.set_county_count(4);
        for id in 1..=4 {
            k.counties[id].population = 500;
        }
        k.history.record(&k.counties.clone());
        let latest = *k.history.latest().expect("one season");
        for slot in 0..4 {
            assert_eq!(latest[slot].population, 500);
        }
        for slot in 4..crate::tables::HISTORY_COUNTIES {
            assert_eq!(latest[slot].population, 0, "slot {slot} is an empty record");
        }
    }

    /// A hundred years in, the ring is full and starts forgetting.
    #[test]
    fn the_ring_forgets_its_beginning_after_four_hundred_seasons() {
        let mut k = Kingdom::new(3);
        k.set_county_count(1);
        for season in 1..=(crate::tables::HISTORY_SEASONS as i32 + 50) {
            k.counties[1].population = season;
            k.history.record(&k.counties.clone());
        }
        assert_eq!(k.history.len(), crate::tables::HISTORY_SEASONS);
        let held = k.history.county(1);
        assert_eq!(held.len(), crate::tables::HISTORY_SEASONS);
        assert_eq!(held[0].population, 51, "the first fifty seasons are gone");
        assert_eq!(held[held.len() - 1].population, 450);
    }

    #[test]
    fn asking_the_ring_for_a_county_it_does_not_hold_gives_nothing() {
        let k = Kingdom::new(4);
        assert!(k.history.county(0).is_empty());
        assert!(k.history.county(crate::tables::HISTORY_COUNTIES + 1).is_empty());
        assert!(k.history.latest().is_none(), "and nothing before the first season");
    }

    /// The ring is written by the season driver, not only by hand.
    #[test]
    fn advancing_a_season_records_a_line_in_the_ring() {
        let mut k = Kingdom::new(5);
        k.set_county_count(2);
        k.counties[1].population = 400;
        k.start_new_game();
        assert_eq!(k.history.len(), 1, "one season, one line");
        k.advance_season();
        assert_eq!(k.history.len(), 2);
    }


    // --- the tax rate -------------------------------------------------------

    /// **`Tax_RecomputePreview` writes three fields and the panel draws all
    /// three**, so
    /// panel stale. A player reported both halves of that in one sentence:
    /// *"'People pay 0 crowns' on the tax thing always says 0 crowns. And the
    /// happiness bonus/minus on the tax screen is also stuck."*
    ///
    /// `tax_shown` had exactly one writer, [`crate::tax::collect`], which runs
    /// once a season — so before the first collection it is zero and after it it
    /// describes last season's rate.
    #[test]
    fn moving_the_tax_rate_moves_what_people_pay_and_the_local_happiness() {
        let mut k = Kingdom::new(21);
        k.set_county_count(2);
        k.realms[1].in_play = true;
        {
            let c = &mut k.counties[1];
            c.owner = 1;
            c.population = 1000;
        }
        k.set_tax_rate(1, 0);
        assert_eq!(k.counties[1].tax_shown, 0, "nobody pays anything at a rate of nothing");

        k.set_tax_rate(1, 20);
        let paid = k.counties[1].tax_shown;
        assert!(paid > 0, "at a fifth, a thousand people pay something");
        assert_eq!(
            k.counties[1].d_hap_tax_local, 5 - 20,
            "5 - rate, and it moves on every click",
        );

        k.set_tax_rate(1, 40);
        assert!(k.counties[1].tax_shown > paid, "and twice the rate is more crowns");
        assert_eq!(k.counties[1].d_hap_tax_local, 5 - 40);
        // Ablation: drop the `tax_shown` line from `tax::recompute_preview` and
        // the second and fourth assertions fail with 0.
    }

    /// **The *Other counties* line really is stuck, and the panel is right.**
    ///
    /// `taxHapOther` is `g_taxHappinessOther[rate]`, a table, and the table is
    /// flat zero from 0 to 19. Every county in every fixture sits at rate 0 and
    /// the highest an AI reaches in a hundred turns is 12, so **most of this
    /// mechanic is human-only and no run of ours exercises it** —
    /// `docs/decisions.md` C26.
    ///
    /// This is the half of the player's report that is not a defect, and it is
    /// asserted so that nobody "fixes" it later.
    #[test]
    fn the_empire_tax_happiness_term_is_flat_until_the_rate_reaches_twenty() {
        let mut k = Kingdom::new(22);
        k.set_county_count(2);
        k.realms[1].in_play = true;
        k.counties[1].owner = 1;
        k.counties[1].population = 1000;
        for rate in 0..20 {
            k.set_tax_rate(1, rate);
            assert_eq!(
                k.counties[1].tax_hap_other, 0,
                "rate {rate} is inside the flat part of g_taxHappinessOther",
            );
        }
        k.set_tax_rate(1, 20);
        assert_ne!(
            k.counties[1].tax_hap_other, 0,
            "and twenty is where the table finally moves",
        );
    }

    /// **`taxShown` ignores suppression and `taxCollected` does not**, which is
    /// the whole of how the two fields differ — `docs/kingdom.md` §1.3 lists
    /// them side by side and says it does not know.
    ///
    /// `Tax_RecomputePreview` has no suppression test in it; `Tax_Collect`
    /// zeroes the base. So
    /// his people *would* pay while the treasury banks nothing. `[D]`.
    #[test]
    fn a_suppressed_county_still_shows_what_people_would_pay() {
        let mut k = Kingdom::new(23);
        k.set_county_count(2);
        k.realms[1].in_play = true;
        {
            let c = &mut k.counties[1];
            c.owner = 1;
            c.population = 1000;
            c.tax_suppressed = true;
        }
        k.set_tax_rate(1, 30);
        assert!(k.counties[1].tax_shown > 0, "the panel shows the rate's worth");
        let banked = crate::tax::collect(&k.tables, &mut k.counties[1], 0);
        assert_eq!(banked, 0, "and the treasury gets none of it");
        assert_eq!(k.counties[1].tax_collected, 0);
    }

    // --- the ration split ---------------------------------------------------


    /// **The third control on the ration panel, found by enumerating the class
/// `Ration_IncreaseCounty` (`0x0043A23F`) is
    /// `rationWanted++`, `Ration_Apply`, `County_RefreshEstimates`,
    /// `Panel_Ration` — so asking for more food changes what the county is
    /// recorded as eating, on the spot.
    ///
    /// Ours wrote `ration_wanted` and returned, like the split slider and like
    /// the tax arrows before it.
    #[test]
    fn asking_for_more_food_changes_what_the_county_eats_at_once() {
        let mut k = Kingdom::new(31);
        k.set_county_count(2);
        k.realms[1].in_play = true;
        {
            let c = &mut k.counties[1];
            c.owner = 1;
            c.population = 2000;
            c.pop_band = 80;
            c.herd = 100;
            c.grain = 2000;
            c.ration_split = 0; // all grain, so the level alone moves the number
            c.ration_wanted = 1;
        }
        k.set_ration_wanted(1, 1);
        let (level, sacks) = (k.counties[1].ration_achieved, k.counties[1].grain_eaten);

        k.set_ration_wanted(1, 5);
        assert!(k.counties[1].ration_achieved > level, "the county can afford more and takes it");
        assert!(
            k.counties[1].grain_eaten > sacks,
            "and the Eaten row moves with it: {} was {sacks}",
            k.counties[1].grain_eaten,
        );
        // Ablation: drop the `ration::preview` call from `set_ration_wanted`
        // and both comparisons collapse to equality.
    }

    /// The cap is the table's length and the recompute is **outside** the
/// guard: a click at the top still re-applies, as
    /// `if (rationWanted < 5) rationWanted++;` followed by an unconditional
    /// `Ration_Apply` says.
    #[test]
    fn a_click_at_the_top_of_the_ration_scale_still_recomputes() {
        let mut k = Kingdom::new(32);
        k.set_county_count(2);
        k.realms[1].in_play = true;
        {
            let c = &mut k.counties[1];
            c.owner = 1;
            c.population = 500;
            c.grain = 500;
            c.ration_wanted = 5;
        }
        // A sentinel no pass would ever leave, so the assertion is about the
// pass having run.
        k.counties[1].ration_achieved = -7;
        assert!(!k.set_ration_wanted(1, 9), "the level did not move");
        assert_eq!(k.counties[1].ration_wanted, 5, "and is clamped to the table");
        assert_ne!(
            k.counties[1].ration_achieved, -7,
            "but the pass ran anyway, because the guard is only on the increment",
        );
    }

    /// It writes what the player asked for and never what the county managed.
    #[test]
    fn the_ration_control_writes_wanted_and_the_pass_writes_achieved() {
        let mut k = Kingdom::new(33);
        k.set_county_count(2);
        k.realms[1].in_play = true;
        {
            let c = &mut k.counties[1];
            c.owner = 1;
            c.population = 1000;
            c.herd = 0;
            c.grain = 0; // nothing in store at all
        }
        k.set_ration_wanted(1, 5);
        assert_eq!(k.counties[1].ration_wanted, 5, "he asked for triple");
        assert_eq!(k.counties[1].ration_achieved, 0, "and the county feeds nobody");
    }

    /// A county that eats some of its herd and some of its grain, which is the
    /// only state in which the slider's search does anything at all.
    fn a_county_that_eats_both() -> Kingdom {
        let mut k = Kingdom::new(9);
        k.set_county_count(2);
        k.realms[1].in_play = true;
        let c = &mut k.counties[1];
        c.owner = 1;
        c.population = 2000;
        c.pop_band = 80;
        // The standing herd feeds five people a head for free, so it has to be
        // small enough that there is a requirement left to split.
        c.herd = 100;
        c.grain = 900;
        c.ration_wanted = 3;
        c.ration_split = 50;
        k
    }

    /// **The write is not the behaviour.** `Ration_SetSplit` runs the food pass
    /// on the spot, so the numbers the panel prints move with the slider — and
    /// a slider whose effect is invisible is what a player reported as *"moves
    /// but is inoperable"*.
    ///
    /// The old test asserted `ration_split == 37` and passed, and the slider was
    /// broken the whole time: it was checking the field the gesture writes, and
    /// the defect was the absence of everything after the write.
    #[test]
    fn moving_the_split_moves_the_numbers_the_panel_prints() {
        let mut k = a_county_that_eats_both();
        k.set_ration_split(1, 100, true);
        let (herd_all, grain_all) = (k.counties[1].herd_eaten, k.counties[1].grain_eaten);

        k.set_ration_split(1, 0, true);
        let (herd_none, grain_none) = (k.counties[1].herd_eaten, k.counties[1].grain_eaten);

        assert!(herd_all > 0, "all-livestock eats the herd");
        assert_eq!(herd_none, 0, "all-grain eats none of it");
        assert!(grain_none > grain_all, "and the grain takes the whole requirement instead");
        // Ablation: delete the `ration::preview` call in `set_ration_split` and
        // every one of these is whatever the last season left, so all four
        // comparisons collapse.
    }

    /// **The store is not touched.** `Ration_Apply` computes and records; the
    /// season spends. A drag runs it up to a hundred times, so if this were
    /// [`crate::ration::apply`] the county would be eaten alive by its own
    /// slider.
    #[test]
    fn dragging_the_split_does_not_feed_anybody() {
        let mut k = a_county_that_eats_both();
        let (herd, grain) = (k.counties[1].herd, k.counties[1].grain);
        for split in 0..=100 {
            k.set_ration_split(1, split, true);
        }
        assert_eq!((k.counties[1].herd, k.counties[1].grain), (herd, grain));
    }

    /// **A track jump that changes nothing springs back**, and an arrow does
    /// not. The two gestures differ only in `sweep`, and a player can see it.
    ///
    /// The search runs when the county has a herd, is eating some of it, the
    /// value moved, the request is strictly inside 0…100, and `herdEaten` came
    /// out unchanged. It then walks one point at a time from the old value
    /// towards the request looking for a split that moves `herdEaten`.
    ///
    /// * with `sweep`, reaching the request having found nothing **restores the
    ///   old split**;
    /// * without it, `(rationSplit != split) || (sweep == 0)` never terminates
    ///   the walk at the request, so it carries on in the same direction — one
    ///   click of an arrow can move the split a long way, and it does not spring
    ///   back.
    #[test]
    fn a_track_jump_springs_back_where_an_arrow_keeps_walking() {
        // Chosen so that one point of split is below the rounding: thirty
        // people, two head of dairy feeding ten of them, so twenty people-worth
        // left to split. `pct(20, 50)` and `pct(20, 51)` are both 10, and ten
        // people-worth is one head either way — so
        // nothing and the search is forced to run. Without that the guard
        // `herd_eaten != 0` is false, the search never fires, and this test
        // passes while asserting nothing, which is what its first draft did.
        let mut k = Kingdom::new(11);
        k.set_county_count(2);
        k.realms[1].in_play = true;
        {
            let c = &mut k.counties[1];
            c.owner = 1;
            c.population = 30;
            c.pop_band = 2;
            c.herd = 2;
            c.grain = 100;
            c.ration_wanted = 3;
            c.ration_split = 50;
        }
        k.set_ration_split(1, 50, true); // settle herd_eaten for where we start
        let settled = k.counties[1].ration_split;
        let eaten = k.counties[1].herd_eaten;
        assert_eq!(settled, 50);
        assert!(eaten > 0, "the search only runs on a county that is eating its herd");

        let mut track = k.clone();
        let mut arrow = k.clone();
        track.set_ration_split(1, settled + 1, true);
        arrow.set_ration_split(1, settled + 1, false);

        assert_eq!(
            track.counties[1].ration_split, settled,
            "a track jump that finds no split worth having puts the old one back",
        );
        assert_eq!(track.counties[1].herd_eaten, eaten, "and the numbers with it");
        assert!(
            arrow.counties[1].ration_split > settled + 1,
            "an arrow does not stop at the request: it walks on until the herd moves, and \
             ended at {} rather than past {}",
            arrow.counties[1].ration_split,
            settled + 1,
        );
        assert_ne!(
            arrow.counties[1].herd_eaten, eaten,
            "and it stops at the first split that changes something",
        );
    }


    /// The search is bounded, and the bound is the original's hundred steps.
    /// Nothing here may loop for ever on a county whose herd never moves.
    #[test]
    fn the_search_terminates_on_a_county_whose_herd_never_changes() {
        let mut k = Kingdom::new(12);
        k.set_county_count(2);
        k.realms[1].in_play = true;
        {
            let c = &mut k.counties[1];
            c.owner = 1;
            c.population = 100;
            c.pop_band = 4;
            c.herd = 1;
            c.grain = 1000;
            c.ration_wanted = 3;
            c.ration_split = 40;
        }
        k.set_ration_split(1, 60, true);
        assert!((0..=100).contains(&k.counties[1].ration_split));
        k.set_ration_split(1, 20, false);
        assert!((0..=100).contains(&k.counties[1].ration_split));
    }

    /// Another realm's county is refused, and
    /// than the screen's: `Ration_SliderClick` opens
    /// `if (counties[sel].owner != g_localPlayer) return 0;`, and
    /// `Game::set_ration_split` is the gate here.
    #[test]
    fn the_split_of_a_county_out_of_range_is_refused() {
        let mut k = a_county_that_eats_both();
        assert!(!k.set_ration_split(0, 50, true), "county 0 is not a county");
        assert!(!k.set_ration_split(99, 50, true), "and neither is one past the count");
    }
}
