#![allow(unused_imports)]
use super::*;
use super::damage::*;
use super::drawbridge::*;
use super::moat::*;
use super::siegetower::*;
use crate::terrain::{Battlefield, Cell, DIM};

pub fn approach_score_at_build(field: &Battlefield) -> i32 {
    if field.cells.iter().any(|c| c.surface == SURFACE_WATER) {
        0
    } else {
        APPROACH_SCORE_START
    }
}


pub fn our_castle(level: u8) -> Battlefield {
    use crate::terrain::{flag, id};

    let level = level.min(4);
    let half = 6 + level as i32 * 2;
    let (cx, cy) = (40i32, 24i32);

    let mut cells = vec![
        Cell {
            terrain: id::OPEN,
            flags: 0,
            flags2: 0,
            gfx: 0,
            elevation: 0,
            surface: SURFACE_FIELD
        };
        DIM * DIM
    ];
    let at = |x: i32, y: i32| (y as usize) * DIM + (x as usize);
    let inside = |x: i32, y: i32| (x - cx).abs() <= half && (y - cy).abs() <= half;
    let on_ring = |x: i32, y: i32| {
        inside(x, y) && ((x - cx).abs() == half || (y - cy).abs() == half)
    };

    if level >= 2 {
        for y in (cy - half - 1)..=(cy + half + 1) {
            for x in (cx - half - 1)..=(cx + half + 1) {
                if !(0..DIM as i32).contains(&x) || !(0..DIM as i32).contains(&y) {
                    continue;
                }
                let ring = (x - cx).abs() == half + 1 || (y - cy).abs() == half + 1;
                if ring {
                    let c = &mut cells[at(x, y)];
                    c.terrain = id::WATER;
                    c.surface = SURFACE_WATER;
                    c.flags |= flag::IMPASSABLE;
                }
            }
        }
    }

    let apron = half + if level >= 2 { 2 } else { 1 };
    for y in (cy - apron)..=(cy + apron) {
        for x in (cx - apron)..=(cx + apron) {
            if !(0..DIM as i32).contains(&x) || !(0..DIM as i32).contains(&y) {
                continue;
            }
            if (x - cx).abs() == apron || (y - cy).abs() == apron {
                cells[at(x, y)].surface = SURFACE_GROUND;
            }
        }
    }

    for y in (cy - half)..=(cy + half) {
        for x in (cx - half)..=(cx + half) {
            if !(0..DIM as i32).contains(&x) || !(0..DIM as i32).contains(&y) {
                continue;
            }
            let on_walk =
                (x - cx).abs() == half - 2 || (y - cy).abs() == half - 2;
            let c = &mut cells[at(x, y)];
            if on_ring(x, y) {
                c.surface = SURFACE_WALL;
                c.elevation = WALL_ELEVATION;
                c.flags |= FLAG_WALL;
            } else if on_walk {
                c.surface = SURFACE_RAMPART_WALK;
                c.elevation = WALL_ELEVATION;
            } else {
                c.surface = SURFACE_BAILEY;
                c.elevation = 0;
            }
        }
    }

    if level >= 3 {
        let gate_x = cx - DRAWBRIDGE_COLS as i32 / 2;
        for dy in 0..3i32 {
            for dx in 0..DRAWBRIDGE_COLS as i32 {
                let (x, y) = (gate_x + dx, cy + half + dy);
                if !(0..DIM as i32).contains(&x) || !(0..DIM as i32).contains(&y) {
                    continue;
                }
                let c = &mut cells[at(x, y)];
                c.flags = FLAG_DRAWBRIDGE;
                c.terrain = id::OPEN;
                c.surface = SURFACE_DRAWBRIDGE;
                c.elevation = 0;
            }
        }
    }

    // > It used to stand at 3 against a bailey of 1, and that made the third
    // > way a siege can end unreachable. `Formation_SlotIsUsable` rejects a
    // > slot more than one below the destination's elevation and
    // > `Formation_RectIsClear` rejects a rectangle whose slots are not *at*
    // > it, so an order onto a keep two levels above its own courtyard is an
    // > order no figure is ever given — the besiegers walk into the bailey,
    // > find nowhere to stand, and the battle runs for ever with the gate open
    // > and two men of the garrison alive in a corner. Found by fighting one to
    // > the end, which nothing had done. The elevation was ours to begin with;
    // > `docs/decisions.md` `C82`.
    cells[at(cx, cy)].flags |= FLAG_KEEP;
    cells[at(cx, cy)].surface = SURFACE_KEEP;
    // `Battlefield_BuildCastle`'s code-6 arm sets `flags2 |= 0x80` as well, and
    // that bit is the whole of the banner's gate — `BattleBanner_Draw`
    // (`0x004BD574`) by way of `FUN_004BD355`. Without it the stand-in castle
    // is the one castle in the game that flies no flag.
    cells[at(cx, cy)].flags2 |= 0x80;

    paint_our_castle(&mut cells, level);

    let mut field = Battlefield {
        cells,
        deploy_side0: [(0, 0); 12],
        deploy_side4: [(0, 0); 12],
        home_side0: (cx as u8, (cy + half / 2) as u8),
        home_side4: (cx as u8, 64),
    };
    for (slot, (dx, dy)) in crate::terrain::DEPLOY_OFFSETS.iter().enumerate() {
        let clamp = |v: i32| v.clamp(1, DIM as i32 - 2) as u8;
        let sx = (dx / 3).clamp(-half + 2, half - 2);
        let sy = (dy / 3).clamp(-half + 2, half - 2);
        field.deploy_side0[slot] =
            (clamp(field.home_side0.0 as i32 + sx), clamp(field.home_side0.1 as i32 + sy));
        field.deploy_side4[slot] =
            (clamp(field.home_side4.0 as i32 + dx), clamp(field.home_side4.1 as i32 + dy));
    }
    field
}


/// **The structure table `Battlefield_BuildCastle` reads every raster byte
/// through** — `0x004D7B80` for a stone castle and `0x004D7D80` for a wooden
/// one, 256 entries of two bytes, indexed by the frame index itself:
///
/// **[V]**, read out of `Lords2.exe` at file offsets `0xD5D80` and `0xD5F80`;
/// `crates/l2-game/tests/siege_picture/main.rs` re-reads the player's own copy and
/// fails if these bytes drift.
///
/// **Which castle gets which is `DAT_0057C910`**, `(uint)(1 < g_castleLevel)`
/// — the same flag that picks `t32_stn1` over `t32_wod1`, so the table and
/// the sheet always agree. **[V]**
pub const STRUCTURE_STONE: [u8; 512] = [
    6, 1, 0, 1, 0, 1, 0, 1, 3, 1, 4, 0, 4, 0, 0, 1, //
    4, 0, 4, 0, 4, 0, 4, 0, 0, 1, 2, 0, 0, 1, 4, 0, //
    5, 1, 5, 1, 0, 1, 0, 1, 3, 1, 4, 0, 0, 1, 4, 0, //
    4, 1, 4, 1, 4, 1, 4, 1, 4, 0, 0, 1, 4, 0, 0, 1, //
    5, 1, 5, 1, 0, 1, 0, 1, 3, 1, 4, 0, 4, 0, 4, 1, //
    4, 1, 4, 1, 4, 1, 4, 1, 4, 1, 4, 0, 0, 1, 0, 1, //
    7, 0, 7, 0, 7, 0, 7, 0, 3, 1, 4, 0, 4, 0, 4, 1, //
    4, 1, 4, 0, 4, 0, 4, 1, 4, 1, 4, 0, 0, 1, 0, 1, //
    7, 0, 7, 0, 7, 0, 7, 0, 0, 1, 0, 1, 4, 0, 4, 1, //
    4, 1, 4, 0, 4, 0, 4, 1, 4, 1, 4, 0, 0, 1, 0, 1, //
    2, 0, 2, 1, 2, 0, 2, 0, 2, 0, 0, 1, 4, 0, 4, 1, //
    4, 1, 4, 1, 4, 1, 4, 1, 4, 1, 4, 0, 0, 1, 0, 1, //
    2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 0, 1, 4, 0, //
    4, 1, 4, 1, 4, 1, 4, 1, 4, 0, 0, 1, 0, 1, 0, 1, //
    2, 0, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 0, 1, 0, 1, //
    4, 0, 4, 0, 4, 0, 4, 0, 0, 1, 0, 1, 0, 1, 0, 1, //
    2, 0, 2, 1, 2, 1, 2, 1, 2, 0, 2, 1, 0, 1, 0, 1, //
    0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, //
    2, 0, 2, 0, 2, 1, 2, 1, 2, 0, 4, 0, 0, 1, 0, 1, //
    0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, //
    2, 1, 2, 1, 2, 1, 0, 1, 9, 1, 9, 1, 9, 1, 9, 1, //
    4, 1, 4, 0, 4, 0, 4, 0, 8, 1, 8, 1, 8, 1, 8, 1, //
    1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 0, 1, 0, 1, 4, 1, //
    4, 1, 4, 1, 4, 1, 4, 1, 0, 1, 0, 1, 0, 1, 0, 1, //
    0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 4, 0, //
    4, 1, 4, 1, 4, 1, 4, 1, 0, 1, 0, 1, 0, 1, 0, 1, //
    0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 4, 0, //
    4, 1, 4, 1, 4, 1, 4, 0, 11, 0, 11, 0, 11, 0, 11, 0, //
    12, 1, 12, 1, 10, 1, 10, 1, 10, 1, 10, 1, 10, 1, 0, 1, //
    4, 0, 4, 1, 4, 1, 4, 0, 0, 1, 0, 1, 0, 1, 0, 1, //
    0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, //
    0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, //
];

/// The wooden castles' half of [`STRUCTURE_STONE`], `0x004D7D80`.
pub const STRUCTURE_WOOD: [u8; 512] = [
    6, 1, 0, 1, 0, 1, 0, 1, 2, 0, 2, 0, 2, 0, 2, 0, //
    2, 0, 2, 0, 0, 1, 0, 1, 0, 1, 7, 0, 7, 0, 7, 0, //
    0, 1, 0, 1, 2, 0, 2, 0, 2, 1, 2, 1, 2, 1, 2, 1, //
    2, 1, 2, 1, 2, 0, 2, 0, 0, 1, 7, 0, 7, 0, 7, 0, //
    0, 1, 2, 0, 2, 0, 2, 1, 2, 1, 1, 1, 1, 1, 1, 1, //
    1, 1, 2, 1, 2, 1, 2, 0, 2, 0, 7, 0, 7, 0, 7, 0, //
    0, 1, 2, 0, 2, 1, 2, 1, 1, 1, 0, 1, 0, 1, 0, 1, //
    0, 1, 1, 1, 2, 1, 2, 1, 2, 0, 0, 1, 0, 1, 0, 1, //
    2, 0, 2, 0, 2, 1, 1, 1, 0, 1, 4, 0, 4, 1, 4, 0, //
    2, 0, 2, 0, 1, 1, 2, 1, 2, 0, 2, 0, 0, 1, 0, 1, //
    2, 0, 2, 1, 1, 1, 0, 1, 3, 1, 4, 0, 4, 1, 4, 0, //
    2, 0, 2, 0, 2, 0, 1, 1, 2, 1, 2, 0, 0, 1, 0, 1, //
    2, 0, 2, 1, 1, 1, 0, 1, 2, 1, 4, 0, 4, 0, 4, 0, //
    2, 1, 2, 0, 2, 0, 1, 1, 2, 1, 2, 0, 0, 1, 0, 1, //
    2, 0, 2, 1, 1, 1, 2, 1, 2, 1, 2, 1, 8, 1, 8, 1, //
    8, 1, 8, 1, 0, 1, 1, 1, 2, 1, 2, 0, 0, 1, 0, 1, //
    2, 0, 2, 1, 1, 1, 2, 1, 2, 1, 2, 1, 0, 1, 0, 1, //
    0, 1, 0, 1, 0, 1, 1, 1, 2, 1, 2, 0, 0, 1, 0, 1, //
    2, 0, 2, 1, 2, 1, 1, 1, 1, 1, 1, 1, 0, 1, 0, 1, //
    0, 1, 0, 1, 1, 1, 2, 1, 2, 1, 2, 0, 0, 1, 0, 1, //
    0, 1, 2, 0, 2, 1, 2, 1, 1, 1, 1, 1, 2, 0, 2, 0, //
    2, 0, 1, 1, 2, 1, 2, 1, 2, 0, 0, 1, 0, 1, 0, 1, //
    0, 1, 2, 0, 2, 0, 2, 1, 2, 1, 1, 1, 1, 1, 1, 1, //
    1, 1, 2, 1, 2, 1, 2, 0, 2, 0, 0, 1, 0, 1, 0, 1, //
    0, 1, 0, 1, 2, 0, 2, 0, 2, 1, 2, 1, 2, 1, 2, 1, //
    2, 1, 2, 1, 2, 0, 2, 0, 0, 1, 0, 1, 0, 1, 0, 1, //
    0, 1, 0, 1, 0, 1, 0, 1, 2, 0, 2, 0, 2, 0, 2, 0, //
    2, 0, 2, 0, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, //
    4, 0, 4, 0, 4, 0, 4, 0, 0, 1, 0, 1, 0, 1, 0, 1, //
    0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, //
    0, 1, 0, 1, 0, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, //
    2, 1, 2, 1, 2, 1, 2, 1, 2, 0, 2, 0, 2, 0, 2, 0, //
];

pub fn frames_with_code(level: u8, want: u8) -> Vec<u8> {
    let table = if level > 1 { &STRUCTURE_STONE } else { &STRUCTURE_WOOD };
    (0..256u16).filter(|&f| table[f as usize * 2] == want).map(|f| f as u8).collect()
}

/// * **The ground and the ditch are the game's.** `Battlefield_BuildCastle`'s
///   escape codes `0xEF` and `0xEE` — `FUN_0047E1DC` and `FUN_0047DCCE` —
/// put open ground on `rand & 0x0F` and the moat on the 49-variant water
///   auto-tiler, both with the cell's tileset selector set to **slot 1**,
///   `t32_stn2` / `t32_wod2`. Reproduced exactly, including the selector.
///
///   **[V]**
/// * **The castle's tiles are the game's; its shape is not.** Which of
///   `t32_stn1`'s 256 frames is a wall, a keep door or a drawbridge plank is
///   [`frames_with_code`], read out of the binary — but *where* those go is
/// [`our_castle`]'s ring, and the original's is a raster in `stnfield.pl8`
///   we have not read. Where a code carries several frames they are cycled by
///   position; which one the original's raster picks is the raster's. `[I]`
///   on the cycling, `[V]` on the set.
fn paint_our_castle(cells: &mut [Cell], level: u8) {
    use crate::terrain::{tileset, Lfsr};

    let wall = frames_with_code(level, code::WALL);
    let keep = frames_with_code(level, code::KEEP);
    let bridge = frames_with_code(level, code::DRAWBRIDGE);
    let walk = frames_with_code(level, code::RAISED_1);

    let terrain: Vec<u8> = cells.iter().map(|c| c.terrain).collect();
    let moat = crate::terrain::moat_autotile(&terrain);

    let mut rng = Lfsr::new(0x5EED);
    for (i, cell) in cells.iter_mut().enumerate() {
        let r = rng.next();
        let (x, y) = (i % DIM, i / DIM);
        let pick = |list: &[u8]| -> Option<u8> {
            (!list.is_empty()).then(|| list[(x + y) % list.len()])
        };
        let structure = if cell.surface == SURFACE_WALL {
            pick(&wall)
        } else if cell.surface == SURFACE_KEEP {
            pick(&keep)
        } else if cell.flags & FLAG_DRAWBRIDGE != 0 {
            bridge.get(x % bridge.len().max(1)).copied()
        } else if cell.surface == SURFACE_RAMPART_WALK {
            pick(&walk)
        } else {
            None
        };
        match structure {
            Some(frame) => cell.gfx = frame,
            None if cell.surface == SURFACE_WATER => {
                cell.flags2 = (cell.flags2 & !tileset::MASK) | tileset::SECOND;
                cell.gfx = moat[i];
            }
            None => {
                cell.flags2 = (cell.flags2 & !tileset::MASK) | tileset::SECOND;
                cell.gfx = r & 0x0F;
            }
        }
    }
}

/// **Also ours.** In the original these are filled by
/// `Battlefield_BuildCastle` from the layout raster, and `crate::ai`'s
/// [`AiField`](crate::AiField) documents each of them as `[I]` for exactly that
/// reason. What is *not* invented is their meaning and their arity — sixteen
/// wall slots a group, three groups, twenty defence posts, four approach lanes
/// — all of which are read out of the binary and all of which this fills
/// honestly.
pub fn our_castle_ai_field(field: &Battlefield, level: u8) -> crate::AiField {
    let mut f = crate::runner::ai_field_for(field);
    let level = level.min(4);
    let half = 6 + level as i32 * 2;
    let (cx, cy) = (40i16, 24i16);
    let h = half as i16;

    f.castle_ref = (cx, cy);
    f.castle_objective = [
        cy as usize * DIM + cx as usize,
        (cy + h) as usize * DIM + cx as usize,
    ];
    f.castle_index = 13;
    f.layout = 1;
    f.castle_layout_flag = level >= 3;

    for (step, row) in f.castle_approach.iter_mut().enumerate() {
        let back = h + 2 + (5 - step as i16) * 2;
        for (lane, point) in row.iter_mut().enumerate() {
            *point = (cx + (lane as i16 - 2) * (h / 2).max(1), cy + back);
        }
    }
    for (lane, point) in f.staging.iter_mut().enumerate() {
        *point = (cx + (lane as i16 - 2) * (h / 2).max(1), cy + h + 4);
    }

    // Wall slots: group 0 walks the rampart, group 1 the inner face, group 2
    // the two corners nearest the gate.
    for slot in 0..16i16 {
        let along = -h + (slot * (2 * h)) / 15;
        f.wall_slot[0][slot as usize] = (cx + along, cy + h);
        f.wall_slot[1][slot as usize] = (cx + along, cy + h - 1);
        f.wall_slot[2][slot as usize] = (cx + (if slot % 2 == 0 { -h } else { h }), cy + along);
    }
    // **The defence posts start empty, and that is the original's.** This used
    // to spread twenty of them along the gate wall. `FUN_0048EE46` — the
    // twenty-entry table's only appender in the whole binary — is called from
    // `Wall_Collapse` and from nowhere else, once per rampart neighbour left
    // hanging by a catapult shot. So a castle nobody has bombarded has no
    // defence posts, `Siege_ClaimDefencePost` returns 0 for every unit, and the
    // garrison's handlers take their `cellOffset == 0` arm — the wall slots —
    // for the whole of that siege. The posts are *the holes*, and they arrive
    // when the holes do.
    f.defence_posts = [0; 20];
    f
}

