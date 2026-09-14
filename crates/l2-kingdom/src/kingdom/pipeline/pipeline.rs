#![allow(unused_imports)]
use super::*;

use super::*;
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
    /// `realm`
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

    /// Whether a realm index is a person. Realm 0 is not a realm
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
            Pass::Weather => self.weather(report),
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

    fn weather(&mut self, report: &mut SeasonReport) {
        let Some(season) = self.season() else { return };
        // `Msg_Enqueue`'s own filter is `to == 0 || to == g_localPlayer`; this
        // crate has no local player and every other rule asks the same
        // question of the realm record instead.
        let humans: [bool; MAX_REALMS] = core::array::from_fn(|i| self.realms[i].is_human);
        let owner_is_human =
            move |owner: u8| owner != 0 && (owner as usize) < MAX_REALMS && humans[owner as usize];
        let advanced_farming = self.options.advanced_farming;
        let county_count = self.county_count;
        let Kingdom { tables, counties, campaign, rng, weather_county, .. } = self;
        weather::update_all(
            tables,
            counties,
            county_count,
            &mut campaign.map,
            season,
            advanced_farming,
            rng,
            weather_county,
            &owner_is_human,
            &mut report.messages,
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
    /// mercenaries. Three things happen here
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
            let action = industry::pay(
                &mut self.realms[id],
                id as u8,
                had_mercenaries,
                &mut report.messages,
            );
            self.apply_bankruptcy(id as u8, action);
        }
    }

    /// The half of `docs/kingdom.md` §7.4's bankruptcy ladder that acts on
/// armies.
    ///
    /// [`industry::pay`] advances the stage and reports it; what the stage
    /// *does* needs the unit array, so it happens here.
    ///
    /// **It is keyed on the action, not on the counter.** The counter is the
    /// *next* stage, and `Wages_PayAll` wraps it to 0 after the mutiny, so two
    /// rungs of the six were unreachable through it: the fourth desertion, and
    /// `Realm_DestroyArmies` itself.
    fn apply_bankruptcy(&mut self, realm: u8, action: industry::BankruptcyAction) {
        use industry::BankruptcyAction as A;
        match action {
            // `Realm_ReleaseMercenaries` (`0x004AD230`) — stage 0, and it runs
            // whichever message the release picked.
            A::MercenariesDesert | A::Warned => {
                self.campaign.mercenaries.release_realm(&mut self.campaign.units, realm);
            }
            // `Realm_DesertArmies` (`0x004AD0E8`) — stages 1..=4, the four
            // seasons `L2.eng` 271 calls "over a year".
            A::Desertion | A::LastWarning => {
                for (_, u) in self.campaign.units.iter_mut() {
                    if u.owner == realm && u.kind == crate::unit::UnitKind::Army {
                        u.desert();
                    }
                }
            }
            // `Realm_DestroyArmies` (`0x004AD316`) — stage 5, `Army_Destroy` on
            // every army the realm has. `L2.eng` 272.
            A::Mutiny => {
                let ids: Vec<usize> = self
                    .campaign
                    .units
                    .iter()
                    .filter(|(_, u)| u.owner == realm && u.kind == crate::unit::UnitKind::Army)
                    .map(|(id, _)| id)
                    .collect();
                for id in ids {
                    crate::unit::destroy(
                        &self.tables,
                        &mut self.campaign.units,
                        &mut self.realms,
                        &mut self.campaign.names,
                        id,
                        self.options.difficulty,
                    );
                }
            }
            A::None => {}
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
            // The estimate is the pass's own tail call
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
    /// **Not here
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
            // test
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
    /// finished castle comes with
    fn castle_build_tick(&mut self, report: &mut SeasonReport) {
        // **`Castle_BuildTick`'s first loop
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

}

