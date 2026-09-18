#![allow(unused_imports)]
use super::*;

use super::*;
use crate::tables::{
    Commodity, Weather, FIELD_PROGRESS_MAX, JOB_CATTLE_FARMING, JOB_COUNT, JOB_GRAIN_FARMING,
    JOB_IDLE_TOWNSFOLK, RATION_LEVEL_COUNT,
};

impl County {
    pub fn new() -> County {
        County {
            event_fired: false,
            event_id: 0,
            owner: 0,
            health_band: 0,
            health_meter: 0,
            happiness: 0,
            happiness_last: 0,
            d_hap_tax: 0,
            d_hap_tax_local: 0,
            d_hap_health: 0,
            d_hap_ration: 0,
            shown_tax: 0,
            shown_ration: 0,
            shown_health: 0,
            shown_army: 0,
            tax_hap_other: 0,
            shown_events: 0,
            happiness_avg: 0,
            happiness_sum: 0,
            shown_ale: 0,
            ale_happiness_given: 0,
            unrest: 0,
            unrest_warned: false,
            population: 0,
            pop_last: 0,
            pop_change_pct: 0,
            births: 0,
            deaths: 0,
            army: 0,
            emigrants: 0,
            immigrants: 0,
            largest_inflow: 0,
            emigrant_destination: 0,
            largest_inflow_source: 0,
            inflow_sources: [0; MAX_INFLOW_SOURCES],
            neighbour_count: 0,
            neighbours: [0; MAX_NEIGHBOURS],
            change_reason: ChangeReason::None,
            pop_band: 0,
            anchor_x: 0,
            anchor_y: 0,
            tax_rate: 0,
            tax_collected: 0,
            purse: 0,
            merchant_count: 0,
            merchant_unit: 0,
            merchant_visits: 0,
            tax_shown: 0,
            labour: [0; JOB_COUNT],
            // Zero, not the sentinels: `FUN_00451150` sets up a fresh county
            // with `useful = 0; wanted = useful; workers = wanted;` for all
            // nine records, and only then runs the estimates.
            labour_wanted: [0; JOB_COUNT],
            labour_useful: [0; JOB_COUNT],
            // `FUN_004514F8`'s defaults: 33 / 50 / 17 across the farm and all
            // of the industry share on wood, each half summing to 100.
            labour_share: [33, 50, 17, 0, 0, 0, 100, 0],
            industry_share: 25,
            field_progress: [0; MAX_FIELDS],
            ration_achieved: 3,
            ration_wanted: 3,
            ration_split: 100,
            grain_eaten: 0,
            herd_eaten: 0,
            grain_available: 0,
            herd_available: 0,
            grain_eaten_shadow: 0,
            herd_eaten_shadow: 0,
            friendly_troops: 0,
            enemy_troops: 0,
            mercenary_offer: 0,
            garrison_unit: 0,
            levy_surcharge: 0,
            castle_type: 0,
            castle_building: 0,
            castle_degraded: 0,
            castle_ruined: false,
            castle_level_left: 0,
            siege_scars: crate::siege::SiegeScars::default(),
            castle_switch: false,
            castle_percent: 0,
            castle_work_left: 0,
            castle_work_total: 0,
            castle_stone_owed: 0,
            castle_stone_total: 0,
            castle_wood_owed: 0,
            castle_wood_total: 0,
            event_population_pct: 0,
            event_population_swing: 0,
            event_grain_pct: 0,
            event_herd_pct: 0,
            field_tiles: [0; MAX_FIELDS],
            pasture_cursor: 0,
            blight_cursor: 0,
            fields_fallow: 0,
            fields_cattle: 0,
            fields_grain: 0,
            fields_waste: 0,
            fields_reclaiming: 0,
            fertility: 0,
            weather: Weather::Cloudy,
            dryness: 0,
            grain: 0,
            crop: [0; 3],
            fields_grain_sown: 0,
            fields_grain_standing: 0,
            sow_shortfall: false,
            herd: 0,
            herd_crowding: crate::tables::HERD_CROWDING[0].1,
            herd_births_expected: 0,
            herd_deaths_expected: 0,
            herd_change_expected: 0,
            grain_weather_change: 0,
            grain_event_change: 0,
            herd_weather_change: 0,
            herd_event_change: 0,
            grain_sown_expected: 0,
            grain_grown_expected: 0,
            grain_change_expected: 0,
            reclaim_fields_finishing: 0,
            reclaim_seasons_to_next: 0,
            industry: [
                Industry::new(Commodity::Wood),
                Industry::new(Commodity::Iron),
                Industry::new(Commodity::Weapons),
                Industry::new(Commodity::Stone),
            ],
            weapon_type: 0,
            farm_style: 0,
            tax_suppressed: false,
        }
    }

    pub fn is_unowned(&self) -> bool {
        self.owner == 0
    }

    pub fn field_total(&self) -> i32 {
        self.fields_fallow + self.fields_cattle + self.fields_grain
    }

    pub fn field_tile(&self, slot: usize) -> Option<usize> {
        match self.field_tiles.get(slot) {
            Some(&0) | None => None,
            Some(&t) => Some(t as usize),
        }
    }

    pub fn set_field_tile(&mut self, slot: usize, tile: Option<usize>) -> bool {
        let Some(cell) = self.field_tiles.get_mut(slot) else { return false };
        *cell = tile.unwrap_or(0) as u16;
        true
    }

    pub fn field_slot(&self, tile: usize) -> Option<usize> {
        (0..MAX_FIELDS).find(|&slot| self.field_tile(slot) == Some(tile))
    }

    pub fn field_slots_used(&self) -> usize {
        (0..MAX_FIELDS).filter(|&slot| self.field_tile(slot).is_some()).count()
    }

    pub fn neighbours(&self) -> &[u8] {
        let n = (self.neighbour_count as usize).min(MAX_NEIGHBOURS);
        &self.neighbours[..n]
    }

    pub fn add_neighbour(&mut self, id: u8) -> bool {
        let n = self.neighbour_count as usize;
        if n >= MAX_NEIGHBOURS {
            return false;
        }
        self.neighbours[n] = id;
        self.neighbour_count += 1;
        true
    }

    /// `popBand` = `(pop - 1) / 25 + 1` (`+0xB8`). Written
    /// document states it, including at population 0 where C's truncating
    /// division makes `(0 - 1) / 25` zero and the band 1.
    pub fn compute_pop_band(&self) -> i32 {
        (self.population - 1) / 25 + 1
    }

    pub fn ration_index(&self) -> usize {
        (self.ration_achieved.max(0) as usize).min(RATION_LEVEL_COUNT - 1)
    }

    /// The three minimap overlay ratings — `FUN_00451BBA` (`0x00451BBA`),
    /// which `Minimap_DrawOverlay` calls on **every** draw before it reads
    /// them. See [`MinimapBands`].
    pub fn minimap_bands(&self) -> MinimapBands {
        // +0x01. The original divides an `i8` happiness by 20 and stores an
        // `i8`
        // happiness of -20 or worse wraps past 5 and the county is left
// uncoloured. Reproduced with the same
        // cast, so the edge behaves the same if happiness ever goes negative.
        let happiness = ((self.happiness / 20) as i8) as u8;

        // +0x02. `DAT_00553E60` is a debug toggle, zeroed by the bulk global
        // reset at `0x00497500` and flipped only inside the command dispatcher
        // at `0x004B29BE`. With it clear — the shipped game — the food rating
        // is *binary*: red when the county did not achieve the ration it was
        // asked for, and **6, meaning draw nothing at all**, when it did. The
        // debug branch spreads `ration_achieved` over bands 1..=5 instead.
        let food = if self.ration_achieved < self.ration_wanted { 0 } else { 6 };

        // +0x03. Idle townsfolk, plus one for each of jobs 0..=7 carrying more
        // workers than it can use. Understaffing either farm job below its
        // wanted floor beats everything and gives band 0; otherwise a county
        // with no slack at all is 6 (draw nothing) and one with slack is 5.
        let mut slack = self.labour[JOB_IDLE_TOWNSFOLK];
        for job in 0..JOB_IDLE_TOWNSFOLK {
            if self.labour_useful[job] < self.labour[job] {
                slack += 1;
            }
        }
        let short = self.labour[JOB_GRAIN_FARMING] < self.labour_wanted[JOB_GRAIN_FARMING]
            || self.labour[JOB_CATTLE_FARMING] < self.labour_wanted[JOB_CATTLE_FARMING];
        let labour = if short {
            0
        } else if slack == 0 {
            6
        } else {
            5
        };

        MinimapBands { labour, food, happiness }
    }

    pub fn reclaim_field(&mut self, field: usize, by: i32) -> u16 {
        let p = self.field_progress[field] as i32;
        let next = (p + by.min(crate::tables::FIELD_RECLAIM_PER_SEASON)).min(FIELD_PROGRESS_MAX);
        self.field_progress[field] = next.max(0) as u16;
        self.field_progress[field]
    }
}


