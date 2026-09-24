#![allow(unused_imports)]
use super::*;
use super::setup::*;
use super::passes::*;
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
    /// `Units_ResetMoves` (`0x004651B9`) — turn phase 7. Every slot, every
    /// type: `moveState = 0` and `movesUsed = 0`.
    pub(super) fn units_reset_moves(&mut self) {
        self.campaign.units.reset_moves();
        // Turn_Tick 0x0049A010 phase 7: Units_ResetMoves(); Move_BuildCostMap(); [V]
        self.campaign.rebuild_move_cost();
    }

    /// `Mercenary_AdvanceAll` (`0x004ACA2B`) — turn phase 7, and **before**
    /// [`Kingdom::units_reset_moves`].
    ///
    /// > Corrected in the document. It matters because the order two peers run
    /// > the season's passes in is the specification (`docs/kingdom.md` §3.4),
    /// > and [`SEASON_PIPELINE`] is where that is written down. `[D]`
    pub(super) fn mercenary_advance(&mut self) {
        let count = self.county_count;
        let quirks = self.options.quirks;
        self.campaign.mercenaries.advance(&mut self.counties, count, quirks);
    }

    pub(crate) fn ration_apply(&mut self, preview: bool) {
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

    pub(super) fn health_update(&mut self) {
        for id in 1..=self.county_count {
            health::update(&self.tables, &mut self.counties[id]);
        }
    }

    pub(super) fn happiness_update(&mut self) {
        for id in 1..=self.county_count {
            let human = self.owner_is_human(self.counties[id].owner);
            happiness::update(&mut self.counties[id], human, self.turn_count);
        }
    }

    /// `Unrest_UpdateAll` (`0x0044AA41`), including the call it makes at the
    /// bottom that this pass used to skip.
    pub(super) fn unrest_update(&mut self, report: &mut SeasonReport) {
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

    pub(super) fn fertility_update(&mut self) {
        for id in 1..=self.county_count {
            land::update_fertility(&mut self.counties[id], self.options.advanced_farming);
        }
    }

    pub(super) fn field_reclaim(&mut self) {
        for id in 1..=self.county_count {
            land::reclaim_fields(&self.tables, &mut self.counties[id], &mut self.campaign.map);
            self.counties[id].labour_wanted[crate::tables::JOB_FIELD_RECLAMATION] =
                crate::county::LABOUR_NO_FLOOR;
            self.counties[id].labour_useful[crate::tables::JOB_FIELD_RECLAMATION] =
                land::reclaim_labour_estimate(&self.tables, &self.counties[id], &self.campaign.map);
        }
    }

    pub(super) fn grain_season_tick(&mut self) {
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

    pub(super) fn herd_season_tick(&mut self) {
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
    pub(super) fn army_recount_troops(&mut self) {
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

    pub(super) fn labour_allocate_all(&mut self) {
        for id in 1..=self.county_count {
            crate::labour::allocate(&mut self.counties[id]);
        }
    }

    pub(super) fn refresh_estimates_all(&mut self) {
        for id in 1..=self.county_count {
            self.refresh_estimates(id);
            crate::tax::recompute_preview(&self.tables, &mut self.counties[id]);
        }
    }

    /// `Realm_SecedeIsolatedCounties` (`0x0044AE3C`) — `docs/kingdom.md` §6.1.
    pub(super) fn secede_isolated_counties(&mut self, report: &mut SeasonReport) {
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

}
