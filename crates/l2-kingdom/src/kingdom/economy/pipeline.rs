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
    /// `Castle_RaiseFreeGarrison` (`0x004A551B`) — the archers a new castle
    /// comes with, mustered and marched straight inside.
    pub(crate) fn raise_free_garrison(&mut self, county: u8, archers: i32) {
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

    pub(crate) fn migration_update(&mut self) {
        population::migrate_all(&mut self.counties, self.county_count, self.options.quirks);
    }

    pub(crate) fn population_update(&mut self) {
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
    pub(crate) fn history(&mut self) {
        self.history.record(&self.counties);
    }

    /// **Paint one field.** `Field_SetType` (`0x00438BEC`) with this kingdom's
    /// own map, ruleset and clock supplied — the whole of what a click on the
    /// campaign map does to the simulation.
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

    /// `Industry_ToggleFromMap` (`0x0043D309`) assembled — the enable byte, the
    /// industry share
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
    /// not ported (C136)
    /// same refresh. **The last line was missing, and it is a visible one.**
    pub fn toggle_industry(&mut self, county: usize, what: crate::industry::MapToggle) -> bool {
        if county == 0 || county > self.county_count {
            return false;
        }
        let quirks = self.options.quirks;
        let armies_eat = self.options.armies_eat;
        let on = crate::industry::toggle_from_map(&mut self.counties[county], what, quirks);
        self.refresh_estimates(county);
        crate::labour::allocate(&mut self.counties[county]);
        crate::ration::preview(&self.tables, &mut self.counties[county], armies_eat, crate::ration::Sowing::from_index(self.season, self.options.advanced_farming));
        crate::labour::allocate(&mut self.counties[county]);
        self.refresh_estimates(county);
        if let crate::industry::MapToggle::Industry(c) = what {
            self.update_industry_site(county, c);
        }
        let owner = self.counties[county].owner;
        self.refresh_blacksmiths(owner);
        on
    }

    /// **`Game_SetupRealmsAndCounties` (`0x0049BD99`)'s two rounds for one start
    /// county**, which run before it switches anything on.
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
            crate::ration::preview(&self.tables, &mut self.counties[county], armies_eat, crate::ration::Sowing::from_index(self.season, self.options.advanced_farming));
            self.refresh_estimates(county);
        }
        for (industry, on) in self.counties[county].industry.iter_mut().zip(switches) {
            industry.enabled = on;
        }
        self.counties[county].castle_switch = castle;
    }

    /// **`FUN_00448648`** — `Industry_LabourEstimate(c, weapons)` for every
    /// county `owner` holds.
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
    /// page's six hotspots do**
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
    /// `Industry_LabourEstimate` writes `labour_useful[7]`, the ceiling the
    /// allocator then deals against, so
    /// on the same click. There is **no `Ration_Apply` and no second
    /// allocation** here; `docs/decisions.md` C177 and
    /// [`Kingdom::set_ration_wanted`] on why that asymmetry is not tidied.
    pub fn set_weapon_type(&mut self, county: usize, weapon: usize) -> bool {
        if county == 0 || county > self.county_count || self.counties.len() <= county {
            return false;
        }
        if weapon >= crate::tables::WEAPON_TYPE_COUNT {
            return false;
        }
        self.counties[county].weapon_type = weapon;
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
            crate::ration::preview(&self.tables, &mut self.counties[county], armies_eat, crate::ration::Sowing::from_index(self.season, self.options.advanced_farming));
            self.refresh_estimates(county);
            self.refresh_blacksmiths(owner);
        }
        workers
    }

    /// **`FUN_00439CC2`** — the first thing `Labour_Move` does after the
    /// arithmetic, and it reads only the destination.
    ///
    /// **Putting men on a site is how a player switches it on**
    /// way besides the map. A new game opens with one industry on per start
    /// county — the first of wood, iron and stone it has, so never the mine in
    /// a county that also has a forest (`Game_SetupRealmsAndCounties`,
    /// `0x0049BD99`) — and without this a player who staffed that mine saw his
    /// men drawn as idle, no iron row on the sidebar
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
    /// The doc this replaces named `FUN_00439122` and county `+0x2C`; the
    /// writer is `0x0043933B` and the field is `+0x08`.
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
        crate::ration::preview(&self.tables, &mut self.counties[county], armies_eat, crate::ration::Sowing::from_index(self.season, self.options.advanced_farming));
        self.refresh_estimates(county);
        true
    }

}

