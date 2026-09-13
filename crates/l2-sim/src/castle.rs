//! **The castle the player actually built, on the battlefield** —
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
//!   lanes and the castle's reference cell — every one of which
//!   [`crate::AiField`] had marked `[I]` because this function was unread.
//!
//! Nothing here reads a file. The caller hands over the bytes.

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
    /// the directory entry, exactly as `Battlefield_BuildCastle` indexes it, so
    /// this stops at the first entry whose two offsets do not both hold a whole
    /// layer rather than trusting a frame count.
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

/// The escape bytes of the frame layer. Everything else indexes the structure
/// table. **[V]** — `if (bVar1 < 0xf0 && 0xec < bVar1)`.
pub const ESCAPE_KEEP_FRAME: u8 = 0xED;
pub const ESCAPE_MOAT: u8 = 0xEE;
pub const ESCAPE_GROUND: u8 = 0xEF;

/// **Build the battlefield from one castle's frame layer** —
/// `Battlefield_BuildCastle` (`0x0047C4BA`), from its first cell loop to
/// `Battlefield_ClassifySurfaces`.
///
/// ```c
/// pass 1:  terrain = (b == 0xEE) ? 0x0B : 1;
/// pass 2:  if (0xEC < b && b < 0xF0) { Rand_Advance(); escape(b); }
///          else {
///            frame = b;  flags2 &= 0xE3;
///            elevation = table[b*2];  if (table[b*2+1] == 0) flags |= 0x10;
///            switch (elevation) { 5..12: the structure ladder }
///          }
/// ```
///
/// The ladder is [`code`], and it *overwrites* `flags` for six of its eight
/// arms rather than setting a bit — so a structure cell is never impassable
/// through `0x10` even where the table says the tile is. **[V]**
///
/// `level` picks the table the same way `DAT_0057C910` does: `1 < level` is
/// stone, and 0 or 1 wooden.
pub fn build(level: u8, sheet: &CastleSheet) -> Battlefield {
    assert_eq!(sheet.frames.len(), LAYER_BYTES, "a castle layer is exactly 80 x 80 bytes");
    let stone = level > 1;
    let table: &[u8; 512] = if stone { &STRUCTURE_STONE } else { &STRUCTURE_WOOD };
    let raster = &sheet.frames;

    let mut cells = vec![Cell::default(); CELLS];
    // Pass 1. `0xEE` is the only byte that is water, and it is water before
    // the auto-tiler is asked what it looks like.
    for (i, c) in cells.iter_mut().enumerate() {
        c.terrain = if raster[i] == ESCAPE_MOAT { id::WATER } else { id::OPEN };
    }
    // `Battlefield_PlaceMoatCell` runs the 49-entry water table over the whole plane; ours
    // does it in one sweep because the rotating counters make the walk order
    // part of the answer and row-major is the builder's.
    let terrain: Vec<u8> = cells.iter().map(|c| c.terrain).collect();
    let moat = crate::terrain::moat_autotile(&terrain);

    // `FUN_00404B2C` is stepped once per escape cell, before the escape runs.
    // The seed the original starts a battle with is untraced; the same note
    // `terrain::build` carries.
    let mut rng = Lfsr::new(0x5EED);
    let mut objective = [0usize; 2];
    let mut walls_seen = 0u32;

    for i in 0..CELLS {
        let b = raster[i];
        if (ESCAPE_KEEP_FRAME..=ESCAPE_GROUND).contains(&b) {
            let r = rng.next();
            let c = &mut cells[i];
            c.flags2 &= !tileset::MASK;
            match b {
                // `FUN_0047DC9C`: the frame is the escape byte itself and the
                // cell stays on slot 0 — the only escape that does.
                ESCAPE_KEEP_FRAME => c.gfx = ESCAPE_KEEP_FRAME,
                // `Battlefield_PlaceMoatCell`: the moat. It also zeroes the approach score,
                // which is `siege::approach_score_at_build`'s business.
                ESCAPE_MOAT => {
                    c.gfx = moat[i];
                    c.flags2 |= tileset::SECOND;
                    c.flags |= flag::IMPASSABLE;
                    c.surface = SURFACE_WATER;
                }
                // `FUN_0047E1DC`: open ground, sixteen variants, slot 1.
                _ => {
                    c.gfx = r & 0x0F;
                    c.flags2 |= tileset::SECOND;
                }
            }
            continue;
        }
        let c = &mut cells[i];
        c.gfx = b;
        c.flags2 &= !tileset::MASK;
        let e = table[b as usize * 2];
        c.elevation = e;
        if table[b as usize * 2 + 1] == 0 {
            c.flags |= flag::IMPASSABLE;
        }
        match e {
            code::RAISED_3 => {
                c.flags = 4;
                c.elevation = 3;
            }
            // **The way in.** `flags2 |= 0x80` as well, and the elevation is
            // the one place the two castle families disagree: a stone keep's
            // door stands at 4 and a wooden one's at 1.
            code::KEEP => {
                c.surface = SURFACE_KEEP;
                c.flags = FLAG_KEEP;
                c.elevation = if stone { 4 } else { 1 };
                c.flags2 |= 0x80;
                if objective[0] == 0 {
                    // Both arms of the original's `ownerIsHuman` test write the
                    // same thing: one row north of the door.
                    objective[0] = i.saturating_sub(DIM);
                }
            }
            code::SEVEN => {
                c.surface = 7;
                c.elevation = 2;
            }
            // The curtain block: surface 8 is what `BattleMan_StateAttackWall`
            // keeps swinging at, and `0x20` is what `Cell_TryEnter` returns 5
            // for.
            code::WALL => {
                c.surface = SURFACE_WALL;
                c.flags = FLAG_WALL | 4;
                c.elevation = 1;
                if objective[1] == 0 {
                    objective[1] = i.saturating_sub(DIM - 1);
                }
                walls_seen += 1;
                // The fifth wall cell seeds `DAT_0053E9D4`, which nothing in
                // the binary reads. Counted, not kept.
                let _ = walls_seen;
            }
            code::DRAWBRIDGE => {
                c.surface = SURFACE_DRAWBRIDGE;
                c.flags = FLAG_DRAWBRIDGE;
                c.elevation = 0;
            }
            code::TEN => {
                c.surface = 0x0E;
                c.flags |= 4;
                c.elevation = 0;
            }
            code::RAISED_1 => {
                c.flags = 4;
                c.elevation = 1;
            }
            code::RAISED_2 => {
                c.flags = 4;
                c.elevation = 2;
            }
            _ => {}
        }
    }

    classify(&mut cells);

    let mut field = Battlefield {
        cells,
        deploy_side0: [(0, 0); 12],
        deploy_side4: [(0, 0); 12],
        home_side0: (0, 0),
        home_side4: (0, 0),
    };
    let t = tables(sheet);
    let clamp = |(x, y): (i16, i16)| {
        (x.clamp(0, DIM as i16 - 1) as u8, y.clamp(0, DIM as i16 - 1) as u8)
    };
    for slot in 0..12 {
        field.deploy_side0[slot] = clamp(t.deploy_side0[slot]);
        field.deploy_side4[slot] = clamp(t.deploy_side4[slot]);
    }
    field.home_side0 = field.deploy_side0[0];
    field.home_side4 = field.deploy_side4[0];
    let _ = objective;
    field
}

/// **The surface classifier** — `Battlefield_ClassifySurfaces` (`0x0047E230`), six flood
/// passes over the whole field, each looping until it changes nothing.
///
/// The builder writes a surface for five structure codes and leaves the rest at
/// zero; these passes give every other cell one. In order:
///
/// | pass | what it spreads |
/// |---|---|
/// | `Battlefield_ClassifyKeep` | the keep (6) outward over surface `0x0E` and over anything above elevation 3 |
/// | `Battlefield_ClassifyBailey` | the bailey (5) over low ground beside the keep door, a 7 or another 5 |
/// | `Battlefield_ClassifyRampartWalk` | the rampart walk (4) over raised ground beside the bailey or another walk |
/// | `Battlefield_ClassifyOutsideGround` | the apron (3) over flat ground within **eight** neighbours of a walk or another apron |
/// | `Battlefield_ClassifyField` | the open field (1), seeded at cells `0` and `0x18B0` — `(0, 0)` and `(0, 79)` |
/// | `Battlefield_ClassifyLeftovers` | what is left: 3 beside water, 6 beside the keep |
///
/// Surface **4** is the one an order handler searches for by value
/// (`Siege_FindCellSurface4`), and surface **5** is what `Wall_Collapse` bills
/// a breach for — so which cells end up 4 and which 5 is not cosmetic. **[V]**
fn classify(cells: &mut [Cell]) {
    let at = |x: usize, y: usize| y * DIM + x;
    // `Cell_NeighbourHasSurface` (`0x00496F72`): the four orthogonals, clipped.
    let orth = |cells: &[Cell], x: usize, y: usize, s: u8| {
        (y >= 1 && cells[at(x, y - 1)].surface == s)
            || (x < 0x4F && cells[at(x + 1, y)].surface == s)
            || (y < 0x4F && cells[at(x, y + 1)].surface == s)
            || (x >= 1 && cells[at(x - 1, y)].surface == s)
    };
    // `FUN_00497024`: the same, plus the four diagonals.
    let eight = |cells: &[Cell], x: usize, y: usize, s: u8| {
        orth(cells, x, y, s)
            || [(1i32, -1i32), (1, 1), (-1, 1), (-1, -1)].iter().any(|&(dx, dy)| {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                (0..DIM as i32).contains(&nx)
                    && (0..DIM as i32).contains(&ny)
                    && cells[at(nx as usize, ny as usize)].surface == s
            })
    };
    // `FUN_0049719E`: **all four** orthogonals are water, off-map counting as
    // water. A cell the moat has closed round.
    let ringed_by_water = |cells: &[Cell], x: usize, y: usize| {
        (y < 1 || cells[at(x, y - 1)].surface == SURFACE_WATER)
            && (x >= 0x4F || cells[at(x + 1, y)].surface == SURFACE_WATER)
            && (y >= 0x4F || cells[at(x, y + 1)].surface == SURFACE_WATER)
            && (x < 1 || cells[at(x - 1, y)].surface == SURFACE_WATER)
    };
    // `FUN_00497247`: any orthogonal neighbour carries the keep-door flag.
    let beside_keep_door = |cells: &[Cell], x: usize, y: usize| {
        (y >= 1 && cells[at(x, y - 1)].flags & FLAG_KEEP != 0)
            || (x < 0x4F && cells[at(x + 1, y)].flags & FLAG_KEEP != 0)
            || (y < 0x4F && cells[at(x, y + 1)].flags & FLAG_KEEP != 0)
            || (x >= 1 && cells[at(x - 1, y)].flags & FLAG_KEEP != 0)
    };
    let sweep = |cells: &mut [Cell], f: &dyn Fn(&mut [Cell], usize, usize, usize) -> bool| {
        let mut again = true;
        while again {
            again = false;
            for y in 0..DIM {
                for x in 0..DIM {
                    if f(cells, x, y, at(x, y)) {
                        again = true;
                    }
                }
            }
        }
    };

    sweep(cells, &|c, x, y, i| {
        let s = c[i].surface;
        if (s == 0x0E || (s == 0 && c[i].elevation > 3)) && orth(c, x, y, SURFACE_KEEP) {
            c[i].surface = SURFACE_KEEP;
            if s == 0x0E {
                c[i].elevation = 4;
            }
            return true;
        }
        false
    });
    sweep(cells, &|c, x, y, i| {
        let s = c[i].surface;
        if s == 0x0E {
            c[i].surface = SURFACE_BAILEY;
            return true;
        }
        if s == 0
            && c[i].elevation < 2
            && (beside_keep_door(c, x, y) || orth(c, x, y, 7) || orth(c, x, y, SURFACE_BAILEY))
        {
            c[i].surface = SURFACE_BAILEY;
            return true;
        }
        false
    });
    sweep(cells, &|c, x, y, i| {
        if c[i].surface == 0
            && c[i].elevation != 0
            && (orth(c, x, y, SURFACE_BAILEY) || orth(c, x, y, SURFACE_RAMPART_WALK))
        {
            c[i].surface = SURFACE_RAMPART_WALK;
            return true;
        }
        false
    });
    sweep(cells, &|c, x, y, i| {
        if (c[i].surface == 0x0E || (c[i].surface == 0 && c[i].elevation == 0))
            && (eight(c, x, y, SURFACE_RAMPART_WALK) || eight(c, x, y, SURFACE_GROUND))
        {
            c[i].surface = SURFACE_GROUND;
            return true;
        }
        false
    });
    // The two seeds, written before the loop and not inside it.
    cells[0].surface = SURFACE_FIELD;
    cells[0x18B0].surface = SURFACE_FIELD;
    sweep(cells, &|c, x, y, i| {
        let s = c[i].surface;
        if s == SURFACE_FIELD || s == SURFACE_WATER || s >= 4 || c[i].elevation != 0 {
            return false;
        }
        let mut did = false;
        if eight(c, x, y, SURFACE_FIELD) {
            c[i].surface = SURFACE_FIELD;
            did = true;
        }
        if s != SURFACE_GROUND && ringed_by_water(c, x, y) {
            c[i].surface = SURFACE_FIELD;
            did = true;
        }
        did
    });
    sweep(cells, &|c, x, y, i| {
        if c[i].surface != 0 {
            return false;
        }
        if orth(c, x, y, SURFACE_WATER) {
            c[i].surface = SURFACE_GROUND;
            return true;
        }
        if orth(c, x, y, SURFACE_KEEP) {
            c[i].surface = SURFACE_KEEP;
            return true;
        }
        false
    });
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

/// The marker bytes the walker anchors on: `0x04` for side 0's tables and
/// `0x0F` for side 4's, which are the same two ids a `.skr` field battle uses
/// for its deployment markers ([`id::MARKER_SIDE0`], [`id::MARKER_SIDE4`] are
/// the terrain-layer spellings of the same idea).
pub const MARKER_SIDE0: u8 = 0x04;
pub const MARKER_SIDE4: u8 = 0x0F;

/// **Walk the structure layer** — `Battlefield_ReadStructureLayer`.
///
/// A marker is a 2 x 2 block. Byte `+0` says which side's tables it feeds,
/// byte `+1` is the **kind**, and the index is written underneath it:
///
/// * kind equal to the marker — a deployment slot, index `buf[+2] - 0x40`
///   clamped to `0..=11` (`Marker_DeploySlot`);
/// * side 0's other kinds — a four-bit index, `buf[+0x50 ..= +0x53]` each
///   contributing `8, 4, 2, 1` when it holds **7** (`Marker_Index4Bit`);
/// * side 4's other kinds — a two-bit index from `buf[+0x50]`, `buf[+0x51]`
///   (`Marker_Index2Bit`).
///
/// The walker zeroes each byte as it consumes it, which is why it is run over a
/// copy here: a `7` belonging to one marker must not be read again by the next.
/// Points are stored `(x + 2, y + 2)`, except a deployment slot's `(x + 2, y)`
/// and the reference cell's `(x, y - 2)` — the block's own corner offsets.
/// **[V]**
///
/// The four filled deployment slots are the check that this decode is right:
/// `Deploy_SlotForUnitSiege` (`0x004816F9`) reaches only slots 0, 1, 4 and 8,
/// and slots 0, 1, 4 and 8 are exactly the ones every shipped layout fills.
pub fn tables(sheet: &CastleSheet) -> CastleTables {
    assert_eq!(sheet.structures.len(), LAYER_BYTES, "a castle layer is exactly 80 x 80 bytes");
    let mut b = sheet.structures.clone();
    let mut t = CastleTables {
        deploy_side0: [(0, 0); 12],
        deploy_side4: [(0, 0); 12],
        wall_slot: [[(0, 0); 16]; 4],
        approach: [[(0, 0); 4]; 6],
    };
    // `Marker_Index4Bit` and `Marker_Index2Bit`: the row below the marker, read as bits
    // and then cleared.
    let index_of = |b: &mut [u8], i: usize, bits: usize| -> usize {
        let mut v = 0usize;
        for k in 0..bits {
            if b.get(i + 0x50 + k) == Some(&7) {
                v += 1 << (bits - 1 - k);
            }
        }
        for k in 0..4 {
            if let Some(x) = b.get_mut(i + 0x50 + k) {
                *x = 0;
            }
        }
        v
    };
    for y in 0..DIM {
        for x in 0..DIM {
            let i = y * DIM + x;
            let marker = b[i];
            if marker != MARKER_SIDE0 && marker != MARKER_SIDE4 {
                continue;
            }
            // `Marker_TakeKind`: take the kind and clear it, and `+3` with it.
            let kind = b[i + 1];
            b[i + 1] = 0;
            if let Some(v) = b.get_mut(i + 3) {
                *v = 0;
            }
            let (px, py) = (x as i16 + 2, y as i16 + 2);
            if marker == MARKER_SIDE0 {
                if kind == MARKER_SIDE0 {
                    // `Marker_DeploySlot`, and the y is the marker's own row.
                    let slot = (b[i + 2] as i16 - 0x40).clamp(0, 11) as usize;
                    b[i + 0x50] = 0;
                    b[i + 0x51] = 0;
                    t.deploy_side0[slot] = (px, y as i16);
                } else if let Some(g) = group_of(kind) {
                    let k = index_of(&mut b, i, 4);
                    t.wall_slot[g][k] = (px, py);
                }
            } else if kind == MARKER_SIDE4 {
                let slot = (b[i + 2] as i16 - 0x40).clamp(0, 11) as usize;
                b[i + 0x50] = 0;
                b[i + 0x51] = 0;
                t.deploy_side4[slot] = (px, y as i16);
            } else if kind == 0x43 {
                // `DAT_0055CDD0` … `DAT_0055CDEC`: one point, written into all
                // four lanes of row 2.
                t.approach[2] = [(x as i16, y as i16 - 2); 4];
            } else if let Some(row) = approach_row(kind) {
                let k = index_of(&mut b, i, 2);
                t.approach[row][k] = (px, py);
            }
        }
    }
    t
}

/// Side 0's four wall-slot groups, by their base address: `0x00554180`,
/// `0x00554200`, `0x00554280`, `0x00554380` — `0x80` apart, so kind `0x47`
/// lands in group **4** with group 3 never written.
fn group_of(kind: u8) -> Option<usize> {
    match kind {
        0x40 => Some(0),
        0x41 => Some(1),
        0x44 => Some(2),
        0x47 => Some(3),
        _ => None,
    }
}

/// Side 4's approach rows, `0x20` apart from `0x0055CD90`.
fn approach_row(kind: u8) -> Option<usize> {
    match kind {
        0x40 => Some(0),
        0x41 => Some(1),
        0x47 => Some(3),
        0x44 => Some(5),
        _ => None,
    }
}

/// **The AI's view of a real castle**, out of [`tables`] rather than out of a
/// ring we drew.
///
/// [`crate::siege::our_castle_ai_field`] is the same shape filled from our
/// stand-in's geometry; every field it marks `[I]` is a `[V]` here, because
/// this is the table `Battlefield_ReadStructureLayer` writes.
///
/// Two of our fields alias one of the original's: `castle_approach[3]` and
/// [`crate::AiField::staging`] are both `0x0055CDF0`, and `castle_approach[2]`
/// and [`crate::AiField::castle_ref`] are both `0x0055CDD0`. They are filled
/// consistently rather than allowed to disagree.
pub fn ai_field(field: &Battlefield, level: u8, t: &CastleTables) -> AiField {
    let mut f = crate::runner::ai_field_for(field);
    f.castle_approach = t.approach;
    f.castle_ref = t.approach[2][0];
    f.staging = t.approach[3];
    f.wall_slot = [t.wall_slot[0], t.wall_slot[1], t.wall_slot[2]];
    f.castle_index = 13;
    f.layout = 1;
    // `DAT_00542CD4`, the flag two handlers jump their orders to 100 on:
    // `Battlefield_BuildCastle` writes `g_castleLevel == 1` in the campaign.
    f.castle_layout_flag = level == 1;
    // `DAT_00553274` — one row north of the keep door — and `DAT_00553EE4`,
    // one row north and one east of the first curtain block.
    let first = |pred: &dyn Fn(&Cell) -> bool, back: usize| {
        field.cells.iter().position(pred).map(|i| i.saturating_sub(back)).unwrap_or(0)
    };
    f.castle_objective = [
        first(&|c| c.flags & FLAG_KEEP != 0, DIM),
        first(&|c| c.flags & FLAG_WALL != 0, DIM - 1),
    ];
    // The posts are the holes, and they arrive when the holes do — see
    // `siege::our_castle_ai_field`.
    f.defence_posts = [0; 20];
    f
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

    /// `parse` stops where the file does rather than trusting a count.
    #[test]
    fn a_truncated_layout_file_yields_no_castles() {
        assert!(CastleSheets::parse(&[0u8; 40]).is_none());
        assert!(CastleSheets::parse(&[]).is_none());
    }
}
