//! ```text
//! 0x0000   1,760   army table    40 records x 44 bytes
//! 0x06E0   3,660   text table    20 records x 183 bytes
//! 0x152C 128,328   terrain       328 bytes of slack, then 20 layers of 6,400
//!        -------
//!        133,748 = 1760 + 3660 + 328 + 20 * 80 * 80
//! ```
//!
//! Every one of those numbers is a literal in `mapl2.exe`, Sierra's own
//! battlemap editor, and `Lords2.exe` independently reads map *m* at
//! `m * 0x1900 + 0x1674` — the same origin and stride computed against the file.

use crate::{Error, Result};

pub const MAP_COUNT: usize = 20;
pub const GRID_DIM: usize = 80;
pub const GRID_LEN: usize = GRID_DIM * GRID_DIM;

pub const ARMY_TABLE: usize = 0x0000;
pub const ARMY_RECORD_LEN: usize = 44;
pub const ARMY_COUNT: usize = MAP_COUNT * 2;

pub const TEXT_TABLE: usize = 0x06E0;
pub const TEXT_RECORD_LEN: usize = 183;

pub const TERRAIN_ORIGIN: usize = 0x1674;
pub const TERRAIN_STRIDE: usize = 0x1900;

pub const FILE_LEN: usize =
    ARMY_COUNT * ARMY_RECORD_LEN + MAP_COUNT * TEXT_RECORD_LEN + 328 + MAP_COUNT * GRID_LEN;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Troop {
    Peasants = 0,
    Crossbowmen = 1,
    Macemen = 2,
    Swordsmen = 3,
    Pikemen = 4,
    Archers = 5,
    Knights = 6,
    Catapults = 7,
    SiegeTowers = 8,
    BatteringRams = 9,
    Oil = 10,
}

pub const TROOP_SLOTS: usize = 11;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Attacker,
    Defender,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Army {
    pub counts: [u32; TROOP_SLOTS],
}

impl Army {
    pub fn get(&self, troop: Troop) -> u32 {
        self.counts[troop as usize]
    }
    pub fn total(&self) -> u64 {
        self.counts.iter().map(|&c| c as u64).sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioText {
    pub name: String,
    pub title: String,
    pub description: String,
}

pub struct Skr<'a> {
    data: &'a [u8],
}

impl<'a> Skr<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        if data.len() != FILE_LEN {
            return Err(Error::BadSkrLength { len: data.len(), expected: FILE_LEN });
        }
        Ok(Skr { data })
    }

    pub fn map_count(&self) -> usize {
        MAP_COUNT
    }

    pub fn army(&self, map: usize, side: Side) -> Result<Army> {
        if map >= MAP_COUNT {
            return Err(Error::FrameOutOfRange { index: map, count: MAP_COUNT });
        }
        let record = map * 2 + if side == Side::Attacker { 0 } else { 1 };
        let base = ARMY_TABLE + record * ARMY_RECORD_LEN;
        let mut counts = [0u32; TROOP_SLOTS];
        for (i, c) in counts.iter_mut().enumerate() {
            let o = base + i * 4;
            *c = u32::from_le_bytes([
                self.data[o],
                self.data[o + 1],
                self.data[o + 2],
                self.data[o + 3],
            ]);
        }
        Ok(Army { counts })
    }

    pub fn text(&self, map: usize) -> Result<ScenarioText> {
        if map >= MAP_COUNT {
            return Err(Error::FrameOutOfRange { index: map, count: MAP_COUNT });
        }
        let base = TEXT_TABLE + map * TEXT_RECORD_LEN;
        let field = |off: usize, len: usize| -> String {
            let s = &self.data[base + off..base + off + len];
            let end = s.iter().position(|&b| b == 0).unwrap_or(s.len());
            String::from_utf8_lossy(&s[..end]).into_owned()
        };
        Ok(ScenarioText {
            name: field(0, 13),
            title: field(0x0D, 29),
            description: field(0x2A, 141),
        })
    }

    pub fn terrain(&self, map: usize) -> Result<&'a [u8]> {
        if map >= MAP_COUNT {
            return Err(Error::FrameOutOfRange { index: map, count: MAP_COUNT });
        }
        let start = TERRAIN_ORIGIN + map * TERRAIN_STRIDE;
        Ok(&self.data[start..start + GRID_LEN])
    }

    pub fn terrain_at(&self, map: usize, x: usize, y: usize) -> Result<u8> {
        Ok(self.terrain(map)?[y * GRID_DIM + x])
    }

    pub fn is_blank(&self, map: usize) -> Result<bool> {
        let g = self.terrain(map)?;
        Ok(g.iter().enumerate().all(|(i, &b)| {
            let (x, y) = (i % GRID_DIM, i / GRID_DIM);
            match (x, y) {
                (40, 20) => b == markers::ATTACKER,
                (40, 60) => b == markers::DEFENDER,
                _ => b == 0,
            }
        }))
    }
}

pub mod markers {
    pub const ATTACKER: u8 = 0x04;
    pub const DEFENDER: u8 = 0x0F;
    pub const BRIDGE_HEAD: u8 = 0x10;
    pub const BRIDGE_SPAN: u8 = 0x12;
    pub const BRIDGE_END: u8 = 0x14;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_container_arithmetic_closes_exactly() {
        assert_eq!(1760 + 3660 + 328 + 20 * 80 * 80, FILE_LEN);
        assert_eq!(FILE_LEN, 133_748);
        assert_eq!(FILE_LEN, 0x20A74);
        assert_eq!(ARMY_COUNT * ARMY_RECORD_LEN, 0x6E0);
        assert_eq!(MAP_COUNT * TEXT_RECORD_LEN, 0xE4C);
        assert_eq!(328 + MAP_COUNT * GRID_LEN, 0x1F548);
        // Lords2.exe reads map m at m * 0x1900 + 0x1674.
        assert_eq!(TERRAIN_ORIGIN, 0x152C + 328);
        assert_eq!(TERRAIN_STRIDE, GRID_LEN);
        assert_eq!(TERRAIN_ORIGIN + (MAP_COUNT - 1) * TERRAIN_STRIDE + GRID_LEN, FILE_LEN);
    }

    fn blank_file() -> Vec<u8> {
        let mut v = vec![0u8; FILE_LEN];
        for m in 0..MAP_COUNT {
            let g = TERRAIN_ORIGIN + m * TERRAIN_STRIDE;
            v[g + 20 * GRID_DIM + 40] = markers::ATTACKER;
            v[g + 60 * GRID_DIM + 40] = markers::DEFENDER;
        }
        v
    }

    #[test]
    fn armies_are_addressed_as_attacker_then_defender() {
        let mut v = blank_file();
        let base = 7 * ARMY_RECORD_LEN + 4 * 4;
        v[base..base + 4].copy_from_slice(&250u32.to_le_bytes());

        let skr = Skr::parse(&v).unwrap();
        assert_eq!(skr.army(3, Side::Defender).unwrap().get(Troop::Pikemen), 250);
        assert_eq!(skr.army(3, Side::Attacker).unwrap().total(), 0);
        assert_eq!(skr.army(3, Side::Defender).unwrap().total(), 250);
    }

    #[test]
    fn text_fields_split_at_13_and_42() {
        let mut v = blank_file();
        let base = TEXT_TABLE + 2 * TEXT_RECORD_LEN;
        v[base..base + 4].copy_from_slice(b"ford");
        v[base + 0x0D..base + 0x0D + 5].copy_from_slice(b"River");
        v[base + 0x2A..base + 0x2A + 6].copy_from_slice(b"A ford");

        let t = Skr::parse(&v).unwrap().text(2).unwrap();
        assert_eq!(t.name, "ford");
        assert_eq!(t.title, "River");
        assert_eq!(t.description, "A ford");
    }

    #[test]
    fn the_blank_template_is_recognised() {
        let v = blank_file();
        let skr = Skr::parse(&v).unwrap();
        for m in 0..MAP_COUNT {
            assert!(skr.is_blank(m).unwrap(), "map {m} should read as blank");
            assert_eq!(skr.terrain_at(m, 40, 20).unwrap(), markers::ATTACKER);
            assert_eq!(skr.terrain_at(m, 40, 60).unwrap(), markers::DEFENDER);
        }

        let mut dirty = v.clone();
        dirty[TERRAIN_ORIGIN + 5 * GRID_DIM + 5] = 0x02;
        assert!(!Skr::parse(&dirty).unwrap().is_blank(0).unwrap());
    }

    #[test]
    fn a_wrong_length_is_refused() {
        assert!(matches!(
            Skr::parse(&vec![0u8; FILE_LEN - 1]),
            Err(Error::BadSkrLength { .. })
        ));
        assert!(Skr::parse(&[]).is_err());
    }

    #[test]
    fn out_of_range_maps_error_rather_than_panic() {
        let v = blank_file();
        let skr = Skr::parse(&v).unwrap();
        assert!(skr.army(MAP_COUNT, Side::Attacker).is_err());
        assert!(skr.text(MAP_COUNT).is_err());
        assert!(skr.terrain(MAP_COUNT).is_err());
    }
}
