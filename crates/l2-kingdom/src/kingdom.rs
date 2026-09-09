//! The kingdom — the whole state, and the driver that walks
//! [`SEASON_PIPELINE`] over it.
//!
//! `Season_Advance` (`0x00448440`) calls 29 functions in a fixed order, and
//! `docs/kingdom.md` §3.4 is explicit that **the order is the rule**. So the
//! driver here is a loop over an array rather than a sequence of statements:
//! [`Kingdom::advance_season`] walks [`SEASON_PIPELINE`] and records what it
//! ran, and the ordering assertions live in tests rather than in the reader's
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
//! set to the season *about to begin* **before** the economy runs. So a rule
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

/// The three game options `docs/kingdom.md` §9 reads out of the England turn-one fixture,
/// and the only ones any rule in this crate branches on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// `g_optDifficulty`, 0..=2 in the England turn-one fixture (0), and 0..=3 in the
    /// AI grant tables. Only the AI's wages and grants read it.
    pub difficulty: u8,
    /// `g_optAdvancedFarming`. Off in the England turn-one fixture, which is why every
    /// county there is Cloudy with zero fertility.
    pub advanced_farming: bool,
    /// `g_optArmiesEat`. Off in the England turn-one fixture.
    pub armies_eat: bool,
}

impl Default for Options {
    fn default() -> Self {
        // The shipped lastturn.sav's settings (docs/kingdom.md §9).
        Options { difficulty: 0, advanced_farming: false, armies_eat: false }
    }
}

/// A whole kingdom.
///
/// The two arrays are fixed size and 1-based, exactly as the original's are:
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
    /// It is *inside* the kingdom rather than beside it because it is
    /// simulation state in exactly the sense `docs/netcode.md` §5 means: two
    /// lockstep peers have to agree about where an army stands and which tiles
    /// it has ruined, the tick checksum has to cover it, and a save that
    /// dropped it would keep the economy and lose the war. See [`Campaign`].
    pub campaign: Campaign,
    /// **The ruleset this kingdom runs on.**
    ///
    /// Fixed for the life of the kingdom, like `l2_sim::Battle`'s troop
    /// table: every rule function in this crate takes `&Tables` and reads it
    /// rather than a module constant, so whatever table built this kingdom is
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
/// block, so the whole structure is confirmed by the save layout rather than
/// only by the code.
///
/// Once the ring is full the head keeps advancing and the tail follows it, so a
/// game longer than 400 seasons — a hundred years — silently forgets its
/// beginning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct History {
    /// `[season][county - 1]`, oldest at the tail.
    ///
    /// Crate-visible rather than private so [`crate::save`] can write the ring
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
                // here, but it is done rather than widened so a caller cannot
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
/// rather than as fourteen fields.
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
        }
    }

    /// `Game_NewGame` (`0x00497CED`)'s clock constants, then one
    /// [`Kingdom::advance_season`].
    ///
    /// **A new game starts in Winter 1268.** The shipped `lastturn.sav` reads
    /// back `g_season = 4`, `g_year = 1268`, `g_turnCount = 1`, and this is the
    /// arithmetic that produces them.
    pub fn start_new_game(&mut self) -> SeasonReport {
        self.season = 3;
        self.season_next = 4;
        self.year = 1267;
        self.year_next = 1268;
        self.turn_count = 0;
        self.advance_season()
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
            Pass::ScoreRank => ai::rank_realms(&self.tables, &mut self.realms),
            Pass::History => self.history(),
            Pass::RationPreview => self.ration_apply(true),
            Pass::RefreshEstimates => self.refresh_estimates_all(),
            Pass::MercenaryAdvance => self.mercenary_advance(),
            Pass::UnitsResetMoves => self.units_reset_moves(),
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
        );
        for id in 1..=self.county_count {
            let owner = self.counties[id].owner as usize;
            let empire = if owner != 0 && owner < MAX_REALMS {
                self.realms[owner].tax_hap_empire as i32
            } else {
                0
            };
            let take = tax::collect(&self.tables, &mut self.counties[id], empire);
            if owner != 0 && owner < MAX_REALMS {
                self.realms[owner].gold += take;
            }
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
    /// armies rather than on the counter.
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
        self.campaign.mercenaries.advance(&mut self.counties, count);
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

    fn unrest_update(&mut self, report: &mut SeasonReport) {
        for id in 1..=self.county_count {
            let human = self.owner_is_human(self.counties[id].owner);
            let mut messages = Vec::new();
            unrest::update(&mut self.counties[id], id as u8, human, &mut messages);
            for m in messages {
                report.message(m);
            }
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
        for id in 1..=self.county_count {
            land::grain_season_tick(&self.tables, &mut self.counties[id], season, advanced);
            if let Some(ceiling) = land::grain_labour_estimate(
                &self.tables,
                &self.counties[id],
                season_next,
                advanced,
            ) {
                self.counties[id].labour_wanted[crate::tables::JOB_GRAIN_FARMING] = ceiling;
                self.counties[id].labour_useful[crate::tables::JOB_GRAIN_FARMING] = ceiling;
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
            if self.counties[id].pop_band != 0 {
                self.counties[id].labour_useful[crate::tables::JOB_CATTLE_FARMING] =
                    land::herd_labour_estimate(
                        &self.tables,
                        &self.counties[id],
                        self.season_next,
                    );
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

    /// `County_MakeIndependent` (`0x004AC3C6`) — **the common ending of every
    /// way a county stops being owned**: secession, revolt, and the elimination
    /// of a realm all call it.
    ///
    /// ```c
    /// owner = 0; shieldIndex = 0;
    /// for (i = 0; i < 4; i++) industry[i].enabled = 0;
    /// castleSwitch = 0;
    /// Labour_Allocate(county); Ration_Apply(county, season);
    /// County_RefreshEstimates(county, seasonNext); Tax_RecomputePreview(county);
    /// if (garrison) handOverGarrison(garrison, county);
    /// ```
    ///
    /// **Switching all four industries off is the mechanism, not a flourish.**
    /// It is what turns the county's four industry ceilings to zero, and the
    /// re-allocation two lines later is what moves those people into *Idle
    /// townsfolk* — the difference between the shipped save's
    /// `[0, 218, 0, 0, 0, 0, 217, 0, 0]` and its `[0, 323, 0, 0, 0, 0, 0, 0,
    /// 133]`.
    ///
    /// County `+0x07`, the owner's shield byte, is presentation and this crate
    /// has no such field; the garrison hand-off is `FUN_00437535`, which lives
    /// in the unit layer and is left to the caller.
    pub fn make_county_independent(&mut self, county: usize) {
        if county == 0 || county > self.county_count {
            return;
        }
        let armies_eat = self.options.armies_eat;
        {
            let c = &mut self.counties[county];
            c.owner = 0;
            for industry in c.industry.iter_mut() {
                industry.enabled = false;
            }
            c.castle_switch = false;
        }
        crate::labour::allocate(&mut self.counties[county]);
        crate::ration::apply(&self.tables, &mut self.counties[county], armies_eat);
        self.refresh_estimates(county);
        crate::tax::recompute_preview(&self.tables, &mut self.counties[county]);
    }

    /// One `Industry_Produce` run over every county.
    ///
    /// The blacksmith's stockpile share ([`industry::weapon_shares`],
    /// `FUN_0044F15B`) is computed **per realm, before the loop**. The original
    /// recomputes it inside every `resourceLimit` call, which is the same
    /// answer: its inputs are which smithies are switched on and staffed, and
    /// the pass changes neither.
    fn industry(&mut self, commodity: Commodity) {
        let shares: [industry::WeaponShare; MAX_REALMS] = core::array::from_fn(|realm| {
            industry::weapon_shares(&self.tables, &self.counties, self.county_count, realm as u8)
        });
        let (counties, realms, tables) = (&mut self.counties, &mut self.realms, &self.tables);
        for id in 1..=self.county_count {
            let owner = counties[id].owner as usize;
            // An unowned county has no realm to credit, so it produces nothing.
            if owner == 0 || owner >= MAX_REALMS {
                continue;
            }
            industry::produce_with_share(
                tables,
                &mut counties[id],
                &mut realms[owner],
                commodity,
                self.options.advanced_farming,
                shares[owner],
            );
        }
    }

    fn castle_build_tick(&mut self, report: &mut SeasonReport) {
        for id in 1..=self.county_count {
            let mut messages = Vec::new();
            industry::build_tick(&self.tables, &mut self.counties[id], id as u8, &mut messages);
            for m in messages {
                report.message(m);
            }
        }
    }

    fn migration_update(&mut self) {
        population::migrate_all(&mut self.counties, self.county_count);
    }

    fn population_update(&mut self) {
        let Some(season) = self.season() else { return };
        population::update_all(&self.tables, &mut self.counties, self.county_count, season);
    }

    /// The history ring — `FUN_004AE7DD`. See [`History`].
    fn history(&mut self) {
        self.history.record(&self.counties);
    }

    /// `AI_SetTaxRates`' first half — set every county `realm` owns to the
    /// rate its happiness earns on that realm's ladder.
    ///
    /// Like [`Kingdom::run_ai_grants`] this runs in the AI's turn rather than
    /// in `Season_Advance`, so it is exposed rather than being a `Pass`.
    /// **Realm 0 is the unowned counties**, which the original taxes once a
    /// turn in phase 1 on the neutral ladder. An out-of-range realm index does
    /// nothing rather than panicking, because the caller is a turn machine and
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
    /// Every caller in this crate goes through here rather than assembling the
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
    /// The original also runs `Ration_Apply` between its two allocations and
    /// four estimate refreshes around them; this runs the three estimates the
    /// crate has (see [`crate::field::refresh_estimates`]) and leaves the
    /// ration pass to the season, which is the same gap
    /// [`crate::field::set_type`] documents.
    pub fn toggle_industry(&mut self, county: usize, what: crate::industry::MapToggle) -> bool {
        if county == 0 || county > self.county_count {
            return false;
        }
        let on = crate::industry::toggle_from_map(&mut self.counties[county], what);
        for _ in 0..2 {
            crate::labour::allocate(&mut self.counties[county]);
            self.refresh_estimates(county);
        }
        on
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

    /// The determinism property lockstep depends on. Two kingdoms built the
    /// same way and driven the same way must stay bit-identical, generator
    /// included.
    #[test]
    fn two_identical_kingdoms_stay_identical_for_forty_seasons() {
        let build = || {
            let mut k = Kingdom::new(0xA11CE);
            k.options = Options { difficulty: 2, advanced_farming: true, armies_eat: true };
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
}
