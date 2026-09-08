//! The kingdom — the whole state, and the driver that walks
//! [`SEASON_PIPELINE`] over it.
//!
//! `Season_Advance` (`0x00448440`) calls 28 functions in a fixed order, and
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
use crate::report::SeasonReport;
use crate::tables::{Commodity, Season};
use crate::tax;
use crate::unrest;
use crate::weather;
use l2_net::Pcg32;

/// The three game options `docs/kingdom.md` §9 reads out of the shipped save,
/// and the only ones any rule in this crate branches on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// `g_optDifficulty`, 0..=2 in the shipped save (0), and 0..=3 in the
    /// AI grant tables. Only the AI's wages and grants read it.
    pub difficulty: u8,
    /// `g_optAdvancedFarming`. Off in the shipped save, which is why every
    /// county there is Cloudy with zero fertility.
    pub advanced_farming: bool,
    /// `g_optArmiesEat`. Off in the shipped save.
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
    /// The simulation's generator. Frozen in-tree in `l2-net`; two draws a
    /// season for the weather, two per county for the event roll.
    pub rng: Pcg32,
}

impl Kingdom {
    /// An empty kingdom, before any map is loaded.
    pub fn new(seed: u64) -> Kingdom {
        Kingdom {
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
            Pass::FertilityUpdate => self.fertility_update(),
            Pass::FieldReclaim => self.field_reclaim(),
            Pass::GrainSeasonTick => self.grain_season_tick(),
            Pass::HerdSeasonTick => self.herd_season_tick(),
            Pass::Industry(c) => self.industry(c),
            Pass::CastleBuildTick => self.castle_build_tick(report),
            Pass::MigrationUpdate => self.migration_update(),
            Pass::PopulationUpdate => self.population_update(),
            Pass::ScoreRank => ai::rank_realms(&mut self.realms),
            Pass::History => self.history(),
            Pass::RationPreview => self.ration_apply(true),
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
        let humans: [bool; MAX_REALMS] = core::array::from_fn(|i| self.realms[i].is_human);
        let owner_is_human = move |owner: u8| {
            owner != 0 && (owner as usize) < MAX_REALMS && humans[owner as usize]
        };
        event::roll_all(
            &mut self.counties,
            self.county_count,
            &owner_is_human,
            self.year,
            &mut self.rng,
            &mut report.messages,
        );
    }

    fn weather(&mut self) {
        let Some(season) = self.season() else { return };
        weather::update_all(
            &mut self.counties,
            self.county_count,
            season,
            self.options.advanced_farming,
            &mut self.rng,
        );
    }

    fn tax_collect(&mut self) {
        tax::sum_empire_happiness(&mut self.counties, &mut self.realms, self.county_count);
        for id in 1..=self.county_count {
            let owner = self.counties[id].owner as usize;
            let empire = if owner != 0 && owner < MAX_REALMS {
                self.realms[owner].tax_hap_empire as i32
            } else {
                0
            };
            let take = tax::collect(&mut self.counties[id], empire);
            if owner != 0 && owner < MAX_REALMS {
                self.realms[owner].gold += take;
            }
        }
    }

    /// `Wages_PayAll` pays the bill already in each realm's `wages`.
    ///
    /// The bill itself is [`industry::compute_wages`] over the realm's units,
    /// and units live in `g_units`, which is not this crate's. So the caller
    /// sets `realm.wages` before the season advances; this pass spends it.
    fn wages_pay(&mut self, report: &mut SeasonReport) {
        for id in 1..MAX_REALMS {
            if !self.realms[id].in_play {
                continue;
            }
            industry::pay(&mut self.realms[id], id as u8, &mut report.messages);
        }
    }

    fn ration_apply(&mut self, preview: bool) {
        for id in 1..=self.county_count {
            if preview {
                ration::preview(&mut self.counties[id], self.options.armies_eat);
            } else {
                ration::apply(&mut self.counties[id], self.options.armies_eat);
            }
        }
    }

    fn health_update(&mut self) {
        for id in 1..=self.county_count {
            health::update(&mut self.counties[id]);
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

    fn field_reclaim(&mut self) {
        for id in 1..=self.county_count {
            land::reclaim_fields(&mut self.counties[id]);
        }
    }

    fn grain_season_tick(&mut self) {
        let Some(season) = self.season() else { return };
        for id in 1..=self.county_count {
            land::grain_season_tick(
                &mut self.counties[id],
                season,
                self.options.advanced_farming,
            );
        }
    }

    fn herd_season_tick(&mut self) {
        for id in 1..=self.county_count {
            land::herd_season_tick(&mut self.counties[id]);
        }
    }

    fn industry(&mut self, commodity: Commodity) {
        let (counties, realms) = (&mut self.counties, &mut self.realms);
        for id in 1..=self.county_count {
            let owner = counties[id].owner as usize;
            // An unowned county has no realm to credit, so it produces nothing.
            if owner == 0 || owner >= MAX_REALMS {
                continue;
            }
            industry::produce(&mut counties[id], &mut realms[owner], commodity);
        }
    }

    fn castle_build_tick(&mut self, report: &mut SeasonReport) {
        for id in 1..=self.county_count {
            let mut messages = Vec::new();
            industry::build_tick(&mut self.counties[id], id as u8, &mut messages);
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
        population::update_all(&mut self.counties, self.county_count, season);
    }

    /// The history ring. `docs/kingdom.md` §3.4 names it and nothing more —
    /// neither its length nor what it stores was traced, so nothing is stored.
    fn history(&mut self) {}

    /// `AI_SetTaxRates`' resource grants, which run in the AI's turn rather
    /// than in `Season_Advance`. Exposed separately for that reason.
    pub fn run_ai_grants(&mut self) {
        ai::grant_resources(
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
}
