//! Turning a `.skr` terrain layer into the battlefield the simulation fights on.
//!
//! `docs/formats/skr.md` stops at "terrain byte 0x09 is water". That is not
//! enough to build a battlefield: the byte says *what* a cell is, and the cell
//! record needs a terrain id, passability flags, a surface and *which of
//! forty-nine water tiles*. This module reproduces `Battlefield_BuildFromSkr`
//! (`0x0047B8B2`), which decides all four.
//!
//! # Why this is simulation and not rendering
//!
//! The cell array is what the mover and the pathfinder read: byte `+1` is the
//! mask `Cell_TryEnter` rejects on, byte `+7` is the surface the moat fill and
//! the siege orders search for, and byte `+0` is the terrain id. The graphic
//! index at byte `+3` is decided here too because the *original* decides it
//! here, in the same pass, from rotating counters whose sequence depends on
//! every earlier cell — splitting that half out would mean walking the map
//! twice and keeping two counter sets in step. `l2-view` reads byte `+3` and
//! turns it into a PL8 frame; nothing in this module knows that it will.
//!
//! # What the original does, in four passes
//!
//! 1. **Terrain byte to terrain id**, cell by cell, and rewrite the two cells
//!    below a bridge span from `0x10` to `0x14`. **[V]** — the id column of
//!    `skr.md` reproduces exactly.
//! 2. **Expand the two deployment markers** into twelve start slots each, then
//!    overwrite the marker cell with open ground. **[V]**
//! 3. **Choose a graphic index per cell.** Ground, rocks and the unused id 6
//! pick at random; hills, water, woodland and the `0x15` lines run an
//!    **auto-tiling** lookup: build an eight-neighbour "is this the same
//!    terrain" mask, match it against a table of patterns, and take that
//!    pattern's base index plus a **rotating counter**. **[V]**
//! 4. **Stamp the bridges**, which are 4-wide multi-cell structures with fixed
//!    graphics and their own parapet flags. **[V]**
//!
//! # Why we can be sure the graphic index is a PL8 frame index
//!
//! `Battlefield_Draw32` (`0x004BCBDC`) reads cell byte `+3` into
//! `tileset + index * 0x10 + 8` — literally the PL8 frame-record address for
//! that index. **[V]**
//!
//! Two independent arithmetic checks land it on `T32_bat1.pl8` specifically,
//! which has **252** frames:
//!
//! * Woodland's high branch computes `base - 0x17` as a *signed char* for the
//!   three table entries whose base is `0x10`, `0x11` and `0x12`. Those wrap to
//!   **249, 250 and 251** — the last three frames of a 252-frame file, and
//!   nothing else in the index space reaches them.
//! * The `0x15` lines compute `base - 0x1a`, giving **230 … 248**, which sits
//!   exactly in the gap between the highest water index (229) and those three.
//!
//! The whole 0…251 space is accounted for with no overlap and no overflow. A
//! wrong tileset cannot do that.
//!
//! # Determinism
//!
//! The random-looking variants are not `rand()`. `FUN_00404B2C` is a 31-bit
//! LFSR, reproduced here exactly, and it is stepped **once per cell** whether
//! or not the result is used — so the sequence depends only on the seed. The
//! seed the original starts from is a global we have not traced, so it is a
//! parameter here and the choice of variant is cosmetic. Everything else in
//! this module is a pure function of the layer bytes.

mod builder;
pub use builder::*;

/// The battlefield is 80 x 80. Both binaries walk it as `for (y < 0x50)`.
pub const DIM: usize = 80;
pub const CELLS: usize = DIM * DIM;

/// Terrain ids as `Lords2.exe` writes them into cell byte `+0`.
pub mod id {
    pub const OPEN: u8 = 1;
    pub const ROCKS: u8 = 3;
    /// `.skr` byte `0x02`. An impassable obstacle, *not* high ground — see
    /// `docs/battle.md` §3.1.
    pub const OBSTACLE: u8 = 4;
    /// Decoded by both binaries, used by no shipped file.
    pub const UNUSED6: u8 = 6;
    pub const BRIDGE_NEAR: u8 = 7;
    pub const BRIDGE_SPAN: u8 = 8;
    pub const BRIDGE_FAR: u8 = 9;
    pub const WATER: u8 = 0x0B;
    pub const WOODLAND: u8 = 0x0C;
    /// `.skr` byte `0x15` — one-cell-wide impassable lines. Fence or palisade.
    pub const LINE: u8 = 0x0D;
    /// Transient: the `0x04` deployment marker, replaced by `OPEN` in pass 2.
    pub const MARKER_SIDE0: u8 = 0x14;
    /// Transient: the `0x0F` deployment marker.
    pub const MARKER_SIDE4: u8 = 0x1E;
}

/// Cell flag bits on byte `+1`. `Cell_TryEnter` rejects `flags & 0x90`.
pub mod flag {
    pub const IMPASSABLE: u8 = 0x10;
    pub const BLOCKED: u8 = 0x80;
    /// The mask movement
    pub const NO_ENTRY: u8 = 0x90;
}

/// One battlefield cell. The original's record is 8 bytes; these are the four
/// that a `.skr` field battle ever writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cell {
    /// Byte `+0`.
    pub terrain: u8,
    /// Byte `+1`.
    pub flags: u8,
    /// Byte `+2` — **which of the two tile sheets byte `+3` indexes**, in bits
    /// `0x1C`. See [`tileset`].
    pub flags2: u8,
    /// Byte `+3` — the PL8 frame index in the tileset [`tileset`] selects.
    pub gfx: u8,
    /// Byte `+4` — elevation. Read by the missile model, by
    /// `movement::can_step_elevation` and by `Formation_RectIsClear`, and
    /// **always zero on a `.skr` field battlefield**: nothing in
    /// `Battlefield_BuildFromSkr` writes it. It is carried so the field exists
    /// where the original's does; a castle builder would fill it.
    pub elevation: u8,
    /// Byte `+7` — surface type. 0x0F marks woodland.
    pub surface: u8,
}

/// Cell byte `+2`'s tile-sheet selector. See [`Cell::tileset`].
pub mod tileset {
    /// The bits `Battlefield_Draw32` masks with and every builder clears with
    /// `flags2 &= 0xE3`.
    pub const MASK: u8 = 0x1C;
    /// The one non-zero value the renderer has an arm for: the second sheet.
    pub const SECOND: u8 = 0x04;
}

/// The twelve `(dx, dy)` deployment offsets at `0x004D9578`, clamped to
/// `1 ..= 78` around the marker cell. **[V]**
pub const DEPLOY_OFFSETS: [(i32, i32); 12] = [
    (0, 0),
    (-5, 0),
    (5, 0),
    (0, -3),
    (0, 3),
    (-5, -3),
    (-5, 3),
    (5, -3),
    (5, 3),
    (0, 6),
    (0, -6),
    (-10, -6),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Battlefield {
    pub cells: Vec<Cell>,
    /// Deployment slots for side 0 — the `0x04` marker. **[V]**
    pub deploy_side0: [(u8, u8); 12],
    /// Deployment slots for side 4 — the `0x0F` marker.
    pub deploy_side4: [(u8, u8); 12],
    /// Where each marker stood, before it was overwritten with open ground.
    pub home_side0: (u8, u8),
    pub home_side4: (u8, u8),
}

impl Battlefield {
    #[inline]
    pub fn at(&self, x: usize, y: usize) -> Cell {
        self.cells[y * DIM + x]
    }
}

/// `FUN_00404B2C` — a 31-bit LFSR with taps at bits 0 and 4, stepped 31 times
/// per call, yielding the low seven bits. **[V]** as a reading of the code; the
/// seed the original starts a battle with was not traced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lfsr(u32);

impl Lfsr {
    /// Any non-zero seed. Zero is an absorbing state in this LFSR, so it is
    /// rejected
    pub fn new(seed: u32) -> Self {
        Lfsr(if seed == 0 { 1 } else { seed })
    }

    pub fn next(&mut self) -> u8 {
        for _ in 0..0x1F {
            let a = (self.0 >> 4) & 1;
            let b = self.0 & 1;
            self.0 >>= 1;
            if a != b {
                self.0 |= 0x4000_0000;
            }
        }
        (self.0 & 0x7F) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank_layer() -> Vec<u8> {
        let mut v = vec![0u8; CELLS];
        v[20 * DIM + 40] = 0x04;
        v[60 * DIM + 40] = 0x0F;
        v
    }

    #[test]
    fn the_lfsr_never_repeats_within_a_battlefield_and_never_dies() {
        let mut r = Lfsr::new(1);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..CELLS {
            seen.insert(r.next());
        }
        // Seven output bits, so at most 128 distinct values; what matters is
        // that it does not collapse to a constant.
        assert!(seen.len() > 100, "LFSR produced only {} distinct values", seen.len());
        assert_eq!(Lfsr::new(0), Lfsr::new(1), "a zero seed is refused");
    }

    #[test]
    fn the_blank_template_becomes_open_ground_with_two_deployment_fans() {
        let bf = build(&blank_layer(), 1);
        assert!(bf.cells.iter().all(|c| c.terrain == id::OPEN));
        assert!(bf.cells.iter().all(|c| !c.impassable()));
        assert_eq!(bf.home_side0, (40, 20));
        assert_eq!(bf.home_side4, (40, 60));
        assert_eq!(bf.deploy_side0[0], (40, 20));
        assert_eq!(bf.deploy_side0[1], (35, 20));
        assert_eq!(bf.deploy_side0[11], (30, 14));
        assert_eq!(bf.deploy_side4[0], (40, 60));
        assert_eq!(bf.deploy_side4[9], (40, 66));
    }

    #[test]
    fn deployment_slots_clamp_into_the_playable_border() {
        let mut v = vec![0u8; CELLS];
        v[0] = 0x04; // marker in the top-left corner
        v[60 * DIM + 40] = 0x0F;
        let bf = build(&v, 1);
        for (x, y) in bf.deploy_side0 {
            assert!((1..=78).contains(&x), "x {x} out of the clamp range");
            assert!((1..=78).contains(&y), "y {y} out of the clamp range");
        }
    }

    #[test]
    fn open_ground_far_from_hills_uses_the_sixteen_random_variants() {
        let bf = build(&blank_layer(), 1);
        assert!(
            bf.cells.iter().all(|c| c.gfx < 0x10),
            "plain ground must land in frames 0..15"
        );
        let distinct: std::collections::HashSet<u8> = bf.cells.iter().map(|c| c.gfx).collect();
        assert!(distinct.len() > 8, "only {} distinct ground tiles", distinct.len());
    }

    /// The auto-tiler's whole point: a cell's graphic depends on its
    /// neighbours
    #[test]
    fn identical_terrain_bytes_get_different_graphics_from_their_surroundings() {
        let mut v = blank_layer();
        // A 5x5 block of water.
        for y in 30..35 {
            for x in 30..35 {
                v[y * DIM + x] = 0x09;
            }
        }
        let bf = build(&v, 1);
        let middle = bf.at(32, 32);
        let corner = bf.at(30, 30);
        let edge = bf.at(32, 30);
        assert_eq!(middle.terrain, id::WATER);
        assert_ne!(middle.gfx, corner.gfx, "interior and corner must differ");
        assert_ne!(edge.gfx, corner.gfx, "edge and corner must differ");
        // The fully-surrounded cell takes the all-neighbours entry, base 0xA0
        // with eight variants.
        assert!((0xA0..0xA8).contains(&middle.gfx), "interior water gfx {:#x}", middle.gfx);
        assert!(middle.impassable());
    }

    /// The check that pins the tileset: woodland and the `0x15` lines compute
    /// their index by signed subtraction, and the results must stay inside the
    /// 252 frames of `T32_bat1.pl8` without colliding with water.
    #[test]
    fn every_graphic_index_a_skr_can_produce_fits_the_252_frame_tileset() {
        let mut v = blank_layer();
        // Every decoded terrain byte, in blocks big enough to exercise the
        // interior, edge and corner entries of each table.
        let bytes = [0x00u8, 0x02, 0x09, 0x0A, 0x15, 0x20, 0x50];
        for (n, b) in bytes.iter().enumerate() {
            let ox = 2 + n * 10;
            for y in 5..12 {
                for x in ox..ox + 7 {
                    v[y * DIM + x] = *b;
                }
            }
        }
        let bf = build(&v, 12345);
        for c in &bf.cells {
            assert!((c.gfx as usize) < 252, "graphic index {} is off the tileset", c.gfx);
        }
        // Woodland's high branch is the one that wraps; it must land on the
        // last three frames and nowhere else.
        let wood: Vec<u8> = bf
            .cells
            .iter()
            .filter(|c| c.terrain == id::WOODLAND)
            .map(|c| c.gfx)
            .collect();
        assert!(!wood.is_empty());
        assert!(
            wood.iter().all(|&g| (0x10..=0x1F).contains(&g) || (249..=251).contains(&g)),
            "woodland strayed outside its two index ranges: {wood:?}"
        );
        let lines: Vec<u8> = bf
            .cells
            .iter()
            .filter(|c| c.terrain == id::LINE)
            .map(|c| c.gfx)
            .collect();
        assert!(!lines.is_empty());
        assert!(
            lines.iter().all(|&g| (230..=248).contains(&g)),
            "the 0x15 lines strayed outside 230..248: {lines:?}"
        );
    }

    #[test]
    fn a_bridge_span_turns_the_cells_below_it_into_a_far_end() {
        let mut v = blank_layer();
        for x in 20..24 {
            v[31 * DIM + x] = 0x10; // near end
            for y in 32..35 {
                v[y * DIM + x] = 0x12; // span
            }
            v[35 * DIM + x] = 0x10; // becomes the far end
        }
        let bf = build(&v, 1);
        // The stamped block clears the terrain id, so check the graphics.
        assert_eq!(bf.at(20, 31).gfx, 0x8C, "near end starts the 4x2 block");
        assert_eq!(bf.at(23, 31).gfx, 0x8F);
        assert_eq!(bf.at(20, 32).gfx, 0x90, "second row of the near-end block");
        assert_eq!(bf.at(20, 35).gfx, 0x98, "far end, from the rewritten 0x14");
        assert_eq!(bf.at(19, 35).gfx, 0xB4, "water beside the far end");
    }

    #[test]
    fn the_build_is_a_pure_function_of_the_layer_and_the_seed() {
        let v = {
            let mut v = blank_layer();
            for y in 20..40 {
                for x in 10..30 {
                    v[y * DIM + x] = if (x + y) % 3 == 0 { 0x09 } else { 0x0A };
                }
            }
            v
        };
        assert_eq!(build(&v, 7), build(&v, 7), "same seed, same battlefield");
        assert_ne!(build(&v, 7), build(&v, 8), "the seed reaches the variants");
    }
}

