#![allow(unused_imports)]
use super::*;
use super::site::*;
use super::castle::*;
use super::cost::*;
use crate::tables::{MOVE_COST_BLOCKED, MOVE_COST_IMPASSABLE};

impl CampaignMap {
    /// An all-zero map. Every tile has [`flags::NO_COUNTY`] clear and county 0,
    /// which the cost map reads as passable open ground in county 0 — a blank
    /// field, which is what a test wants and what a real scenario overwrites.
    pub fn empty() -> CampaignMap {
        CampaignMap {
            terrain: vec![0; MAP_TILES],
            flags: vec![0; MAP_TILES],
            bank: vec![0; MAP_TILES],
            county: vec![0; MAP_TILES],
        }
    }

    /// Build from four 4,096-byte planes, in tile-record order `+0`, `+1`,
    /// `+2`, `+7`. `None` if any is the wrong length —
    /// `l2-scenario` already applies to a save.
    pub fn from_planes(
        terrain: &[u8],
        flags: &[u8],
        bank: &[u8],
        county: &[u8],
    ) -> Option<CampaignMap> {
        if terrain.len() != MAP_TILES
            || flags.len() != MAP_TILES
            || bank.len() != MAP_TILES
            || county.len() != MAP_TILES
        {
            return None;
        }
        Some(CampaignMap {
            terrain: terrain.to_vec(),
            flags: flags.to_vec(),
            bank: bank.to_vec(),
            county: county.to_vec(),
        })
    }

    pub fn terrain_at(&self, x: u8, y: u8) -> u8 {
        self.terrain[index(x, y)]
    }

    pub fn flags_at(&self, x: u8, y: u8) -> u8 {
        self.flags[index(x, y)]
    }

    pub fn bank_at(&self, x: u8, y: u8) -> u8 {
        self.bank[index(x, y)]
    }

    /// `Map_ResolvePick` (`0x0046D5FE`): `(tile.bank & 0x1C) == 4` — the tile
    /// is drawn from the `Mtns` set, so its `0x08` rough bit is a mountain and
    /// not a wood.
    pub fn is_mountain(&self, tile: usize) -> bool {
        self.bank[tile] & BANK_SELECTOR == BANK_MOUNTAIN
    }

    pub fn county_at(&self, x: u8, y: u8) -> u8 {
        self.county[index(x, y)]
    }

    pub fn has(&self, x: u8, y: u8, bit: u8) -> bool {
        self.flags_at(x, y) & bit != 0
    }

    pub fn set_terrain(&mut self, x: u8, y: u8, value: u8) {
        self.terrain[index(x, y)] = value;
    }

    pub fn set_flags(&mut self, x: u8, y: u8, value: u8) {
        self.flags[index(x, y)] = value;
    }

    pub fn set_county(&mut self, x: u8, y: u8, value: u8) {
        self.county[index(x, y)] = value;
    }

    /// `Move_BuildCostMap` (`0x0046FF43`) — the whole 64×64 `i16` cost map,
    /// rebuilt from the three planes.
    ///
    /// **The test order is the rule**, because the bits combine: a farmland
    /// tile that is also a road is a road, and a settlement that has been
    /// ruined is impassable. The chain, in the order the original's nested
    /// `if`/`else` makes it:
    ///
    /// ```text
    /// 1. flags & 0x0C            -> 0     sea, mountain or woodland
    /// 2. county >= 17            -> 0     not a county on this map
    /// 3. flags & 0x01            -> 1     road
    /// 4. flags & 0x20            -> 3 if terrain < 2; 6 if terrain < 0x17; else 3
    /// 5. flags & 0x40            -> 100   castle site
    /// 6. flags & 0x80            -> 3 if terrain == 0x14
    ///                               0 if terrain in {3, 6, 12, 9}   (ruined)
    ///                               else 100
    /// 7. flags & 0x10            -> 100 if terrain == 0x10; else 0
    /// 8. otherwise               -> 3     open ground
    /// ```
    ///
    /// `[V]` — road 1, open 3 and field 6 agree exactly with the *stepper*,
    /// which classifies the same bits independently (`docs/armies.md` §2.2),
    /// and the field's 6 is assembled there from two separate `+3`s. `[D]` on
    /// the four 100s and the two 0s
/// about because those tiles are never entered.
    ///
    /// **Nothing about units or ownership enters this.** Byte `+5` of the tile
    /// record — the occupying unit — is never read, so armies path straight
    /// through each other and through enemy stacks; blocking is resolved at
    /// step time instead. Ownership only decides what a step *does* to the
    /// tile, never whether it is cheap.
    pub fn cost_map(&self) -> CostMap {
        let mut cost = vec![0i16; MAP_TILES];
        for i in 0..MAP_TILES {
            let f = self.flags[i];
            let t = self.terrain[i];
            cost[i] = if f & flags::IMPASSABLE != 0 {
                MOVE_COST_IMPASSABLE as i16
            } else if self.county[i] > crate::county::MAX_COUNTY_ID {
                MOVE_COST_IMPASSABLE as i16
            } else if f & flags::ROAD != 0 {
                crate::tables::STEP_COST_ROAD as i16
            } else if f & flags::FARMLAND != 0 {
                if t < terrain::FIELD_STANDING_FROM || t >= terrain::FIELD_STANDING_TO {
                    crate::tables::STEP_COST_OPEN as i16
                } else {
                    (crate::tables::STEP_COST_OPEN + crate::tables::STEP_COST_FIELD_EXTRA) as i16
                }
            } else if f & flags::CASTLE != 0 {
                MOVE_COST_BLOCKED as i16
            } else if f & flags::SETTLEMENT != 0 {
                if t == terrain::TOWN {
                    crate::tables::STEP_COST_OPEN as i16
                } else if terrain::RUINED.contains(&t) {
                    MOVE_COST_IMPASSABLE as i16
                } else {
                    MOVE_COST_BLOCKED as i16
                }
            } else if f & flags::PLOT != 0 {
                if t == terrain::DWELLING {
                    MOVE_COST_BLOCKED as i16
                } else {
                    MOVE_COST_IMPASSABLE as i16
                }
            } else {
                crate::tables::STEP_COST_OPEN as i16
            };
        }
        CostMap { cost }
    }

    /// Is this tile a standing crop — the test `Unit_CrossField` makes before
    /// it destroys anything. `[D]` — terrain 2…0x16 on a farmland tile, the
    /// same window the cost map charges 6 for.
    pub fn is_standing_field(&self, x: u8, y: u8) -> bool {
        let t = self.terrain_at(x, y);
        self.has(x, y, flags::FARMLAND)
            && (terrain::FIELD_STANDING_FROM..terrain::FIELD_STANDING_TO).contains(&t)
    }
}

