#![allow(unused_imports)]
use super::*;
use super::pe::*;
use super::layout::*;
use types::*;

impl Save {
    pub fn open(exe: &[u8], save: &[u8]) -> Result<Save, SaveError> {
        let layout = Layout::from_executable(exe)?;
        let expected = layout.expected_len();
        if save.len() != expected {
            return Err(SaveError::SizeMismatch { expected, actual: save.len() });
        }
        Ok(Save { layout, bytes: save.to_vec() })
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    fn at(&self, va: u32) -> Result<usize, SaveError> {
        self.layout.offset_of(va).ok_or(SaveError::NotSaved { va })
    }

    pub fn u8_at(&self, va: u32) -> Result<u8, SaveError> {
        let o = self.at(va)?;
        self.bytes.get(o).copied().ok_or(SaveError::NotSaved { va })
    }

    pub fn i8_at(&self, va: u32) -> Result<i8, SaveError> {
        Ok(self.u8_at(va)? as i8)
    }

    pub fn u16_at(&self, va: u32) -> Result<u16, SaveError> {
        let o = self.at(va)?;
        let s = self.bytes.get(o..o + 2).ok_or(SaveError::NotSaved { va })?;
        Ok(u16::from_le_bytes([s[0], s[1]]))
    }

    pub fn i16_at(&self, va: u32) -> Result<i16, SaveError> {
        Ok(self.u16_at(va)? as i16)
    }

    pub fn i32_at(&self, va: u32) -> Result<i32, SaveError> {
        let o = self.at(va)?;
        let s = self.bytes.get(o..o + 4).ok_or(SaveError::NotSaved { va })?;
        Ok(i32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }

    pub fn county(&self, index: usize) -> Result<County, SaveError> {
        if index >= COUNTY_RECORDS {
            return Err(SaveError::OutOfRange { index, count: COUNTY_RECORDS });
        }
        let base = COUNTY_BASE + (index * COUNTY_STRIDE) as u32;
        Ok(County {
            index,
            owner: self.u8_at(base + 0x05)?,
            health_band: self.i8_at(base + 0x09)?,
            health_meter: self.i8_at(base + 0x0B)?,
            happiness: self.i8_at(base + 0x0C)?,
            happiness_last: self.i8_at(base + 0x0D)?,
            d_hap_tax: self.i8_at(base + 0x0E)?,
            d_hap_health: self.i8_at(base + 0x10)?,
            d_hap_ration: self.i8_at(base + 0x11)?,
            happiness_avg: self.i8_at(base + 0x18)?,
            happiness_sum: self.i32_at(base + 0x1C)?,
            shown_tax: self.i8_at(base + 0x12)?,
            shown_ration: self.i8_at(base + 0x13)?,
            shown_health: self.i8_at(base + 0x14)?,
            shown_army: self.i8_at(base + 0x15)?,
            shown_events: self.i8_at(base + 0x17)?,
            unrest: self.u8_at(base + 0x20)?,
            population: self.i32_at(base + 0x24)?,
            pop_last: self.i32_at(base + 0x28)?,
            births: self.i32_at(base + 0x30)?,
            deaths: self.i32_at(base + 0x34)?,
            emigrants: self.i32_at(base + 0x3C)?,
            immigrants: self.i32_at(base + 0x40)?,
            neighbour_count: self.u8_at(base + 0x5A)?,
            anchor_x: self.u8_at(base + 0x6C)?,
            anchor_y: self.u8_at(base + 0x6D)?,
            pop_band: self.u8_at(base + 0xB8)?,
            tax_rate: self.u8_at(base + 0xB9)?,
            tax_collected: self.i32_at(base + 0xBC)?,
            ration_achieved: self.i8_at(base + 0x15D)?,
            ration_wanted: self.i8_at(base + 0x15E)?,
            ration_split: self.i8_at(base + 0x15F)?,
            grain_eaten: self.i32_at(base + 0x178)?,
            herd_eaten: self.i32_at(base + 0x17C)?,
            grain_available: self.i32_at(base + 0x180)?,
            herd_available: self.i32_at(base + 0x184)?,
            castle_type: self.u8_at(base + 0x1C0)?,
            castle_building: self.u8_at(base + 0x1C1)?,
            fields_fallow: self.u8_at(base + 0x1FF)?,
            fields_cattle: self.u8_at(base + 0x200)?,
            fields_grain: self.u8_at(base + 0x201)?,
            fertility: self.i32_at(base + 0x208)?,
            weather: self.u8_at(base + 0x21B)?,
            dryness: self.i8_at(base + 0x21D)?,
            grain: self.i32_at(base + 0x224)?,
            herd: self.i32_at(base + 0x250)?,
            neighbours: {
                let mut ids = [0u8; NEIGHBOUR_SLOTS];
                for (slot, id) in ids.iter_mut().enumerate() {
                    *id = self.u8_at(base + 0x5C + slot as u32)?;
                }
                ids
            },
        })
    }

    pub fn counties(&self) -> Result<Vec<County>, SaveError> {
        (0..COUNTY_RECORDS).map(|i| self.county(i)).collect()
    }

    pub fn realm(&self, index: usize) -> Result<Realm, SaveError> {
        if index >= REALM_RECORDS {
            return Err(SaveError::OutOfRange { index, count: REALM_RECORDS });
        }
        let base = REALM_BASE + (index * REALM_STRIDE) as u32;
        Ok(Realm {
            index,
            ai_step: self.i32_at(base)?,
            strength: self.u8_at(base + 0x04)?,
            is_human: self.u8_at(base + 0x05)? != 0,
            lord: self.u8_at(base + 0x07)?,
            shield_index: self.u8_at(base + 0x0A)?,
            tax_hap_empire: self.i8_at(base + 0x28)?,
            county_count: self.u8_at(base + 0x29)?,
            rank: self.u8_at(base + 0x2B)?,
            score: self.i32_at(base + 0x50)?,
            wages: self.i32_at(base + 0xFC)?,
            gold: self.i32_at(base + 0x118)?,
            iron: self.i32_at(base + 0x120)?,
            stone: self.i32_at(base + 0x128)?,
            wood: self.i32_at(base + 0x130)?,
            weapons: {
                let mut w = [0i32; WEAPON_TYPES];
                for (t, slot) in w.iter_mut().enumerate() {
                    *slot = self.i32_at(base + 0x140 + (t * 4) as u32)?;
                }
                w
            },
            // **`+0x84 … +0xE3`, and nothing read it until now.** The whole
            // diplomatic matrix — every alliance, every grudge — was in the
            // file and dropped on the floor, so `scenario::from_save` ran
            // `Diplo_Init` instead. Right for a turn-one fixture, and wrong for
            // every later save: a player who loaded a mid-game file found the
            // AI had forgotten every war.
            //
            // Found because somebody built the consumer. `docs/decisions.md`
            // C83.
            pairs: {
                let mut p = [DiploPair::default(); REALM_RECORDS];
                for (other, slot) in p.iter_mut().enumerate() {
                    let at = base + 0x84 + (other * 0x10) as u32;
                    *slot = DiploPair {
                        standing: self.i8_at(at)?,
                        allied: self.u8_at(at + 1)? != 0,
                        grudge: self.u8_at(at + 2)?,
                        warnings_sent: self.u8_at(at + 3)?,
                        at_war: self.u8_at(at + 4)? != 0,
                        compliments_from: self.u8_at(at + 5)?,
                        best_gift: self.i32_at(at + 8)?,
                        has_mail: self.u8_at(at + 0x0C)? != 0,
                        help_price_multiple: self.u8_at(at + 0x0D)?,
                    };
                }
                p
            },
        })
    }

    pub fn realms(&self) -> Result<Vec<Realm>, SaveError> {
        (0..REALM_RECORDS).map(|i| self.realm(i)).collect()
    }

    /// **Everything is read, including the fields whose meaning depends on the
    /// type byte.** `+0x14F`, `+0x164` and `+0x167` are each two fields sharing
    /// one offset (`docs/armies.md` §1), so this layer reads the bytes and
    /// leaves the naming to whoever knows the type — which is the same division
    /// the rest of this module keeps.
    pub fn unit(&self, index: usize) -> Result<Unit, SaveError> {
        if index >= UNIT_RECORDS {
            return Err(SaveError::OutOfRange { index, count: UNIT_RECORDS });
        }
        let base = UNIT_BASE + (index * UNIT_STRIDE) as u32;
        Ok(Unit {
            index,
            owner: self.u8_at(base)?,
            owner_is_human: self.u8_at(base + 0x01)? != 0,
            shield: self.u8_at(base + 0x02)?,
            player_driven: self.u8_at(base + 0x06)? != 0,
            sprite_frame: self.u8_at(base + 0x07)?,
            kind: self.u8_at(base + 0x08)?,
            facing: self.u8_at(base + 0x09)?,
            x: self.u8_at(base + 0x0A)?,
            y: self.u8_at(base + 0x0B)?,
            tile_offset: self.i32_at(base + 0x0C)?,
            county: self.u8_at(base + 0x10)?,
            home_county: self.u8_at(base + 0x11)?,
            step_target_x: self.u8_at(base + 0x14)?,
            step_target_y: self.u8_at(base + 0x15)?,
            dest_x: self.u8_at(base + 0x16)?,
            dest_y: self.u8_at(base + 0x17)?,
            walk_phase: self.u8_at(base + 0x1B)?,
            path_len: self.u8_at(base + 0x1C)?,
            path: {
                let mut steps = [(0u8, 0u8); UNIT_PATH_STEPS];
                for (n, step) in steps.iter_mut().enumerate() {
                    let at = base + 0x1D + (n * 2) as u32;
                    *step = (self.u8_at(at)?, self.u8_at(at + 1)?);
                }
                steps
            },
            move_state: self.u8_at(base + 0x14C)?,
            on_road: self.u8_at(base + 0x14D)? != 0,
            ignore_settlements: self.u8_at(base + 0x14E)? != 0,
            name_index: self.u8_at(base + 0x14F)?,
            needs_destination: self.u8_at(base + 0x150)? != 0,
            dest_county: self.u8_at(base + 0x151)?,
            merge_target: self.u8_at(base + 0x152)?,
            moves_used: self.i8_at(base + 0x153)?,
            move_allowance: self.i8_at(base + 0x154)?,
            starvation: self.i8_at(base + 0x155)?,
            wages: self.i32_at(base + 0x15C)?,
            year_formed: self.u16_at(base + 0x164)?,
            morale: self.u8_at(base + 0x166)?,
            role: self.u8_at(base + 0x167)?,
            men: self.i32_at(base + 0x168)?,
            troops: {
                let mut t = [0i16; UNIT_TROOP_SLOTS];
                for (n, slot) in t.iter_mut().enumerate() {
                    *slot = self.i16_at(base + 0x16C + (n * 2) as u32)?;
                }
                t
            },
            merc_troop: self.u8_at(base + 0x195)?,
            merc_men: self.u8_at(base + 0x196)?,
            merc_band: self.u8_at(base + 0x197)?,
            garrison_county: self.u8_at(base + 0x198)?,
            besieging_county: self.u8_at(base + 0x199)?,
            besieged_by: self.u8_at(base + 0x19A)?,
            siege_seasons_left: self.u8_at(base + 0x19C)?,
        })
    }

    pub fn units(&self) -> Result<Vec<Unit>, SaveError> {
        (0..UNIT_RECORDS).map(|i| self.unit(i)).collect()
    }

    pub fn player(&self, index: usize) -> Result<Player, SaveError> {
        if index >= PLAYER_RECORDS {
            return Err(SaveError::OutOfRange { index, count: PLAYER_RECORDS });
        }
        let base = PLAYER_BASE + (index * PLAYER_STRIDE) as u32;
        let mut name = [0u8; PLAYER_NAME_LEN];
        for (n, slot) in name.iter_mut().enumerate() {
            *slot = self.u8_at(base + 0x04 + n as u32)?;
        }
        Ok(Player {
            index,
            dp_player_id: self.i32_at(base)?,
            name,
            shield: self.u8_at(base + 0x25)?,
            human_flag: self.u8_at(base + 0x27)?,
        })
    }

    pub fn players(&self) -> Result<Vec<Player>, SaveError> {
        (0..PLAYER_RECORDS).map(|i| self.player(i)).collect()
    }

    pub fn merchant_routes(
        &self,
    ) -> Result<[[u8; MERCHANT_ROUTE_LEN]; MERCHANT_ROUTE_ROWS], SaveError> {
        let mut rows = [[0u8; MERCHANT_ROUTE_LEN]; MERCHANT_ROUTE_ROWS];
        for (r, row) in rows.iter_mut().enumerate() {
            for (n, cell) in row.iter_mut().enumerate() {
                *cell = self.u8_at(MERCHANT_ROUTES + (r * MERCHANT_ROUTE_LEN + n) as u32)?;
            }
        }
        Ok(rows)
    }

    pub fn merchant_start_counties(&self) -> Result<[u8; MERCHANT_ROUTE_ROWS], SaveError> {
        let mut out = [0u8; MERCHANT_ROUTE_ROWS];
        for (r, slot) in out.iter_mut().enumerate() {
            *slot = self.u8_at(MERCHANT_START_COUNTIES + r as u32)?;
        }
        Ok(out)
    }

    pub fn globals(&self) -> Result<Globals, SaveError> {
        Ok(Globals {
            county_count: self.i32_at(globals::COUNTY_COUNT)?,
            scenario_index: self.i32_at(globals::SCENARIO_INDEX)?,
            local_player: self.i32_at(globals::LOCAL_PLAYER)?,
            season: self.i32_at(globals::SEASON)?,
            season_next: self.i32_at(globals::SEASON_NEXT)?,
            year: self.i32_at(globals::YEAR)?,
            turn_count: self.i32_at(globals::TURN_COUNT)?,
            turn_phase: self.i32_at(globals::TURN_PHASE)?,
            turn_phase_step: self.i32_at(globals::TURN_PHASE_STEP)?,
            opt_difficulty: self.i32_at(globals::OPT_DIFFICULTY)?,
            opt_advanced_farming: self.i32_at(globals::OPT_ADVANCED_FARMING)?,
            opt_armies_eat: self.i32_at(globals::OPT_ARMIES_EAT)?,
            opt_exploration: self.i32_at(globals::OPT_EXPLORATION)?,
            opt_time_limit: self.i32_at(globals::OPT_TIME_LIMIT)?,
            ai_lords: self.i32_at(globals::AI_LORDS)?,
            merchant_count: self.i32_at(globals::MERCHANT_COUNT)?,
            weather_county: self.i32_at(globals::WEATHER_COUNTY)?,
        })
    }
}


