//! **The castle the player
//! `Battlefield_BuildCastle` (`0x0047C4BA`), `Battlefield_ReadStructureLayer` and the six
//! surface passes of `Battlefield_ClassifySurfaces`.
//!
//! Until this module existed the besieged castle was
//! [`crate::siege::our_castle`]: a square ring whose wall stood one cell high.
//! Two mechanics that are built and tested could therefore never fire in a
//! real siege, because both want a wall **two** high — boiling oil
//! (`UnitOrder_SiegeDefOil`, elevation `>= 2`) and a siege tower's dock
//! (`FUN_00491492`, elevation `== 2` exactly, [`crate::siege::DOCK_WALL_ELEVATION`]).
//! They fired only in `crate::proving`, which builds its own scenery.
//!
//! # Where the castle is
//!
//! `stnfield.pl8` — `s_Q_Q_Qbatfield_pl8` at `0x004D9103`, a 14-byte-stride
//! string table whose name field starts at `+5`, indexed **1** for a campaign
//! castle and **3** for a skirmish one (`DAT_0057A0F0`). The builder reads the
//! first 1000 bytes as a directory and takes two 24-bit little-endian offsets
//! out of entry `castle * 0x20`:
//!
//! ```c
//! File_ReadChunk(name, buf, 1000, 0);
//! off = buf[c*0x20+0x0C] | buf[c*0x20+0x0D]<<8 | buf[c*0x20+0x0E]<<16;  /* frames     */
//! File_ReadChunk(name, buf, 0x1900, off);
//! ...  buf[c*0x20+0x1C .. +0x1E]                                        /* structures */
//! ```
//!
//! **[V]**, and the directory is a PL8's own: entry `c` spans two 16-byte PL8
//! frame records, so the two offsets are frames `2c` and `2c + 1`, each
//! 80 x 80 and uncompressed. The shipped `Stnfield.pl8` is 64,168 bytes —
//! 168 of header and directory plus exactly ten 6,400-byte layers.
//!
//! # What the two layers are
//!
//! * the **frame** layer is `cell[+3]` directly, read through the 256-entry
//!   structure table ([`crate::siege::STRUCTURE_STONE`] / `_WOOD`) for its
//!   height, its passability and its structure code;
//! * the **structure** layer is markers, not tiles: `Battlefield_ReadStructureLayer` walks it for
//!   the twelve deployment slots a side, the wall-slot groups, the approach
//! lanes and the castle's reference cell — every one of which
//!   [`crate::AiField`] had marked `[I]` because this function was unread.
//!
//! Nothing here reads a file. The caller hands over the bytes.

mod builder;
pub use builder::*;
mod tables_part;
pub use tables_part::*;

use crate::siege::{
    code, FLAG_DRAWBRIDGE, FLAG_KEEP, FLAG_WALL, STRUCTURE_STONE, STRUCTURE_WOOD, SURFACE_BAILEY,
    SURFACE_DRAWBRIDGE, SURFACE_FIELD, SURFACE_GROUND, SURFACE_KEEP, SURFACE_RAMPART_WALK,
    SURFACE_WALL, SURFACE_WATER,
};
use crate::terrain::{flag, id, tileset, Battlefield, Cell, Lfsr, CELLS, DIM};
use crate::AiField;

/// One layer is one 80 x 80 raster — the `0x1900` of both `File_ReadChunk`
/// calls.
pub const LAYER_BYTES: usize = CELLS;

/// The two layers of one castle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastleSheet {
    /// Directory `+0x0C` — cell byte `+3`, with `0xED`/`0xEE`/`0xEF` as escapes.
    pub frames: Vec<u8>,
    /// Directory `+0x1C` — the marker layer `Battlefield_ReadStructureLayer` walks.
    pub structures: Vec<u8>,
}

/// Every castle in one of the two layout files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastleSheets {
    sheets: Vec<CastleSheet>,
}

impl CastleSheets {
    /// The campaign file — `s_stnfield_pl8` (`0x004D9EF8`, `0x004D9F18`), index
    /// 1 of the builder's string table.
    pub const FILE: &'static str = "stnfield.pl8";
    /// The skirmish file, index 3 — `DAT_0057A0F0` picks it. Not loaded here:
    /// nothing in this engine fights a skirmish yet.
    pub const SKIRMISH_FILE: &'static str = "stnfiel2.pl8";

    /// The five campaign castles, in `g_castleLevel` order. `castle * 0x20` is
    /// the directory entry,
    /// this stops at the first entry whose two offsets do not both hold a whole
    /// layer
    pub fn parse(bytes: &[u8]) -> Option<CastleSheets> {
        let dir = bytes.get(..1000.min(bytes.len()))?;
        let u24 = |at: usize| -> Option<usize> {
            let b = dir.get(at..at + 3)?;
            Some(b[0] as usize | (b[1] as usize) << 8 | (b[2] as usize) << 16)
        };
        let layer = |off: usize| -> Option<Vec<u8>> {
            bytes.get(off..off + LAYER_BYTES).map(<[u8]>::to_vec)
        };
        let mut sheets = Vec::new();
        for castle in 0.. {
            let entry = castle * 0x20;
            let (Some(f), Some(s)) = (u24(entry + 0x0C), u24(entry + 0x1C)) else { break };
            let (Some(frames), Some(structures)) = (layer(f), layer(s)) else { break };
            sheets.push(CastleSheet { frames, structures });
        }
        (!sheets.is_empty()).then_some(CastleSheets { sheets })
    }

    pub fn get(&self, castle: u8) -> Option<&CastleSheet> {
        self.sheets.get(castle as usize)
    }

    pub fn len(&self) -> usize {
        self.sheets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sheets.is_empty()
    }
}

/// **The positional tables the structure layer carries** — `Battlefield_ReadStructureLayer`
/// (`0x0047CEC1`), every one of which [`crate::AiField`] had as `[I]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastleTables {
    /// `0x00553150` — `Deploy_SlotForUnit`'s twelve slots for side 0, the
    /// garrison. `Deploy_SlotForUnitSiege` maps a troop type to slot 0, 1, 4 or
    /// 8 of this table, and those are exactly the four the shipped layouts
    /// fill.
    pub deploy_side0: [(i16, i16); 12],
    /// `0x00531B0` — the same twelve for side 4, the besieger. All twelve are
    /// filled, in two rows across the foot of the field.
    pub deploy_side4: [(i16, i16); 12],
    /// `0x00554180`, `0x00554200`, `0x00554280`, `0x00554380` — four groups of
    /// sixteen wall slots, from marker `0x04` under kinds `0x40`, `0x41`,
    /// `0x44` and `0x47`. [`crate::AiField::wall_slot`] models the first three.
    pub wall_slot: [[(i16, i16); 16]; 4],
    /// `0x0055CD90 + row * 0x20` — six rows of four lanes, from marker `0x0F`.
    /// Row 2 is the castle's reference cell written into all four lanes
    /// (kind `0x43`), row 3 is the staging table `Battlefield_BuildCastle`
    /// itself reads back, and row 4 is never written by any shipped layout.
    pub approach: [[(i16, i16); 4]; 6],
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A layer of nothing but open ground is a field with no castle in it, and
    /// the classifier still has to give every cell a surface.
    #[test]
    fn an_empty_layer_classifies_to_open_field() {
        let sheet = CastleSheet {
            frames: vec![ESCAPE_GROUND; LAYER_BYTES],
            structures: vec![0; LAYER_BYTES],
        };
        let field = build(2, &sheet);
        assert!(field.cells.iter().all(|c| c.surface == SURFACE_FIELD));
        assert!(field.cells.iter().all(|c| c.elevation == 0));
        // Every cell came off slot 1 — `t32_stn2`, the ground sheet.
        assert!(field.cells.iter().all(|c| c.tileset() == 1));
    }

    /// `parse` stops where the file does
    #[test]
    fn a_truncated_layout_file_yields_no_castles() {
        assert!(CastleSheets::parse(&[0u8; 40]).is_none());
        assert!(CastleSheets::parse(&[]).is_none());
    }
}

