#![allow(unused_imports)]
use super::*;
use super::setup::*;
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

}
