//! **The open-field battlefield** — `Battlefield_BuildRandom` (`0x0047AAA3`),
//! the third of the original's three builders and the one a battle fought out
//! in the country uses.
//!
//! Not from the campaign tile, and not from a seed: from a **picture**. The
//! builder opens `batfield.pl8` and reads one frame's 6,400 raw bytes as an
//! 80 x 80 plane, one source byte per cell. **[V]** — the two reads are
//!
//! ```c
//! File_ReadChunk(s_batfield_pl8 + variant * 0x0E + 5, buf, 1000, 0);   // the directory
//! entry = g_battlefieldMap * 0x10;                                      // a 16-byte PL8 record
//! File_ReadChunk(name, buf, 0x1900, u24_at(entry + 0x0C));              // 0x1900 == 80 * 80
//! ```
//!
//! and the string table at `0x004D9103` holds four 14-byte names — index 0
//! `batfield.pl8`, 1 `stnfield.pl8`, 2 `batfiel2.pl8`, 3 `stnfiel2.pl8`.
//!
//! `DAT_0057A0F0`, the skirmish flag, picks 0 or 2. **[V]**, read out of the
//! binary's `.data`.
//!
//! **Which frame is a shuffled playlist, not a random draw.** `0x0057CAE0` is
//! 48 bytes filled at new-game time by scattering `0 ..= 47` with
//! `Rand_Advance` into free slots, and `Battlefield_BuildRandom` reads
//! `playlist[DAT_005653F8]`, a cursor that steps one field per battle and
//! wraps at `0x2E`. `Net_WriteField(&DAT_0057CAE0, 0x30)` sends the whole
//! playlist to every peer, so it is shared game state, not a local roll.
//!
//! **[V]** on all three sites. We hold no per-game playlist yet, so
//! [`FieldSheets::pick`] takes the map from the battle seed instead — **ours**,
//! and the only part of this module.
//!
//! **`Battlefield_BuildRandom` never writes cell byte `+4`.** `docs/battle.md`
//! §3 says elevation "must come from" this builder or the castle's; only the
//! castle's writes it. An open field is flat, and `Path_BuildElevation`
//! (`0x00471D30`), the last thing but two that this builder calls, copies the
//! zeroes straight into `g_pathElevation`. **[V]** on the decompilation.

use super::*;

/// One frame of `batfield.pl8` is one field — `0x1900` bytes, the size of both
/// builders' second `File_ReadChunk`.
pub const RASTER_BYTES: usize = CELLS;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldSheets {
    rasters: Vec<Vec<u8>>,
}

impl FieldSheets {
    /// The campaign file, index 0 of the builder's string table at
    /// `0x004D9103`.
    pub const FILE: &'static str = "batfield.pl8";
    /// The skirmish file, index 2 — `DAT_0057A0F0` picks it.
    pub const SKIRMISH_FILE: &'static str = "batfiel2.pl8";
    /// How many the playlist at `0x0057CAE0` scatters.
    pub const PLAYLIST: usize = 48;

    /// Read the PL8 directory — 16 bytes a frame, a 24-bit data offset at
    /// `+0x0C` — and take `0x1900` bytes at each. Stops at the first frame
    pub fn parse(bytes: &[u8]) -> Option<FieldSheets> {
        let dir = bytes.get(..1000.min(bytes.len()))?;
        let mut rasters = Vec::new();
        for map in 0..Self::PLAYLIST {
            let at = map * 0x10 + 0x0C;
            let Some(b) = dir.get(at..at + 3) else { break };
            let off = b[0] as usize | (b[1] as usize) << 8 | (b[2] as usize) << 16;
            if off == 0 {
                break;
            }
            let Some(r) = bytes.get(off..off + RASTER_BYTES) else { break };
            rasters.push(r.to_vec());
        }
        (!rasters.is_empty()).then_some(FieldSheets { rasters })
    }

    pub fn get(&self, map: usize) -> Option<&[u8]> {
        self.rasters.get(map).map(Vec::as_slice)
    }

    pub fn len(&self) -> usize {
        self.rasters.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rasters.is_empty()
    }

    /// **Ours, not the original's.** `DAT_005653F8` walks a playlist we do not
    /// keep; the battle seed picks the map instead. Deterministic, which is
    /// what `docs/netcode.md` asks of anything a digest sees.
    pub fn pick(&self, seed: u64) -> &[u8] {
        &self.rasters[(seed % self.rasters.len() as u64) as usize]
    }
}

pub mod field_id {
    pub const MARKER_SIDE0: u8 = 0x14;
    pub const MARKER_SIDE4: u8 = 0x1E;
    pub const RALLY_WEST: u8 = 0x28;
    pub const RALLY_EAST: u8 = 0x29;
}

pub fn build_field(raster: &[u8], seed: u32) -> Battlefield {
    assert_eq!(raster.len(), RASTER_BYTES, "a batfield.pl8 field is exactly 80 x 80 bytes");

    let mut cells = vec![Cell::default(); CELLS];
    for (cell, &b) in cells.iter_mut().zip(raster.iter()) {
        cell.terrain = match b {
            0x00 => id::OPEN,
            0x02 => id::OBSTACLE,
            0x04 => field_id::MARKER_SIDE0,
            0x07 => field_id::RALLY_WEST,
            0x08 => field_id::RALLY_EAST,
            0x09 => id::WATER,
            0x0A => id::WOODLAND,
            0x0F => field_id::MARKER_SIDE4,
            0x10 => id::BRIDGE_NEAR,
            0x12 => id::BRIDGE_SPAN,
            0x14 => id::BRIDGE_FAR,
            0x15 => id::LINE,
            0x20..=0x3F => {
                cell.gfx = b;
                cell.flags |= flag::IMPASSABLE;
                id::ROCKS
            }
            0x50..=0x5F => {
                cell.gfx = b.wrapping_add(0x2C);
                cell.flags |= flag::IMPASSABLE;
                id::UNUSED6
            }
            other => other,
        };
    }

    // Pass 2 - the markers. A marker is the pair (x, y) and (x, y+1) holding
    // the same id; the cell at (x+2, y) is `0x40 + slot`, clamped to 0 ..= 11,
    // and the slot's position is (x + 2, y). A pair whose south cell is a
    // rally half instead is one of four rally arrays we do not carry
    // (`0x00544050`, `0x00544070`, `0x00522D30`, `0x00522D60` — nothing in
    // this crate reads a rally waypoint yet); its block is still erased.
    let mut deploy_side0 = [(0u8, 0u8); 12];
    let mut deploy_side4 = [(0u8, 0u8); 12];
    let mut home_side0 = (0u8, 0u8);
    let mut home_side4 = (0u8, 0u8);
    for y in 0..DIM {
        for x in 0..DIM {
            let i = y * DIM + x;
            let north = cells[i].terrain;
            let (slots, home) = match north {
                field_id::MARKER_SIDE0 => (&mut deploy_side0, &mut home_side0),
                field_id::MARKER_SIDE4 => (&mut deploy_side4, &mut home_side4),
                _ => continue,
            };
            let south = if y + 1 < DIM { cells[i + DIM].terrain } else { 0 };
            if south == north {
                let slot = (i32::from(cells.get(i + 2).map_or(0, |c| c.terrain)) - 0x40)
                    .clamp(0, 11) as usize;
                slots[slot] = ((x + 2).min(DIM - 1) as u8, y as u8);
                *home = (x as u8, y as u8);
                erase(&mut cells, x, y, 6, 2);
            } else if south == field_id::RALLY_WEST || south == field_id::RALLY_EAST {
                erase(&mut cells, x, y, 4, 2);
            }
        }
    }

    let terrain: Vec<u8> = cells.iter().map(|c| c.terrain).collect();
    graphics_pass(&mut cells, &terrain, seed, false);
    bridge_pass(&mut cells);

    Battlefield { cells, deploy_side0, deploy_side4, home_side0, home_side4 }
}

/// `FUN_0047D79C(1, cols, rows)` — write open ground over the marker's block,
/// so the slot number and the marker itself leave nothing behind.
fn erase(cells: &mut [Cell], x: usize, y: usize, cols: usize, rows: usize) {
    for r in 0..rows {
        for c in 0..cols {
            if x + c < DIM && y + r < DIM {
                cells[(y + r) * DIM + x + c].terrain = id::OPEN;
            }
        }
    }
}

#[cfg(test)]
#[path = "field_tests.rs"]
mod tests;
