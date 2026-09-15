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
    /// **The wanted ration level — the third control on the ration panel, and
    /// the third with the same omission.** `Ration_IncreaseCounty`
    /// (`0x0043A23F`) and its twin:
    ///
    /// It writes `rationWanted` (`+0x15E`) and never `rationAchieved`
/// (`+0x15D`): what the player asks for and what the stores could
    /// feed are different fields, and only the pass decides the second.
    pub fn set_ration_wanted(&mut self, county: usize, level: i32) -> bool {
        if county == 0 || county > self.county_count {
            return false;
        }
        let level = level.clamp(0, crate::tables::RATION_LEVEL_COUNT as i32 - 1);
        let moved = self.counties[county].ration_wanted != level;
        self.counties[county].ration_wanted = level;
        let armies_eat = self.options.armies_eat;
        crate::ration::preview(&self.tables, &mut self.counties[county], armies_eat, crate::ration::Sowing::from_index(self.season, self.options.advanced_farming));
        self.refresh_estimates(county);
        moved
    }

/// **`Opt_ToggleArmyForaging` (`0x004345D0`).**
///
    /// The switch changes who a county feeds — [`crate::ration::people_to_feed`]
    /// adds the armies standing in it — so the original re-runs the ration
    /// pass and the forecasts over every county *on the flip*
    /// panel is right the moment the options panel closes. Ours flipped the
    /// flag and left every county's ration fields describing the old rule until
    /// the next season. `Ration_Apply` is [`crate::ration::preview`] here for the
    /// reason [`Kingdom::set_ration_wanted`] gives: it records and does not
    /// spend. `[V]` against the decompilation.
    pub fn toggle_army_foraging(&mut self) {
        self.options.armies_eat = !self.options.armies_eat;
        let armies_eat = self.options.armies_eat;
        for id in 1..=self.county_count {
            crate::ration::preview(&self.tables, &mut self.counties[id], armies_eat, crate::ration::Sowing::from_index(self.season, self.options.advanced_farming));
            self.refresh_estimates(id);
        }
    }

    /// `Tax_IncreaseCounty` (`0x0043AA83`) and its twin, whole:
///
    /// and `Tax_RecomputePreview` itself ends with `Tax_SumEmpireHappiness(owner)`
    /// and `FUN_0044BA35`, the empire-wide sum of `taxShown` that the court
    /// prints. **Every tax control in the original recomputes and repaints**,
    /// exactly like the ration slider — and this is the second panel found with
/// the same omission, which is the finding.
    pub fn set_tax_rate(&mut self, county: usize, rate: i32) -> bool {
        if county == 0 || county > self.county_count {
            return false;
        }
        let rate = rate.clamp(0, crate::tables::MAX_TAX_RATE);
        let moved = self.counties[county].tax_rate != rate;
        self.counties[county].tax_rate = rate;
        crate::tax::recompute_preview(&self.tables, &mut self.counties[county]);
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

    /// `Ration_SetSplit` (`0x0043A5A9`), whole.
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
        crate::ration::preview(&self.tables, &mut self.counties[county], armies_eat, crate::ration::Sowing::from_index(self.season, self.options.advanced_farming));

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
                crate::ration::preview(&self.tables, &mut self.counties[county], armies_eat, crate::ration::Sowing::from_index(self.season, self.options.advanced_farming));
                if self.counties[county].herd_eaten != was {
                    break;
                }
                if self.counties[county].ration_split == split && sweep {
                    self.counties[county].ration_split = old;
                    crate::ration::preview(
                        &self.tables,
                        &mut self.counties[county],
                        armies_eat,
                        crate::ration::Sowing::from_index(self.season, self.options.advanced_farming),
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

    pub fn run_ai_grants(&mut self) {
        ai::grant_resources(
            &self.tables,
            &mut self.counties,
            &mut self.realms,
            self.county_count,
            self.options.difficulty,
        );
    }

    fn farm_env(&self) -> crate::ai_farm::FarmEnv {
        crate::ai_farm::FarmEnv {
            season: self.season().unwrap_or(Season::Spring),
            season_next: Season::from_index(self.season_next).unwrap_or(Season::Spring),
            advanced_farming: self.options.advanced_farming,
            armies_eat: self.options.armies_eat,
        }
    }

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

    pub fn run_neutral_farms_at_the_stall(&mut self) -> i32 {
        let env = self.farm_env();
        let view = self.realms.clone();
        let mut market = crate::ai_farm::CountyStall::new(
            &self.tables,
            &self.counties,
            &self.campaign.units,
            &mut self.realms,
            env.season_next,
            env.armies_eat,
            crate::ration::Sowing::new(env.season, env.advanced_farming),
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
            crate::ration::Sowing::new(env.season, env.advanced_farming),
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
    /// **The class is four callers of `Ai_ManageCountyFarms`, and three are
    /// reproduced.** AI step 5 and this are two. `FUN_0049DF48` is a
    /// byte-for-byte twin of this loop (tests in the other order) whose only
    /// caller is the tail of `Battle_ReturnToCampaign` (`0x004AB383`):
    ///
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

    /// `FUN_0049DF48` — the **last call of `Battle_ReturnToCampaign`**
    /// (`0x004AB383`): `FUN_004AD426(); Panels_RefreshAll(); FUN_0049DF48();`.
    ///
    /// It is the fourth caller of `Ai_ManageCountyFarms` and the same loop as
    /// [`Kingdom::ai_manage_farms_all`] (`Ai_ManageFarmsAll`, `0x0049A990`)
    /// with its two tests written the other way round — `isHuman == 0 &&
    /// strength != 0` there against `strength != 0 && isHuman == 0` here.
    pub fn ai_manage_farms_after_battle(&mut self) -> i32 {
        self.ai_manage_farms_all()
    }

    pub fn run_ai_castles(&mut self, realm: u8) -> Vec<u8> {
        ai::build_castles(
            &self.tables,
            &mut self.counties,
            self.county_count,
            &mut self.realms,
            realm,
        )
    }

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
    pub fn init_diplomacy(&mut self) {
        crate::diplomacy::init(&mut self.realms, &mut self.diplomacy);
    }

    pub fn run_ai_inbox(&mut self, realm_id: u8) -> Vec<crate::diplomacy::Letter> {
        crate::diplomacy::answer_inbox(
            &mut self.realms,
            &mut self.diplomacy,
            &self.tables,
            realm_id,
        )
    }

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
    pub fn reconcile_alliances(&mut self) {
        crate::diplomacy::reconcile_alliances(&mut self.realms);
    }

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

    pub fn offend(&mut self, offended: u8, offender: u8, amount: i8) -> Vec<crate::diplomacy::Letter> {
        crate::diplomacy::offend(&mut self.realms, offended, offender, amount)
    }

    pub fn run_ai_taunt(&mut self, realm_id: u8) -> Vec<ai::Taunt> {
        let trailer = ai::rank_trailer(&self.realms);
        let snapshot = self.realms.clone();
        let Some(realm) = self.realms.get_mut(realm_id as usize) else { return Vec::new() };
        ai::taunt(realm, realm_id, &snapshot, trailer)
    }
}


