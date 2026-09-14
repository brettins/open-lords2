#![allow(unused_imports)]
use super::*;
use super::tables_part::*;
use crate::siege::{
    code, FLAG_DRAWBRIDGE, FLAG_KEEP, FLAG_WALL, STRUCTURE_STONE, STRUCTURE_WOOD, SURFACE_BAILEY,
    SURFACE_DRAWBRIDGE, SURFACE_FIELD, SURFACE_GROUND, SURFACE_KEEP, SURFACE_RAMPART_WALK,
    SURFACE_WALL, SURFACE_WATER,
};
use crate::terrain::{flag, id, tileset, Battlefield, Cell, Lfsr, CELLS, DIM};
use crate::AiField;

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
/// arms
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

