#![allow(unused_imports)]
use super::*;
use super::damage::*;
use super::drawbridge::*;
use super::moat::*;
use super::siegetower::*;
use crate::terrain::{Battlefield, Cell, DIM};

/// [`APPROACH_SCORE_START`] or zero, by whether this battlefield has a ditch —
/// the two-line consequence of the three writes that constant documents.
pub fn approach_score_at_build(field: &Battlefield) -> i32 {
    if field.cells.iter().any(|c| c.surface == SURFACE_WATER) {
        0
    } else {
        APPROACH_SCORE_START
    }
}

// ---------------------------------------------------------------------------
// Our castle
// ---------------------------------------------------------------------------

/// **The name is a warning.** This is *our* castle, not the original's — see
/// the module header. It exists so that the fourteen siege order handlers have
/// somewhere to run, and so the damage model has a wall to eat.
///
/// One concentric keep, laid out around the defender's deployment marker:
///
/// ```text
///                 . . . . . . . . . . . . . .      the open field, surface 1
///             ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~      the moat, surface 2, from level 2 up
///             # # # # # # # # # # # # # # # #      the curtain wall, surface 8, flag 0x20, elevation 1
///             # r r r r r r r r r r r r r r #      the rampart walk, surface 4, elevation 1
///             # r . . . . . . . . . . . . r #      the bailey, surface 5, elevation 0
///             # r . . . . K . . . . . . . r #      K: the way in, flag 0x08, surface 6
///             # # # # # G # # # # # # # # # #      G: the gatehouse, flag 0x40
///                       ^ the drawbridge, level 3 and up
/// ```
///
/// Size grows with the level, and two features are gated on it because the
/// game says so: the moat from level 2, and the drawbridge from level 3 —
/// *"only the Stone and Royal castles have drawbridges"*.
///
/// The two deployment markers are the field's: the garrison starts inside, the
/// besieger on the far side of the wall.
///
/// # The surfaces here were the inverse of the binary's, and it was fatal
///
/// This layout used to make the wall `surface = 5` at **elevation 2** and the
/// bailey `surface = 3` at elevation 1. Three things followed and all three
/// broke the siege:
///
/// * a wall two cells high is a wall nobody can step onto after it has been
///   opened — `movement::can_step_elevation` allows a difference of one, and
///   the original's builder writes [`WALL_ELEVATION`];
/// * `Siege_FindCellSurface4`, the only search any order handler runs by
///   surface value, had nothing to find, so **no defender ever posted on the
///   wall**;
/// * a besieger standing outside on a 5 fed the *rampart* accumulator at 5,000
///
///
/// The layout is still ours. The five values in it are the binary's.
pub fn our_castle(level: u8) -> Battlefield {
    use crate::terrain::{flag, id};

    let level = level.min(4);
    // A palisade is a small ring and a royal castle a large one. These numbers
    // are ours; nothing in the binary says a castle is any particular size.
    let half = 6 + level as i32 * 2;
    let (cx, cy) = (40i32, 24i32);

    // The open field. Surface 1 is what the castle build's flood classifier
    // reaches from the two map corners, and it is what everything that is not
    // the castle ends up as.
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

    // The moat, one ring outside the wall, from a Norman keep up.
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

    // A ring of ground-level apron immediately outside the moat — surface 3,
    // which is what the classifier writes for elevation-0 ground beside the
    // castle, and what `Moat_Fill` counts as one point of approach score for
    // each neighbour it opens onto.
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

    // The bailey, the rampart walk, and the curtain wall.
    for y in (cy - half)..=(cy + half) {
        for x in (cx - half)..=(cx + half) {
            if !(0..DIM as i32).contains(&x) || !(0..DIM as i32).contains(&y) {
                continue;
            }
            // **Two cells in, not one.** The ring immediately inside the wall
            // has to be bailey, because `Wall_Collapse` scores a breach only
            // for the orthogonal neighbours of the collapsed cell that are at
            // [`SURFACE_BAILEY`] — put anything else against the wall's inner
            // face and a catapult can knock the whole curtain down for a
            // breach score of **zero**, which is a besieger whose engines
            // achieve nothing. Measured, not reasoned: four collapsed cells,
            // `breach_score` 0, 100,000 frames.
            let on_walk =
                (x - cx).abs() == half - 2 || (y - cy).abs() == half - 2;
            let c = &mut cells[at(x, y)];
            if on_ring(x, y) {
                c.surface = SURFACE_WALL;
                c.elevation = WALL_ELEVATION;
                c.flags |= FLAG_WALL;
            } else if on_walk {
                // Surface 4 — the one surface an order handler searches for.
                // Without it `Siege_FindCellSurface4` never finds anything and
                // four defender actions are dead.
                c.surface = SURFACE_RAMPART_WALK;
                c.elevation = WALL_ELEVATION;
            } else {
                c.surface = SURFACE_BAILEY;
                c.elevation = 0;
            }
        }
    }

    // **The gatehouse**, in the middle of the wall facing the besieger, where
    // the level has one — *"only the Stone and Royal castles have
    // drawbridges."*
    //
    // It is `DRAWBRIDGE_COLS` wide and three deep so that the patch
    // [`lower_drawbridge`] lays down lands on it: the routine anchors on the
    // **first** `0x40` cell in row-major order and runs 7 × 4 south and east
    // from there, so the block's north-west corner has to be the north-west
    // corner of the patch. Four wall cells, then the moat, then three cells of
    // open ground — which is a bridge across the ditch and a hole in the wall,
    // in one stroke, and is exactly what the four bytes the original writes
    // amount to.
    //
    // While it is up the cells are flagged `0x40`, which `Cell_TryEnter`
    // refuses to **both** sides: a raised drawbridge is a shut gate.
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

    // **The way in** — one cell, in the middle of the bailey, and it is left at
    // the bailey's own elevation.
    //
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
        // The garrison deploys inside, on the bailey; the besieger on the open
        // ground well south of the moat.
        home_side0: (cx as u8, (cy + half / 2) as u8),
        home_side4: (cx as u8, 64),
    };
    for (slot, (dx, dy)) in crate::terrain::DEPLOY_OFFSETS.iter().enumerate() {
        let clamp = |v: i32| v.clamp(1, DIM as i32 - 2) as u8;
        // Side 0's slots are squeezed to fit inside the bailey; side 4's are
        // the field's own spread.
        let sx = (dx / 3).clamp(-half + 2, half - 2);
        let sy = (dy / 3).clamp(-half + 2, half - 2);
        field.deploy_side0[slot] =
            (clamp(field.home_side0.0 as i32 + sx), clamp(field.home_side0.1 as i32 + sy));
        field.deploy_side4[slot] =
            (clamp(field.home_side4.0 as i32 + dx), clamp(field.home_side4.1 as i32 + dy));
    }
    field
}

// ---------------------------------------------------------------------------
// What a siege's cells are *drawn* with
// ---------------------------------------------------------------------------

/// **The structure table `Battlefield_BuildCastle` reads every raster byte
/// through** — `0x004D7B80` for a stone castle and `0x004D7D80` for a wooden
/// one, 256 entries of two bytes, indexed by the frame index itself:
///
/// ```c
/// cell.frame = b;                                  /* the raster byte */
/// cell.elevation = table[b * 2];                   /* height, or a structure code */
/// if (table[b * 2 + 1] == 0) cell.flags |= 0x10;   /* and impassable */
/// ```
///
/// The first byte is a height 0…4 for an ordinary tile and one of the codes
/// 5…12 for a structure, which the builder then expands into a surface, flags
/// and a real elevation — [`code`] lists them. The second is passability.
/// **[V]**, read out of `Lords2.exe` at file offsets `0xD5D80` and `0xD5F80`;
/// `crates/l2-game/tests/siege_picture/main.rs` re-reads the player's own copy and
/// fails if these bytes drift.
///
/// **Which castle gets which is `DAT_0057C910`**, `(uint)(1 < g_castleLevel)`
/// — the same flag that picks `t32_stn1` over `t32_wod1`, so the table and
/// the sheet always agree. **[V]**
///
/// Two things the pair of tables says on its own: the stone table has four
/// entries at code **9**, the drawbridge, at `0xA4`…`0xA7`, and the wooden
/// table has **none** — which is the Readme's *"only the Stone and Royal
/// castles have drawbridges"* from inside the art. And code 8, the wall, is
/// `0xAC`…`0xAF` in stone and `0x76`…`0x79` in wood: four tiles either way.
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

/// Every frame index the level's structure table files under `want`, in index
/// order. The lists are short — four, usually — and this is how the tile a
/// structure is drawn with is *derived*.
pub fn frames_with_code(level: u8, want: u8) -> Vec<u8> {
    let table = if level > 1 { &STRUCTURE_STONE } else { &STRUCTURE_WOOD };
    (0..256u16).filter(|&f| table[f as usize * 2] == want).map(|f| f as u8).collect()
}

/// **Give [`our_castle`]'s cells something to be drawn with**, and say exactly
/// how much of it is the game's.
///
/// * **The ground and the ditch are the game's.** `Battlefield_BuildCastle`'s
///   escape codes `0xEF` and `0xEE` — `FUN_0047E1DC` and `FUN_0047DCCE` —
/// put open ground on `rand & 0x0F` and the moat on the 49-variant water
///   auto-tiler, both with the cell's tileset selector set to **slot 1**,
///   `t32_stn2` / `t32_wod2`. Reproduced exactly, including the selector.
///   **[V]**
/// * **The castle's tiles are the game's; its shape is not.** Which of
///   `t32_stn1`'s 256 frames is a wall, a keep door or a drawbridge plank is
///   [`frames_with_code`], read out of the binary — but *where* those go is
/// [`our_castle`]'s ring, and the original's is a raster in `stnfield.pl8`
///   we have not read. Where a code carries several frames they are cycled by
///   position; which one the original's raster picks is the raster's. `[I]`
///   on the cycling, `[V]` on the set.
/// * **The bailey takes ground tiles**, because no structure code describes an
///   open courtyard and inventing masonry for one would be inventing the
///   castle. The rampart walk takes code 11, whose expansion —
///   `flags = 4, elevation = 1` — is exactly what the walk is.
///
/// The LFSR is seeded with a constant and stepped once a cell, which is what
/// the original does; the seed it starts a battle from is untraced — the same
/// note `terrain::build` carries.
fn paint_our_castle(cells: &mut [Cell], level: u8) {
    use crate::terrain::{tileset, Lfsr};

    let wall = frames_with_code(level, code::WALL);
    let keep = frames_with_code(level, code::KEEP);
    let bridge = frames_with_code(level, code::DRAWBRIDGE);
    // Code 11 is `flags = 4, elevation = 1` with no wall flag, which is
    // exactly the shape of our rampart walk. The stone table files four; the
    // wooden table files none, and a palisade's walk then stays on the ground
    // tiles — which is the right answer for a palisade anyway.
    let walk = frames_with_code(level, code::RAISED_1);

    // The moat first: the auto-tiler wants the whole terrain plane, and its
    // rotating counters make the walk order part of the answer.
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
            // Slot 0 — the castle sheet — and the selector stays clear, which
            // is what every cell taken from the raster does.
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

/// The positional tables the siege handlers read, derived from [`our_castle`]'s
/// geometry.
///
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

    // Four approach lanes onto the gate wall, at six stand-off distances.
    //
    // **The furthest of these has to stay inside the ditch search's reach**,
    // and it did not. `Order_ToBreachOrStaging`'s first arm hunts for a
    // surface-2 cell within a radius of 12 to 19 of the unit and **does nothing
    // at all** when it finds none — so a unit parked further out than that from
    // the moat never shovels, never raises the approach score, never passes the
    // `approach_score < 3` gate, and never assaults. The ladder alternates
// staging with the ditch hunt on the assumption that staging is
    // close enough, and ours was 21 cells beyond the water: measured, 848 men
    // sat in the field for 200,000 frames with a full ditch in front of them.
    // Six rings two cells apart, the outermost `h + 12` from the centre, keeps
    // every rung of the ladder inside the search.
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

