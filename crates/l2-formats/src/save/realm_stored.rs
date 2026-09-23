//! The realm fields past [`Realm`]'s first cut. A field's
//! `docs/stored-fields.json` id is `Realm+` the offset its reader names.
use super::*;

/// `+0x02D` — `Army_PickName`'s per-name counters.
pub const REALM_ARMY_NAMES: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RealmStored {
    pub mean_happiness: u8,
    pub population_total: i32,
    pub population_mean: i32,
    pub population_last: i32,
    pub offer_pending: u8,
    pub peak_counties: u8,
    pub army_count: u8,
    pub army_names: [u8; REALM_ARMY_NAMES],
    pub muster_timer: u8,
    pub threat_realm: u8,
    pub attack_county: u8,
    pub castle_count: u8,
    pub total_men: i32,
    pub mean_health: u8,
    pub share_of_map_pct: u8,
    pub weapon_rota: i32,
    pub want: [i32; 4],
    pub ally_candidate: u8,
    pub ally: u8,
    pub muster_county: u8,
    pub raid_county: u8,
    pub target_county: u8,
    pub taunt_timer: u8,
    pub taunt_stage: u8,
    pub war_target: u8,
    pub offer_timer: i8,
    pub crowned_once: u8,
    pub tax_ledger: [i32; 2],
    pub trade_spent_a: i32,
    pub trade_spent_b: i32,
    pub trade_received_a: i32,
    pub trade_received_b: i32,
    pub bankrupt_stage: u8,
    pub voice_rotation: u8,
    pub raid_timer: u8,
}

impl Save {
    pub fn realm_stored(&self, index: usize) -> Result<RealmStored, SaveError> {
        if index >= REALM_RECORDS {
            return Err(SaveError::OutOfRange { index, count: REALM_RECORDS });
        }
        let b = REALM_BASE + (index * REALM_STRIDE) as u32;
        let mut army_names = [0u8; REALM_ARMY_NAMES];
        for (slot, n) in army_names.iter_mut().enumerate() {
            *n = self.u8_at(b + 0x02D + slot as u32)?;
        }
        let mut want = [0i32; 4];
        for (i, w) in want.iter_mut().enumerate() {
            *w = self.i32_at(b + 0x070 + i as u32 * 4)?;
        }
        Ok(RealmStored {
            mean_happiness: self.u8_at(b + 0x00C)?,
            population_total: self.i32_at(b + 0x010)?,
            population_mean: self.i32_at(b + 0x014)?,
            population_last: self.i32_at(b + 0x018)?,
            offer_pending: self.u8_at(b + 0x01C)?,
            peak_counties: self.u8_at(b + 0x02A)?,
            army_count: self.u8_at(b + 0x02C)?,
            army_names,
            muster_timer: self.u8_at(b + 0x045)?,
            threat_realm: self.u8_at(b + 0x048)?,
            attack_county: self.u8_at(b + 0x04B)?,
            castle_count: self.u8_at(b + 0x04C)?,
            total_men: self.i32_at(b + 0x054)?,
            mean_health: self.u8_at(b + 0x058)?,
            share_of_map_pct: self.u8_at(b + 0x060)?,
            weapon_rota: self.i32_at(b + 0x06C)?,
            want,
            ally_candidate: self.u8_at(b + 0x080)?,
            ally: self.u8_at(b + 0x081)?,
            muster_county: self.u8_at(b + 0x0E5)?,
            raid_county: self.u8_at(b + 0x0E6)?,
            target_county: self.u8_at(b + 0x0E8)?,
            taunt_timer: self.u8_at(b + 0x0E9)?,
            taunt_stage: self.u8_at(b + 0x0EA)?,
            war_target: self.u8_at(b + 0x0EB)?,
            offer_timer: self.i8_at(b + 0x0EC)?,
            crowned_once: self.u8_at(b + 0x0ED)?,
            tax_ledger: [self.i32_at(b + 0x0F4)?, self.i32_at(b + 0x0F8)?],
            trade_spent_a: self.i32_at(b + 0x104)?,
            trade_spent_b: self.i32_at(b + 0x108)?,
            trade_received_a: self.i32_at(b + 0x10C)?,
            trade_received_b: self.i32_at(b + 0x110)?,
            bankrupt_stage: self.u8_at(b + 0x158)?,
            voice_rotation: self.u8_at(b + 0x159)?,
            raid_timer: self.u8_at(b + 0x15A)?,
        })
    }
}

/// `g_tiles` (`0x00522F90`) — 4,096 eight-byte records, `y * 64 + x`.
pub const TILES: u32 = 0x0052_2F90;
pub const TILE_STRIDE: u32 = 8;
pub const TILE_RECORDS: usize = 64 * 64;

/// `g_mercBands` (`0x00568DC0`), stride `0x14`, slot 0 never a band; and
/// `g_mercBandsInPlay` (`0x00554030`). `[V]` `Save_Write` saves the table as
/// 260 bytes = 13 × `0x14`; `Mercenary_Init` (`0x004AC904`) fills `+0x02`,
/// `+0x05`, `+0x07`, `+0x08`, `+0x0C`, `+0x10` from six static arrays.
pub const MERCENARY_BANDS_VA: u32 = 0x0056_8DC0;
pub const MERCENARY_BAND_STRIDE: u32 = 0x14;
pub const MERCENARY_BANDS_IN_PLAY: u32 = 0x0055_4030;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MercenaryBandRecord {
    /// `+0x00` — the hiring unit; zero in every band of every save.
    pub hired_by: i16,
    pub start_county: u8,
    pub offered_in: u8,
    pub next_county: u8,
    pub troop: u8,
    pub countdown: i8,
    pub reload: i8,
    pub men: i32,
    pub price: i32,
    pub listed_wage: i32,
}

impl Save {
    /// One `g_tiles` record's eight bytes.
    pub fn tile(&self, index: usize) -> Result<[u8; 8], SaveError> {
        if index >= TILE_RECORDS {
            return Err(SaveError::OutOfRange { index, count: TILE_RECORDS });
        }
        let mut out = [0u8; 8];
        for (i, b) in out.iter_mut().enumerate() {
            *b = self.u8_at(TILES + index as u32 * TILE_STRIDE + i as u32)?;
        }
        Ok(out)
    }

    pub fn mercenary_bands_in_play(&self) -> Result<i32, SaveError> {
        self.i32_at(MERCENARY_BANDS_IN_PLAY)
    }

    pub fn mercenary_band(&self, band: usize) -> Result<MercenaryBandRecord, SaveError> {
        let at = MERCENARY_BANDS_VA + band as u32 * MERCENARY_BAND_STRIDE;
        Ok(MercenaryBandRecord {
            hired_by: self.i16_at(at)?,
            start_county: self.u8_at(at + 0x02)?,
            offered_in: self.u8_at(at + 0x03)?,
            next_county: self.u8_at(at + 0x04)?,
            troop: self.u8_at(at + 0x05)?,
            countdown: self.i8_at(at + 0x06)?,
            reload: self.i8_at(at + 0x07)?,
            men: self.i32_at(at + 0x08)?,
            price: self.i32_at(at + 0x0C)?,
            listed_wage: self.i32_at(at + 0x10)?,
        })
    }
}
