//! ```text
//! slot + 0x0000  6 planes of 64x64 bytes, one per tile
//! slot + 0x6000  a 65x129 isometric screen lattice
//!                = 6*4096 + 8385 = 32961 bytes per slot
//! ```

use crate::{Error, Result};

pub const PLANE_DIM: usize = 64;
pub const PLANE_LEN: usize = PLANE_DIM * PLANE_DIM;
pub const PLANE_COUNT: usize = 6;

pub const LATTICE_W: usize = 65;
pub const LATTICE_H: usize = 129;
pub const LATTICE_LEN: usize = LATTICE_W * LATTICE_H;

/// 32,961 bytes. The engine seeks to `slotIndex * 0x80C1`.
pub const SLOT_LEN: usize = PLANE_COUNT * PLANE_LEN + LATTICE_LEN;

pub const MAX_COUNTY_ID: u8 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plane {
    Flags = 0,
    /// Selects one of five tile banks, as `layerIndex * 4`: Base, Mtns, Roads,
    /// Town, Castle. The layer order comes from the resource table at
    /// `0x004DA050`. **Verified.**
    GfxBank = 1,
    GfxIndex = 2,
    ObjectPart = 3,
    Marker = 4,
    County = 5,
}

pub mod flags {
    pub const NO_COUNTY: u8 = 0x04;
    pub const FARMLAND: u8 = 0x20;
    pub const COUNTY_BOUNDARY: u8 = 0x02;
    pub const CASTLE: u8 = 0x40;
    pub const SETTLEMENT: u8 = 0x80;
}

pub struct MapSet<'a> {
    data: &'a [u8],
}

impl<'a> MapSet<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        if data.is_empty() || data.len() % SLOT_LEN != 0 {
            return Err(Error::PartialMapSlot { len: data.len(), slot_len: SLOT_LEN });
        }
        Ok(MapSet { data })
    }

    pub fn slot_count(&self) -> usize {
        self.data.len() / SLOT_LEN
    }

    pub fn slot(&self, index: usize) -> Result<MapSlot<'a>> {
        if index >= self.slot_count() {
            return Err(Error::FrameOutOfRange { index, count: self.slot_count() });
        }
        let start = index * SLOT_LEN;
        Ok(MapSlot { data: &self.data[start..start + SLOT_LEN] })
    }

    pub fn used_slots(&self) -> Vec<usize> {
        (0..self.slot_count())
            .filter(|&i| self.slot(i).map(|s| !s.is_empty()).unwrap_or(false))
            .collect()
    }
}

pub struct MapSlot<'a> {
    data: &'a [u8],
}

impl<'a> MapSlot<'a> {
    pub fn plane(&self, plane: Plane) -> &'a [u8] {
        let start = plane as usize * PLANE_LEN;
        &self.data[start..start + PLANE_LEN]
    }

    pub fn lattice(&self) -> &'a [u8] {
        &self.data[PLANE_COUNT * PLANE_LEN..]
    }

    #[inline]
    pub fn at(&self, plane: Plane, x: usize, y: usize) -> u8 {
        self.plane(plane)[y * PLANE_DIM + x]
    }

    pub fn county_at(&self, x: usize, y: usize) -> u8 {
        self.at(Plane::County, x, y)
    }

    pub fn flags_at(&self, x: usize, y: usize) -> u8 {
        self.at(Plane::Flags, x, y)
    }

    pub fn is_empty(&self) -> bool {
        self.data[..PLANE_COUNT * PLANE_LEN].iter().all(|&b| b == 0)
    }

    pub fn county_count(&self) -> usize {
        let mut seen = [false; 256];
        for &c in self.plane(Plane::County) {
            seen[c as usize] = true;
        }
        (1..=MAX_COUNTY_ID).filter(|&i| seen[i as usize]).count()
    }

    /// `Map_LoadPlanes` (`0x00467770`) dispatches every tile whose
    /// [`Plane::Marker`] byte is non-zero on the *flags* byte, and the two arms
    /// are different tables: a `0x40` tile — the county town — appends to a
    /// merchant route, and a `0x80` tile — the castle — is a player start.
    ///
    /// `PlayerStart_Record` (`0x0049BBE8`) then writes
    /// `g_playerStartTable[marker]` and counts it.
    ///
    /// picking a map calls `FUN_004AE5E2(g_playerStartCount)`, which sets the
    /// *Nobles* drop-down from it, and the drop-down is shortened to match.
    pub fn player_start_count(&self) -> usize {
        let mut seen = [false; 256];
        for y in 0..PLANE_DIM {
            for x in 0..PLANE_DIM {
                let marker = self.at(Plane::Marker, x, y);
                // [`flags::SETTLEMENT`] is bit `0x80`. `docs/decisions.md` C25
                // renamed what the two bits mean without renaming the two
                // constants: `0x40` is the county town and `0x80` is the castle
                // or industry site. It is the `0x80` arm that records a start.
                if marker != 0 && self.flags_at(x, y) & flags::SETTLEMENT != 0 {
                    seen[marker as usize] = true;
                }
            }
        }
        seen[1..].iter().filter(|&&s| s).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_length_is_six_planes_plus_the_lattice() {
        assert_eq!(SLOT_LEN, 6 * 64 * 64 + 65 * 129);
        assert_eq!(SLOT_LEN, 32_961);
        assert_eq!(SLOT_LEN, 0x80C1);
        assert_eq!(1_318_440 % SLOT_LEN, 0);
        assert_eq!(1_318_440 / SLOT_LEN, 40);
        assert_eq!(2_636_880 / SLOT_LEN, 80);
    }

    #[test]
    fn planes_and_lattice_are_addressed_correctly() {
        let mut buf = vec![0u8; SLOT_LEN];
        buf[Plane::County as usize * PLANE_LEN + 3 * PLANE_DIM + 5] = 7;
        buf[Plane::Flags as usize * PLANE_LEN] = flags::CASTLE;
        buf[PLANE_COUNT * PLANE_LEN] = 0x16;

        let set = MapSet::parse(&buf).unwrap();
        assert_eq!(set.slot_count(), 1);
        let slot = set.slot(0).unwrap();
        assert_eq!(slot.county_at(5, 3), 7);
        assert_eq!(slot.flags_at(0, 0) & flags::CASTLE, flags::CASTLE);
        assert_eq!(slot.lattice()[0], 0x16);
        assert_eq!(slot.lattice().len(), LATTICE_LEN);
        assert_eq!(slot.county_count(), 1);
        assert!(!slot.is_empty());
    }

    #[test]
    fn an_all_zero_slot_reads_as_empty() {
        let buf = vec![0u8; SLOT_LEN];
        let set = MapSet::parse(&buf).unwrap();
        assert!(set.slot(0).unwrap().is_empty());
        assert!(set.used_slots().is_empty());
    }

    #[test]
    fn a_partial_slot_is_refused() {
        assert!(matches!(
            MapSet::parse(&vec![0u8; SLOT_LEN + 1]),
            Err(Error::PartialMapSlot { .. })
        ));
        assert!(MapSet::parse(&[]).is_err());
    }
}
