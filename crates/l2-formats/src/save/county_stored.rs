//! The county fields past [`County`]'s first cut, read so `l2-scenario` does
//! no offset arithmetic of its own (C142, C146, C149, C161). A field's
//! `docs/stored-fields.json` id is `County+` the offset its reader names.
use super::*;

/// `g_countyFieldTiles` (`0x0053EA00`) — 17 × 20 × `u32` byte offsets into
/// `g_tiles`. `[V]`: save block 12 is 1,360 bytes = 17 × 80.
pub const COUNTY_FIELD_TILES: u32 = 0x0053_EA00;
pub const COUNTY_FIELD_STRIDE: u32 = 0x50;
pub const COUNTY_FIELD_SLOTS: usize = 20;
/// `+0xC4 + job*0x0C` — nine labour records; `[V]` every county's workers sum
/// to its population in the England turn-one fixture. `docs/kingdom.md` §13.
pub const LABOUR_JOBS: usize = 9;
/// `+0x294 + c*0x18` — four industry records. C57, C161.
pub const INDUSTRIES: usize = 4;

/// One industry record, county `+0x294 + c*0x18`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IndustryRecord {
    pub efficiency: u8,
    pub has_resource: u8,
    pub disabled_seasons: u8,
    pub enabled: u8,
    pub capacity: i16,
    pub total: i32,
    pub total_snapshot: i32,
    /// `+0x14` — `Industry_LabourEstimate` (`0x0044F318`)'s tail; C142.
    pub next_season: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CountyStored {
    /// `+0x008` — `[V]` `FUN_00450000` indexes it with `+0x130`.
    pub industry_share: i8,
    /// `+0x00F`, `+0x0C0` — `Tax_RecomputePreview` (`0x0044B80B`); C142.
    pub d_hap_tax_local: i8,
    pub tax_shown: i32,
    /// `+0x0C4 + job*0x0C` — workers, wanted, useful.
    pub labour: [[i32; LABOUR_JOBS]; 3],
    pub labour_share: [i32; LABOUR_JOBS - 1],
    /// `+0x1B0` — `Industry_ToggleFromMap` flips it, `Labour_Allocate` gates on it.
    pub castle_switch: u8,
    /// `+0x1F4`, `+0x1A0`, `+0x1A4`, `+0x1A5` — `Tax_CollectAll` (`0x0044B59B`)
    /// banks an unowned county's tax here; `Ai_BuyGood` (`0x004A4B12`) reads
    /// the stall. C149.
    pub purse: i32,
    pub merchant_visits: i32,
    pub merchant_count: u8,
    pub merchant_unit: u8,
    /// `+0x1FE` — `County_Reset` (`0x00451150`) seeds `countyId & 1`.
    pub farm_style: u8,
    pub industry: [IndustryRecord; INDUSTRIES],
    /// `g_countyFieldTiles[county]`, not the county record.
    pub field_tiles: [i32; COUNTY_FIELD_SLOTS],
    // C161 — `docs/stored-fields.json` rows with status `imported`.
    pub event_fired: u8,
    pub tax_hap_other: i8,
    pub unrest_warned: u8,
    pub pop_change_pct: i32,
    pub army: i32,
    pub largest_inflow: i32,
    pub inflow_sources: [u8; 16],
    pub emigrant_destination: u8,
    pub largest_inflow_source: u8,
    pub change_reason: u8,
    pub field_progress: [u16; 20],
    pub pasture_cursor: u8,
    pub blight_cursor: u8,
    pub shown_ale: i32,
    pub friendly_troops: i32,
    pub enemy_troops: i32,
    pub sow_shortfall: u8,
    pub tax_suppressed: u8,
    pub event_id: u16,
    pub mercenary_offer: u8,
    pub castle_ruined: u8,
    pub castle_degraded: u8,
    pub castle_percent: u8,
    pub castle_work_left: i32,
    pub castle_stone_owed: i32,
    pub castle_wood_owed: i32,
    pub castle_work_total: i32,
    pub castle_stone_total: i32,
    pub castle_wood_total: i32,
    pub siege_moat_filled: u16,
    pub siege_wall_damage: u16,
    pub siege_breach_score: i32,
    pub siege_approach_score: i32,
    pub siege_ramparts_breached: u8,
    pub siege_gate_open: u8,
    pub castle_level_left: u8,
    pub event_population_pct: i8,
    pub event_grain_pct: i8,
    pub event_herd_pct: i8,
    pub fields_grain_sown: u8,
    pub fields_grain_standing: u8,
    pub reclaim_fields_finishing: i32,
    pub reclaim_seasons_to_next: i32,
    pub ale_happiness_given: u8,
    pub grain_change_expected: i32,
    pub grain_sown_expected: i32,
    pub grain_weather_change: i32,
    pub herd_change_expected: i32,
    pub herd_births_expected: i32,
    pub herd_deaths_expected: i32,
    pub herd_weather_change: i32,
    pub herd_event_change: i32,
    pub grain_event_change: i32,
    pub weapon_type: u8,
    pub levy_surcharge: i32,
    pub event_population_swing: i32,
    pub grain_grown_expected: i32,
}

impl Save {
    pub fn county_stored(&self, index: usize) -> Result<CountyStored, SaveError> {
        if index >= COUNTY_RECORDS {
            return Err(SaveError::OutOfRange { index, count: COUNTY_RECORDS });
        }
        let b = COUNTY_BASE + (index * COUNTY_STRIDE) as u32;
        let mut labour = [[0i32; LABOUR_JOBS]; 3];
        for (word, row) in labour.iter_mut().enumerate() {
            for (job, n) in row.iter_mut().enumerate() {
                *n = self.i32_at(b + 0x0C4 + job as u32 * 0x0C + word as u32 * 4)?;
            }
        }
        let mut labour_share = [0i32; LABOUR_JOBS - 1];
        for (job, n) in labour_share.iter_mut().enumerate() {
            *n = self.i32_at(b + 0x130 + job as u32 * 4)?;
        }
        let mut industry = [IndustryRecord::default(); INDUSTRIES];
        for (c, r) in industry.iter_mut().enumerate() {
            let at = b + 0x294 + c as u32 * 0x18;
            *r = IndustryRecord {
                efficiency: self.u8_at(at)?,
                has_resource: self.u8_at(at + 1)?,
                disabled_seasons: self.u8_at(at + 2)?,
                enabled: self.u8_at(at + 3)?,
                capacity: self.i16_at(at + 0x0A)?,
                total: self.i32_at(at + 0x0C)?,
                total_snapshot: self.i32_at(at + 0x10)?,
                next_season: self.i32_at(at + 0x14)?,
            };
        }
        let mut field_tiles = [0i32; COUNTY_FIELD_SLOTS];
        for (slot, t) in field_tiles.iter_mut().enumerate() {
            *t = self.i32_at(COUNTY_FIELD_TILES + index as u32 * COUNTY_FIELD_STRIDE + slot as u32 * 4)?;
        }
        let mut inflow_sources = [0u8; 16];
        for (slot, id) in inflow_sources.iter_mut().enumerate() {
            *id = self.u8_at(b + 0x048 + slot as u32)?;
        }
        let mut field_progress = [0u16; 20];
        for (slot, p) in field_progress.iter_mut().enumerate() {
            *p = self.u16_at(b + 0x090 + slot as u32 * 2)?;
        }
        Ok(CountyStored {
            industry_share: self.i8_at(b + 0x008)?,
            d_hap_tax_local: self.i8_at(b + 0x00F)?,
            tax_shown: self.i32_at(b + 0x0C0)?,
            labour,
            labour_share,
            castle_switch: self.u8_at(b + 0x1B0)?,
            purse: self.i32_at(b + 0x1F4)?,
            merchant_visits: self.i32_at(b + 0x1A0)?,
            merchant_count: self.u8_at(b + 0x1A4)?,
            merchant_unit: self.u8_at(b + 0x1A5)?,
            farm_style: self.u8_at(b + 0x1FE)?,
            industry,
            field_tiles,
            event_fired: self.u8_at(b + 0x000)?,
            tax_hap_other: self.i8_at(b + 0x016)?,
            unrest_warned: self.u8_at(b + 0x021)?,
            pop_change_pct: self.i32_at(b + 0x02C)?,
            army: self.i32_at(b + 0x038)?,
            largest_inflow: self.i32_at(b + 0x044)?,
            inflow_sources,
            emigrant_destination: self.u8_at(b + 0x058)?,
            largest_inflow_source: self.u8_at(b + 0x059)?,
            change_reason: self.u8_at(b + 0x05B)?,
            field_progress,
            pasture_cursor: self.u8_at(b + 0x15A)?,
            blight_cursor: self.u8_at(b + 0x15B)?,
            shown_ale: self.i32_at(b + 0x194)?,
            friendly_troops: self.i32_at(b + 0x198)?,
            enemy_troops: self.i32_at(b + 0x19C)?,
            sow_shortfall: self.u8_at(b + 0x1A7)?,
            tax_suppressed: self.u8_at(b + 0x1A8)?,
            event_id: self.u16_at(b + 0x1AA)?,
            mercenary_offer: self.u8_at(b + 0x1AD)?,
            castle_ruined: self.u8_at(b + 0x1C2)?,
            castle_degraded: self.u8_at(b + 0x1C3)?,
            castle_percent: self.u8_at(b + 0x1C4)?,
            castle_work_left: self.i32_at(b + 0x1CC)?,
            castle_stone_owed: self.i32_at(b + 0x1D0)?,
            castle_wood_owed: self.i32_at(b + 0x1D4)?,
            castle_work_total: self.i32_at(b + 0x1D8)?,
            castle_stone_total: self.i32_at(b + 0x1DC)?,
            castle_wood_total: self.i32_at(b + 0x1E0)?,
            siege_moat_filled: self.u16_at(b + 0x1E4)?,
            siege_wall_damage: self.u16_at(b + 0x1E6)?,
            siege_breach_score: self.i32_at(b + 0x1E8)?,
            siege_approach_score: self.i32_at(b + 0x1EC)?,
            siege_ramparts_breached: self.u8_at(b + 0x1F0)?,
            siege_gate_open: self.u8_at(b + 0x1F1)?,
            castle_level_left: self.u8_at(b + 0x1F9)?,
            event_population_pct: self.i8_at(b + 0x1FB)?,
            event_grain_pct: self.i8_at(b + 0x1FC)?,
            event_herd_pct: self.i8_at(b + 0x1FD)?,
            fields_grain_sown: self.u8_at(b + 0x202)?,
            fields_grain_standing: self.u8_at(b + 0x206)?,
            reclaim_fields_finishing: self.i32_at(b + 0x20C)?,
            reclaim_seasons_to_next: self.i32_at(b + 0x214)?,
            ale_happiness_given: self.u8_at(b + 0x219)?,
            grain_change_expected: self.i32_at(b + 0x22C)?,
            grain_sown_expected: self.i32_at(b + 0x230)?,
            grain_weather_change: self.i32_at(b + 0x24C)?,
            herd_change_expected: self.i32_at(b + 0x258)?,
            herd_births_expected: self.i32_at(b + 0x268)?,
            herd_deaths_expected: self.i32_at(b + 0x26C)?,
            herd_weather_change: self.i32_at(b + 0x270)?,
            herd_event_change: self.i32_at(b + 0x274)?,
            grain_event_change: self.i32_at(b + 0x278)?,
            weapon_type: self.u8_at(b + 0x290)?,
            levy_surcharge: self.i32_at(b + 0x2F4)?,
            event_population_swing: self.i32_at(b + 0x2F8)?,
            grain_grown_expected: self.i32_at(b + 0x2FC)?,
        })
    }
}
