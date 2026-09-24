//! The coast and the margin's variety: `FUN_0046DBF5` then `FUN_0046DD34`, run by
//! `Map_InitScenario` (`0x004676E0`), `FUN_004976A1` (Save_Read) and `FUN_004788C6`
//! (back from a battle). [V] call sites.

use super::Lattice;
use l2_formats::maps::{LATTICE_H, LATTICE_W};
use l2_sim::terrain::Lfsr;

/// `0x004D6C28` (even rows) and `0x004D6BE8` (odd rows): (col, row) steps of the
/// eight neighbours, `FUN_0046DE05`. [V] exe bytes.
const EVEN: [(i32, i32); 8] = [(1, -1), (1, 0), (1, 1), (0, 2), (0, 1), (-1, 0), (0, -1), (0, -2)];
const ODD: [(i32, i32); 8] = [(0, -1), (1, 0), (0, 1), (0, 2), (-1, 1), (-1, 0), (-1, -1), (0, -2)];

/// `0x004D7310`, the 0x30 twelve-byte entries `FUN_0046C2DE(0x4D7310, 0x30)` walks:
/// mask (0 sea, 1 land, 2 either), +8 base frame, +10 variants; entry 47 (all sea, 0)
/// is behind entry 46 and never hit. [V] exe bytes.
const COAST: [([u8; 8], u8, u8); 47] = [
    ([1, 2, 1, 2, 1, 2, 1, 2], 78, 2), ([1, 2, 1, 2, 1, 2, 0, 2], 75, 1),
    ([0, 2, 1, 2, 1, 2, 1, 2], 76, 1), ([1, 2, 0, 2, 1, 2, 1, 2], 77, 1),
    ([1, 2, 1, 2, 0, 2, 1, 2], 74, 1), ([1, 2, 0, 2, 1, 2, 0, 2], 54, 2),
    ([0, 2, 1, 2, 0, 2, 1, 2], 56, 2), ([1, 2, 1, 2, 0, 0, 0, 2], 58, 4),
    ([0, 2, 1, 2, 1, 2, 0, 0], 62, 4), ([0, 0, 0, 2, 1, 2, 1, 2], 66, 4),
    ([1, 2, 0, 0, 0, 2, 1, 2], 70, 4), ([1, 2, 1, 2, 0, 1, 0, 2], 92, 1),
    ([0, 2, 1, 2, 1, 2, 0, 1], 93, 1), ([0, 1, 0, 2, 1, 2, 1, 2], 94, 1),
    ([1, 2, 0, 1, 0, 2, 1, 2], 95, 1), ([1, 2, 0, 0, 0, 0, 0, 2], 38, 4),
    ([0, 2, 1, 2, 0, 0, 0, 0], 42, 4), ([0, 0, 0, 2, 1, 2, 0, 0], 46, 4),
    ([0, 0, 0, 0, 0, 2, 1, 2], 50, 4), ([1, 2, 0, 1, 0, 0, 0, 2], 80, 1),
    ([0, 2, 1, 2, 0, 1, 0, 0], 83, 1), ([0, 0, 0, 2, 1, 2, 0, 1], 86, 1),
    ([0, 1, 0, 0, 0, 2, 1, 2], 89, 1), ([1, 2, 0, 0, 0, 1, 0, 2], 81, 1),
    ([0, 2, 1, 2, 0, 0, 0, 1], 84, 1), ([0, 1, 0, 2, 1, 2, 0, 0], 87, 1),
    ([0, 0, 0, 1, 0, 2, 1, 2], 90, 1), ([1, 2, 0, 1, 0, 1, 0, 2], 82, 1),
    ([0, 2, 1, 2, 0, 1, 0, 1], 85, 1), ([0, 1, 0, 2, 1, 2, 0, 1], 88, 1),
    ([0, 1, 0, 1, 0, 2, 1, 2], 91, 1), ([0, 1, 0, 1, 0, 1, 0, 1], 122, 1),
    ([0, 1, 0, 1, 0, 1, 0, 0], 120, 1), ([0, 0, 0, 1, 0, 1, 0, 1], 121, 1),
    ([0, 1, 0, 0, 0, 1, 0, 1], 118, 1), ([0, 1, 0, 1, 0, 0, 0, 1], 119, 1),
    ([0, 1, 0, 0, 0, 1, 0, 0], 116, 1), ([0, 0, 0, 1, 0, 0, 0, 1], 117, 1),
    ([0, 1, 0, 1, 0, 0, 0, 0], 114, 1), ([0, 0, 0, 1, 0, 1, 0, 0], 115, 1),
    ([0, 0, 0, 0, 0, 1, 0, 1], 112, 1), ([0, 1, 0, 0, 0, 0, 0, 1], 113, 1),
    ([0, 1, 0, 0, 0, 0, 0, 0], 104, 4), ([0, 0, 0, 1, 0, 0, 0, 0], 108, 4),
    ([0, 0, 0, 0, 0, 1, 0, 0], 96, 4), ([0, 0, 0, 0, 0, 0, 0, 1], 100, 4),
    ([0, 0, 0, 0, 0, 0, 0, 0], 22, 8),
];

/// Surround byte `FUN_0046DE94` reads as land: `cell == 0xFFF0006`. [V]
const GRASS: u8 = 6;
/// `DAT_0057C8A4 == 0x16`, the open-sea entry, takes `LFSR & 7`. [V]
const OPEN_SEA: u8 = 0x16;

impl Lattice {
    /// Rewrites the sea's surround cells and returns `(x, y, frame)` for every sea
    /// tile, `g_tiles[t].frame = DAT_00568200 + DAT_0057C8A4` (`FUN_0046DBF5`);
    /// then grass surround `6 + (LFSR & 0xF)` (`FUN_0046DD34`). `sea(x, y)` is
    /// `g_tiles[t].flags & 4`.
    pub fn dress(&mut self, sea: &dyn Fn(usize, usize) -> bool, seed: u32) -> Vec<(usize, usize, u8)> {
        let mut rng = Lfsr::new(seed);
        let mut turn = [0u8; COAST.len()];
        // DAT_0057C8A4 / DAT_00568200 keep the last hit's values on a miss. [V]
        let (mut base, mut variant) = (0u8, 0u8);
        let mut tiles = Vec::new();
        for row in 0..LATTICE_H as i32 {
            for col in 0..LATTICE_W as i32 {
                if self.is_land(row, col, sea) {
                    continue;
                }
                let steps = if row & 1 == 0 { &EVEN } else { &ODD };
                let land = steps.map(|(dc, dr)| self.is_land(row + dr, col + dc, sea));
                if let Some(i) = COAST.iter().position(|(mask, _, _)| {
                    mask.iter().zip(land).all(|(&m, l)| m == 2 || (m == 1) == l)
                }) {
                    turn[i] += 1;
                    if COAST[i].2 <= turn[i] {
                        turn[i] = 0;
                    }
                    base = COAST[i].1;
                    variant = turn[i];
                }
                if base == OPEN_SEA {
                    variant = rng.next() & 7;
                }
                let frame = variant.wrapping_add(base);
                match self.tile(row, col) {
                    Some((x, y)) => tiles.push((x, y, frame)),
                    None => self.surround[row as usize * LATTICE_W + col as usize] = frame,
                }
            }
        }
        for cell in self.surround.iter_mut().zip(&self.tiles).filter(|(_, t)| **t == 0) {
            if *cell.0 == GRASS {
                *cell.0 = GRASS + (rng.next() & 0xF);
            }
        }
        tiles
    }

    /// `FUN_0046DE94`: off the lattice is sea. [V]
    fn is_land(&self, row: i32, col: i32, sea: &dyn Fn(usize, usize) -> bool) -> bool {
        if self.index(row, col).is_none() {
            return false;
        }
        match self.tile(row, col) {
            Some((x, y)) => !sea(x, y),
            None => self.surround(row, col) == GRASS,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank() -> Lattice {
        Lattice { tiles: vec![0; LATTICE_W * LATTICE_H], surround: vec![22; LATTICE_W * LATTICE_H] }
    }

    #[test]
    fn open_sea_takes_22_to_29_and_grass_6_to_21() {
        let mut l = blank();
        l.surround[40 * LATTICE_W + 30] = GRASS;
        l.dress(&|_, _| true, 1);
        assert!(l.surround[..30 * LATTICE_W].iter().all(|f| (22..30).contains(f)));
        assert!(l.surround.iter().any(|&f| f != 22), "the sea is varied");
        assert!((6..22).contains(&l.surround[40 * LATTICE_W + 30]));
        let mut other = blank();
        other.dress(&|_, _| true, 2);
        assert_ne!(other.surround[..LATTICE_W], l.surround[..LATTICE_W], "open sea is LFSR & 7, not the counter");
    }

    /// A lone grass cell: the sea cell two rows above has land at step 3 (0, +2)
    /// only, entry `([0,0,0,1,0,0,0,0], 108, 4)`, first hit +1; the one below, step 7, 100 + 1.
    #[test]
    fn a_sea_cell_beside_land_takes_the_coast_entry() {
        let mut l = blank();
        l.surround[40 * LATTICE_W + 30] = GRASS;
        l.dress(&|_, _| true, 1);
        assert_eq!(l.surround[38 * LATTICE_W + 30], 109);
        assert_eq!(l.surround[42 * LATTICE_W + 30], 101);
    }
}
