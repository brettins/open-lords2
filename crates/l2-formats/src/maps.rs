//! `L2_maps.dat` — the campaign / skirmish map container.
//!
//! A flat array of fixed-size map slots. No header, no table of contents, no
//! delimiters — the file is exactly `slotCount * 32961` bytes.
//!
//! ```text
//! slot + 0x0000  6 planes of 64x64 bytes, one per tile
//! slot + 0x6000  a 65x129 isometric screen lattice
//!                = 6*4096 + 8385 = 32961 bytes per slot
//! ```
//!
//! The DOS release holds 40 slots, the Windows release 80 — its first
//! 1,318,440 bytes are byte-identical to the DOS file, so the appended slots
//! are simply more maps in the same layout.
//!
//! See `docs/formats/maps.md`. Plane meanings beyond `Flags` and `County` are
//! inferred rather than proven; [`Plane`] says which.

use crate::{Error, Result};

pub const PLANE_DIM: usize = 64;
pub const PLANE_LEN: usize = PLANE_DIM * PLANE_DIM;
pub const PLANE_COUNT: usize = 6;

pub const LATTICE_W: usize = 65;
pub const LATTICE_H: usize = 129;
pub const LATTICE_LEN: usize = LATTICE_W * LATTICE_H;

/// 32,961 bytes. The engine seeks to `slotIndex * 0x80C1`.
pub const SLOT_LEN: usize = PLANE_COUNT * PLANE_LEN + LATTICE_LEN;

/// Highest real county id. The engine clamps at `< 0x11` when counting
/// counties. The value 32 also appears in the county plane but is not a county.
pub const MAX_COUNTY_ID: u8 = 16;

/// The six 64x64 byte planes, in file order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plane {
    /// Bitfield. See [`flags`]. **Verified.**
    Flags = 0,
    /// Selects one of five sprite banks. *Inferred.*
    GfxBank = 1,
    /// Indexes a descriptor within the bank. *Inferred.*
    GfxIndex = 2,
    /// Part index within a multi-tile object. *Inferred.*
    ObjectPart = 3,
    /// Marker payload; the five settlement tiles carrying 1..5 are the player
    /// start table. *Partly inferred.*
    Marker = 4,
    /// County id, 1..16 (plus 32). **Verified** — the loader counts counties by
    /// clamping this plane at `< 0x11`.
    County = 5,
}

/// Bits of [`Plane::Flags`].
pub mod flags {
    /// Tile belongs to no county. Holds for 100% of tiles: `(f & 0x04)` is set
    /// exactly when the county id is 0.
    pub const NO_COUNTY: u8 = 0x04;
    /// Dwelling. Confirmed against the housing-placement routine.
    pub const DWELLING: u8 = 0x20;
    /// Castle. These tiles form complete 2x2 blocks, four per county.
    pub const CASTLE: u8 = 0x40;
    /// Settlement.
    pub const SETTLEMENT: u8 = 0x80;
}

/// The whole `L2_maps.dat` file.
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

    /// Indices of slots holding an actual map. Unused slots are all-zero across
    /// every plane; the shipped files leave 36 of 80 empty.
    pub fn used_slots(&self) -> Vec<usize> {
        (0..self.slot_count())
            .filter(|&i| self.slot(i).map(|s| !s.is_empty()).unwrap_or(false))
            .collect()
    }
}

/// One map.
pub struct MapSlot<'a> {
    data: &'a [u8],
}

impl<'a> MapSlot<'a> {
    pub fn plane(&self, plane: Plane) -> &'a [u8] {
        let start = plane as usize * PLANE_LEN;
        &self.data[start..start + PLANE_LEN]
    }

    /// The trailing 65x129 isometric screen lattice. At runtime the engine
    /// overwrites most cells with pointers into its tile array; the bytes on
    /// disk are background tile graphic indices for the off-map surround.
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

    /// An unused slot: every tile plane is zero. Empty slots still carry a
    /// non-zero lattice (a blank template), so the planes are the test.
    pub fn is_empty(&self) -> bool {
        self.data[..PLANE_COUNT * PLANE_LEN].iter().all(|&b| b == 0)
    }

    /// Number of real counties on this map.
    ///
    /// Counts distinct ids in `1..=16` only, matching the engine, which clamps
    /// at `< 0x11` when counting. Id **32** also occurs in the county plane but
    /// is not a county — counting it gives one castle block too few per map,
    /// which is how this was caught.
    pub fn county_count(&self) -> usize {
        let mut seen = [false; 256];
        for &c in self.plane(Plane::County) {
            seen[c as usize] = true;
        }
        (1..=MAX_COUNTY_ID).filter(|&i| seen[i as usize]).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_length_is_six_planes_plus_the_lattice() {
        assert_eq!(SLOT_LEN, 6 * 64 * 64 + 65 * 129);
        assert_eq!(SLOT_LEN, 32_961);
        // The stride the engine seeks by.
        assert_eq!(SLOT_LEN, 0x80C1);
        // Both shipped releases divide exactly.
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
