#![allow(unused_imports)]
use super::*;
use super::unit::*;
use super::industry::*;
use super::realm::*;
use super::history::*;
use super::tables::*;
use super::*;
use super::codec::*;
use crate::county::{ChangeReason, County, Industry, MAX_COUNTIES, MAX_INFLOW_SOURCES, MAX_NEIGHBOURS};
use crate::kingdom::{History, HistoryEntry, Kingdom, Options};
use crate::phase::{Phase, TurnMachine};
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::{
    Tables, Weather, HISTORY_COUNTIES, HISTORY_SEASONS, JOB_COUNT, WEAPON_TYPE_COUNT,
};
use l2_net::canonical::{Canonical, CodecError, Decode, Encode, Reader};

impl Encode for County {
    fn encode(&self, out: &mut Canonical) {
        out.bool(self.event_fired);
        out.u16(self.event_id);
        out.u8(self.owner);
        out.u8(self.health_band);
        out.i32(self.health_meter);
        out.i32(self.happiness);
        out.i32(self.happiness_last);
        out.i32(self.d_hap_tax);
        out.i32(self.d_hap_tax_local);
        out.i32(self.d_hap_health);
        out.i32(self.d_hap_ration);
        out.i32(self.shown_tax);
        out.i32(self.shown_ration);
        out.i32(self.shown_health);
        out.i32(self.shown_army);
        out.i32(self.tax_hap_other);
        out.i32(self.shown_events);
        out.i32(self.happiness_avg);
        out.i32(self.happiness_sum);
        out.i32(self.shown_ale);
        out.i32(self.ale_happiness_given);
        out.u8(self.unrest);
        out.bool(self.unrest_warned);

        out.i32(self.population);
        out.i32(self.pop_last);
        out.i32(self.pop_change_pct);
        out.i32(self.births);
        out.i32(self.deaths);
        out.i32(self.army);
        out.i32(self.emigrants);
        out.i32(self.immigrants);
        out.i32(self.largest_inflow);
        out.u8(self.emigrant_destination);
        out.u8(self.largest_inflow_source);
        out.raw(&self.inflow_sources);
        out.u8(self.neighbour_count);
        out.raw(&self.neighbours);
        out.u8(self.change_reason as u8);
        out.i32(self.pop_band);
        out.u8(self.anchor_x);
        out.u8(self.anchor_y);

        out.i32(self.tax_rate);
        out.i32(self.tax_collected);
        out.i32(self.tax_shown);
        out.i32(self.purse);
        out.i32(self.merchant_count);
        out.u8(self.merchant_unit);
        out.i32(self.merchant_visits);
        for job in &self.labour {
            out.i32(*job);
        }
        // **The other three labour arrays and the industry split.** They were
        // missing until a game save round-tripped the England position and came
        // back with `County::new`'s defaults in all four; see [`VERSION`] 5.
        // `labour_useful` and `labour_share` are what `FUN_0044F6E7` allocates
        // *from*, so they are simulation state and not a display hint, and
        // leaving them out of the encoding left them out of the lockstep
        // checksum too.
        for job in &self.labour_wanted {
            out.i32(*job);
        }
        for job in &self.labour_useful {
            out.i32(*job);
        }
        for job in &self.labour_share {
            out.i32(*job);
        }
        out.i32(self.industry_share);
        for field in &self.field_progress {
            out.u16(*field);
        }
        out.i32(self.ration_achieved);
        out.i32(self.ration_wanted);
        out.i32(self.ration_split);
        out.i32(self.grain_eaten);
        out.i32(self.herd_eaten);
        out.i32(self.grain_available);
        out.i32(self.herd_available);
        out.i32(self.grain_eaten_shadow);
        out.i32(self.herd_eaten_shadow);
        out.i32(self.friendly_troops);
        out.i32(self.enemy_troops);
        out.u8(self.mercenary_offer);
        out.u32(self.garrison_unit as u32);
        out.i32(self.levy_surcharge);
        out.u8(self.castle_type);
        out.u8(self.castle_building);
        out.u8(self.castle_degraded);
        out.bool(self.castle_ruined);
        out.u8(self.castle_level_left);
        // The scars, `VERSION` 15.
        out.u16(self.siege_scars.moat_filled);
        out.u16(self.siege_scars.wall_damage);
        out.i32(self.siege_scars.breach_score);
        out.i32(self.siege_scars.approach_score);
        out.u8(self.siege_scars.ramparts_breached);
        out.bool(self.siege_scars.gate_open);
        out.bool(self.castle_switch);
        out.u8(self.castle_percent);
        out.i32(self.castle_work_left);
        out.i32(self.castle_work_total);
        out.i32(self.castle_stone_owed);
        out.i32(self.castle_stone_total);
        out.i32(self.castle_wood_owed);
        out.i32(self.castle_wood_total);
        out.i32(self.event_population_pct);
        // The letter's figure, `VERSION` 20.
        out.i32(self.event_population_swing);
        out.i32(self.event_grain_pct);
        out.i32(self.event_herd_pct);
        for tile in &self.field_tiles {
            out.u16(*tile);
        }
        // The two round-robin field cursors, `VERSION` 27.
        out.u8(self.pasture_cursor);
        out.u8(self.blight_cursor);
        out.i32(self.fields_fallow);
        out.i32(self.fields_cattle);
        out.i32(self.fields_grain);
        out.i32(self.fields_waste);
        out.i32(self.fields_reclaiming);
        out.i32(self.fertility);
        out.u8(self.weather.index());
        out.i32(self.dryness);
        out.i32(self.grain);
        for stage in &self.crop {
            out.i32(*stage);
        }
        out.i32(self.fields_grain_sown);
        out.i32(self.fields_grain_standing);
        out.bool(self.sow_shortfall);
        out.i32(self.herd);
        out.i32(self.herd_crowding);
        out.i32(self.herd_births_expected);
        out.i32(self.herd_deaths_expected);
        out.i32(self.herd_change_expected);
        out.i32(self.grain_weather_change);
        out.i32(self.grain_event_change);
        out.i32(self.herd_weather_change);
        out.i32(self.herd_event_change);
        out.i32(self.grain_sown_expected);
        out.i32(self.grain_grown_expected);
        out.i32(self.grain_change_expected);
        out.i32(self.reclaim_fields_finishing);
        out.i32(self.reclaim_seasons_to_next);
        for industry in &self.industry {
            industry.encode(out);
        }
        out.u32(self.weapon_type as u32);
        out.u8(self.farm_style);
        out.bool(self.tax_suppressed);
    }
}

impl Decode for County {
    fn decode(input: &mut Reader<'_>) -> Result<County, CodecError> {
        let mut c = County::new();
        c.event_fired = input.bool()?;
        c.event_id = input.u16()?;
        c.owner = input.u8()?;
        c.health_band = input.u8()?;
        c.health_meter = input.i32()?;
        c.happiness = input.i32()?;
        c.happiness_last = input.i32()?;
        c.d_hap_tax = input.i32()?;
        c.d_hap_tax_local = input.i32()?;
        c.d_hap_health = input.i32()?;
        c.d_hap_ration = input.i32()?;
        c.shown_tax = input.i32()?;
        c.shown_ration = input.i32()?;
        c.shown_health = input.i32()?;
        c.shown_army = input.i32()?;
        c.tax_hap_other = input.i32()?;
        c.shown_events = input.i32()?;
        c.happiness_avg = input.i32()?;
        c.happiness_sum = input.i32()?;
        c.shown_ale = input.i32()?;
        c.ale_happiness_given = input.i32()?;
        c.unrest = input.u8()?;
        c.unrest_warned = input.bool()?;

        c.population = input.i32()?;
        c.pop_last = input.i32()?;
        c.pop_change_pct = input.i32()?;
        c.births = input.i32()?;
        c.deaths = input.i32()?;
        c.army = input.i32()?;
        c.emigrants = input.i32()?;
        c.immigrants = input.i32()?;
        c.largest_inflow = input.i32()?;
        c.emigrant_destination = input.u8()?;
        c.largest_inflow_source = input.u8()?;
        c.inflow_sources.copy_from_slice(input.raw(MAX_INFLOW_SOURCES)?);
        c.neighbour_count = input.u8()?;
        c.neighbours.copy_from_slice(input.raw(MAX_NEIGHBOURS)?);
        let at = input.position();
        c.change_reason = match input.u8()? {
            0 => ChangeReason::None,
            1 => ChangeReason::Births,
            2 => ChangeReason::Deaths,
            3 => ChangeReason::Emigration,
            4 => ChangeReason::Immigration,
            tag => return Err(CodecError::BadTag { tag, expected: "change reason", at }),
        };
        c.pop_band = input.i32()?;
        c.anchor_x = input.u8()?;
        c.anchor_y = input.u8()?;

        c.tax_rate = input.i32()?;
        c.tax_collected = input.i32()?;
        c.tax_shown = input.i32()?;
        c.purse = input.i32()?;
        c.merchant_count = input.i32()?;
        c.merchant_unit = input.u8()?;
        c.merchant_visits = input.i32()?;
        for job in 0..JOB_COUNT {
            c.labour[job] = input.i32()?;
        }
        for job in 0..JOB_COUNT {
            c.labour_wanted[job] = input.i32()?;
        }
        for job in 0..JOB_COUNT {
            c.labour_useful[job] = input.i32()?;
        }
        for job in 0..JOB_COUNT - 1 {
            c.labour_share[job] = input.i32()?;
        }
        c.industry_share = input.i32()?;
        for field in 0..c.field_progress.len() {
            c.field_progress[field] = input.u16()?;
        }
        c.ration_achieved = input.i32()?;
        c.ration_wanted = input.i32()?;
        c.ration_split = input.i32()?;
        c.grain_eaten = input.i32()?;
        c.herd_eaten = input.i32()?;
        c.grain_available = input.i32()?;
        c.herd_available = input.i32()?;
        c.grain_eaten_shadow = input.i32()?;
        c.herd_eaten_shadow = input.i32()?;
        c.friendly_troops = input.i32()?;
        c.enemy_troops = input.i32()?;
        c.mercenary_offer = input.u8()?;
        c.garrison_unit = input.u32()? as usize;
        c.levy_surcharge = input.i32()?;
        c.castle_type = input.u8()?;
        c.castle_building = input.u8()?;
        c.castle_degraded = input.u8()?;
        c.castle_ruined = input.bool()?;
        c.castle_level_left = input.u8()?;
        // The scars, `VERSION` 15.
        c.siege_scars.moat_filled = input.u16()?;
        c.siege_scars.wall_damage = input.u16()?;
        c.siege_scars.breach_score = input.i32()?;
        c.siege_scars.approach_score = input.i32()?;
        c.siege_scars.ramparts_breached = input.u8()?;
        c.siege_scars.gate_open = input.bool()?;
        c.castle_switch = input.bool()?;
        c.castle_percent = input.u8()?;
        c.castle_work_left = input.i32()?;
        c.castle_work_total = input.i32()?;
        c.castle_stone_owed = input.i32()?;
        c.castle_stone_total = input.i32()?;
        c.castle_wood_owed = input.i32()?;
        c.castle_wood_total = input.i32()?;
        c.event_population_pct = input.i32()?;
        c.event_population_swing = input.i32()?;
        c.event_grain_pct = input.i32()?;
        c.event_herd_pct = input.i32()?;
        for slot in 0..c.field_tiles.len() {
            c.field_tiles[slot] = input.u16()?;
        }
        c.pasture_cursor = input.u8()?;
        c.blight_cursor = input.u8()?;
        c.fields_fallow = input.i32()?;
        c.fields_cattle = input.i32()?;
        c.fields_grain = input.i32()?;
        c.fields_waste = input.i32()?;
        c.fields_reclaiming = input.i32()?;
        c.fertility = input.i32()?;
        let at = input.position();
        let byte = input.u8()?;
        c.weather = Weather::from_index(byte)
            .ok_or(CodecError::BadTag { tag: byte, expected: "weather", at })?;
        c.dryness = input.i32()?;
        c.grain = input.i32()?;
        for stage in 0..c.crop.len() {
            c.crop[stage] = input.i32()?;
        }
        c.fields_grain_sown = input.i32()?;
        c.fields_grain_standing = input.i32()?;
        c.sow_shortfall = input.bool()?;
        c.herd = input.i32()?;
        c.herd_crowding = input.i32()?;
        c.herd_births_expected = input.i32()?;
        c.herd_deaths_expected = input.i32()?;
        c.herd_change_expected = input.i32()?;
        c.grain_weather_change = input.i32()?;
        c.grain_event_change = input.i32()?;
        c.herd_weather_change = input.i32()?;
        c.herd_event_change = input.i32()?;
        c.grain_sown_expected = input.i32()?;
        c.grain_grown_expected = input.i32()?;
        c.grain_change_expected = input.i32()?;
        c.reclaim_fields_finishing = input.i32()?;
        c.reclaim_seasons_to_next = input.i32()?;
        for slot in 0..c.industry.len() {
            c.industry[slot] = Industry::decode(input)?;
        }
        c.weapon_type = input.u32()? as usize;
        c.farm_style = input.u8()?;
        c.tax_suppressed = input.bool()?;
        Ok(c)
    }
}

