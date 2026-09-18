#![allow(unused_imports)]
use super::*;

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
    pub fn new(seed: u64) -> Kingdom {
        Kingdom::with_tables(seed, Tables::DEFAULT)
    }

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
            // `Game_NewGame` (`0x00497CED`) calls `PlayerStart_Shuffle`
            // (`0x00497E65`), whose second half deals `DAT_0057CAE0`.
            campaign: {
                let mut campaign = Campaign::new();
                campaign.field_playlist = crate::field_playlist::FieldPlaylist::deal(seed);
                campaign
            },
            diplomacy: crate::diplomacy::Diplomacy::new(seed),
        }
    }

    /// `Game_NewGame` (`0x00497CED`)'s clock constants, then one
    /// [`Kingdom::advance_season`].
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
            //
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
    pub fn tax_expected(&self, realm: u8) -> i32 {
        if realm == 0 {
            return 0;
        }
        self.county_ids()
            .filter(|&id| self.counties[id].owner == realm)
            .map(|id| self.counties[id].tax_shown)
            .sum()
    }

    pub fn set_county_count(&mut self, count: usize) -> bool {
        if count > MAX_COUNTY_ID as usize {
            return false;
        }
        self.county_count = count;
        true
    }

    pub fn season(&self) -> Option<Season> {
        Season::from_index(self.season)
    }

    pub fn owner_is_human(&self, owner: u8) -> bool {
        owner != 0 && (owner as usize) < MAX_REALMS && self.realms[owner as usize].is_human
    }

    pub fn county_ids(&self) -> core::ops::RangeInclusive<usize> {
        1..=self.county_count
    }

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
        // `Turn_BeginPlayersTurn` (`0x0049B6D3`), which `Turn_AdvancePhase`
        // (`0x0049CE51`) calls on the wrap into phase 4 — **unless the
        // interactive frames already opened this turn's phase 4 and ran the
        // AI's steps in it**, which is where the original runs them. See
        // [`TurnMachine::players_turn_open`] and `l2_game::turn::open_players_turn`.
        if tick.advanced_to == Some(Phase::PlayersTurn) && !self.turn.players_turn_open {
            ai::begin_turn(&mut self.realms);
        }
        if tick.phase == Phase::PlayersTurn && tick.advanced_to.is_some() {
            self.turn.players_turn_open = false;
        }
        (tick, report)
    }

    pub fn advance_season(&mut self) -> SeasonReport {
        let mut report = SeasonReport::new();
        for pass in SEASON_PIPELINE {
            self.run_pass(pass, &mut report);
            report.passes.push(pass);
        }
        report
    }

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
            Pass::ArmyRecountTroops => self.army_recount_troops(),
            Pass::History => self.history(),
            Pass::RationPreview => self.ration_apply(true),
            Pass::RefreshEstimates => self.refresh_estimates_all(),
            Pass::MercenaryAdvance => self.mercenary_advance(),
            Pass::UnitsResetMoves => self.units_reset_moves(),
            Pass::ReconcileAlliances => self.reconcile_alliances(),
        }
    }


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
            tax::bank(&mut self.counties[id], &mut self.realms, take);
        }
    }

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
            // `Ration_ApplyAll` (`0x0044BF04`) hands `Ration_Apply`
            // `g_seasonPrev`; every other call in the binary hands it
            // `g_season`. The gate is season 4, so the spending pass reserves
            // the seed on the turn `Grain_SeasonTick` sows — Spring.
            let season = if preview { self.season } else { self.season_prev };
            let sowing = ration::Sowing::from_index(season, self.options.advanced_farming);
            if preview {
                ration::preview(&self.tables, &mut self.counties[id], self.options.armies_eat, sowing);
            } else {
                ration::apply(&self.tables, &mut self.counties[id], self.options.armies_eat, sowing);
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

    fn field_reclaim(&mut self) {
        for id in 1..=self.county_count {
            land::reclaim_fields(&self.tables, &mut self.counties[id], &mut self.campaign.map);
            self.counties[id].labour_wanted[crate::tables::JOB_FIELD_RECLAMATION] =
                crate::county::LABOUR_NO_FLOOR;
            self.counties[id].labour_useful[crate::tables::JOB_FIELD_RECLAMATION] =
                land::reclaim_labour_estimate(&self.tables, &self.counties[id], &self.campaign.map);
        }
    }

    fn grain_season_tick(&mut self) {
        let Some(season) = self.season() else { return };
        let season_next = Season::from_index(self.season_next).unwrap_or(Season::Spring);
        let advanced = self.options.advanced_farming;
        let quirks = self.options.quirks;
        for id in 1..=self.county_count {
            land::grain_season_tick(&self.tables, &mut self.counties[id], season, advanced, quirks);
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

    fn herd_season_tick(&mut self) {
        for id in 1..=self.county_count {
            land::herd_season_tick(
                &self.tables,
                &mut self.counties[id],
                self.season,
                self.season_next,
            );
            crate::field::herd_update_crowding(
                &self.tables,
                &mut self.counties[id],
                &mut self.campaign.map,
            );
            if self.counties[id].pop_band != 0 {
                let herd =
                    land::herd_labour_estimate(&self.tables, &self.counties[id], self.season_next);
                self.counties[id].labour_wanted[crate::tables::JOB_CATTLE_FARMING] = herd.wanted;
                self.counties[id].labour_useful[crate::tables::JOB_CATTLE_FARMING] = herd.useful;
            }
        }
    }

    /// `Labour_AllocateAll` (`0x0044F699`) — [`crate::labour::allocate`] for
    /// counties 1..=`county_count`, in index order.
    ///
    /// `Army_RecountCountyTroops` (`0x004AD6C0`) — the recount, then its own
    /// tail: `Ration_Apply(c, g_seasonNext); Grain_LabourEstimate(c,
    /// g_seasonNext); Herd_LabourEstimate(c, g_seasonNext)` for counties
    /// `1 ..= 16`. See [`Pass::ArmyRecountTroops`] for why the tail is the
    /// pass's point.
    ///
    /// The `Ration_Apply` here is [`crate::ration::preview`]: the original's
    /// never debits a store — `Ration_ApplyAll` shadows what it priced into
    /// `+0x18C`/`+0x190` and `Herd_SeasonTick` (`0x0044D60D`) spends the shadow.
    fn army_recount_troops(&mut self) {
        self.campaign.units.recount_county_troops(&mut self.counties, &self.realms);
        let season_next = Season::from_index(self.season_next).unwrap_or(Season::Spring);
        let advanced = self.options.advanced_farming;
        let armies_eat = self.options.armies_eat;
        let sowing = ration::Sowing::from_index(self.season, advanced);
        for id in 1..=self.county_count {
            ration::preview(&self.tables, &mut self.counties[id], armies_eat, sowing);
            if let Some(grain) =
                land::grain_labour_estimate(&self.tables, &self.counties[id], season_next, advanced)
            {
                self.counties[id].labour_wanted[crate::tables::JOB_GRAIN_FARMING] = grain.wanted;
                self.counties[id].labour_useful[crate::tables::JOB_GRAIN_FARMING] = grain.useful;
            }
            if self.counties[id].pop_band != 0 {
                let herd = land::herd_labour_estimate(
                    &self.tables,
                    &self.counties[id],
                    self.season_next,
                );
                self.counties[id].labour_wanted[crate::tables::JOB_CATTLE_FARMING] = herd.wanted;
                self.counties[id].labour_useful[crate::tables::JOB_CATTLE_FARMING] = herd.useful;
            }
        }
    }

    fn labour_allocate_all(&mut self) {
        for id in 1..=self.county_count {
            crate::labour::allocate(&mut self.counties[id]);
        }
    }

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

    pub fn restore(&self) -> crate::conquest::Restore {
        crate::conquest::Restore {
            sowing: crate::ration::Sowing::from_index(self.season, self.options.advanced_farming),
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
    pub fn make_county_independent(&mut self, county: usize) {
        if county == 0 || county > self.county_count {
            return;
        }
        let r = self.restore();
        let Kingdom { counties, realms, campaign, tables, .. } = self;
        crate::conquest::make_independent(tables, counties, realms, county as u8, &campaign.map, r);
    }

    /// The blacksmith's stockpile share ([`industry::weapon_shares`],
    /// `FUN_0044F15B`) is computed **per realm, before the loop**. The original
    /// recomputes it inside every `resourceLimit` call, which is the same
    /// answer: its inputs are which smithies are switched on and staffed, and
    /// the pass changes neither.
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
            let realm = realms.get(owner).unwrap_or(&neutral);
            for &c in estimated {
                industry::refresh(tables, &mut counties[id], c, realm, shares[owner], advanced);
            }
        }
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
        // `crate::tables::SCORE_INPUT_OFFSETS` has read `+0x4C` as *castles
        // held* for as long as it has existed, with the C for it in the doc
        // comment — and **nothing in this workspace ever wrote it**, so
        // `score_inputs[5]`
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


