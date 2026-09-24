#![allow(unused_imports)]
use super::*;
use super::passes::*;
use super::seasonal::*;
use super::holdings::*;
use super::super::*;
use super::super::*;
use super::super::*;
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
        // Turn_AdvancePhase 0x0049CE51: `if (7 < g_turnPhase) { g_turnPhase = 1;
        // Turn_BeginPlayersTurn(); }` — on the wrap into phase 1, not the entry to 4 [V].
        if tick.phase == Phase::SeasonEnd && tick.advanced_to == Some(Phase::NeutralCounties) {
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

}
