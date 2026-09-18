#![allow(unused_imports)]
use super::*;



impl Cell {
    pub fn impassable(&self) -> bool {
        self.flags & flag::NO_ENTRY != 0
    }

    /// `Battlefield_Draw32` (`0x004BCBDC`) masks cell byte `+2` with `0x1C`
    /// and dispatches on the result:
    ///
    /// ```c
    /// bVar2 = cell[+2] & 0x1c;
    /// if (bVar2 == 0) { psVar3 = tileset  + cell[+3] * 0x10 + 8; … FUN_004B5333(tileset);  }
    /// else if (bVar2 == 4)
    ///                 { psVar3 = tileset2 + cell[+3] * 0x10 + 8; … FUN_004B5333(tileset2); }
    /// ```
    ///
    /// and no other value of the field draws anything at all. `tileset` and
    /// `tileset2` are `FUN_004BC020`'s first two arguments, which
    /// `Battle_LoadAssets` (`0x004987B7`) fills from slots 0 and 1 of the
    /// battle asset table — `t32_bat1` alone for a field battle,
    /// `t32_stn1`/`t32_stn2` or `t32_wod1`/`t32_wod2` for a siege. **[V]**
    ///
    /// **A field battle is always sheet 0.** `Battlefield_BuildFromSkr` clears
    /// the bits on every cell it writes and never sets them, and slot 1 of the
    /// table is `t32_bat2.pl8` at size **0** — a file the install does not
    /// even ship. **[V]**
    pub fn tileset(&self) -> usize {
        usize::from(self.flags2 & tileset::MASK == tileset::SECOND)
    }
}

#[derive(Debug, Clone, Copy)]
struct Pattern {
    mask: [u8; 8],
    base: u8,
    variants: u8,
}

const fn e(
    n: u8,
    ne: u8,
    ea: u8,
    se: u8,
    s: u8,
    sw: u8,
    w: u8,
    nw: u8,
    base: u8,
    variants: u8,
) -> Pattern {
    Pattern { mask: [n, ne, ea, se, s, sw, w, nw], base, variants }
}

// Each entry is 12 bytes: eight pattern bytes, base, an untraced byte, the
// variant count, and a rotating counter that the build resets (`FUN_0046C40C`)
// and advances on every match (`FUN_0046C2DE`).

/// `0x004D7550`, 11 entries — hills / obstacles (id 4). Entry 10 is a
/// catch-all, so entry 11 (which is physically also the first row of the
/// fringe table below) is unreachable from here.
const HILL: [Pattern; 11] = [
    e(1, 2, 1, 2, 1, 2, 1, 2, 0x40, 16),
    e(0, 2, 1, 2, 1, 2, 1, 2, 0x50, 4),
    e(1, 2, 0, 2, 1, 2, 1, 2, 0x58, 4),
    e(1, 2, 1, 2, 0, 2, 1, 2, 0x60, 4),
    e(1, 2, 1, 2, 1, 2, 0, 2, 0x68, 4),
    e(0, 2, 0, 2, 1, 2, 1, 2, 0x54, 4),
    e(1, 2, 0, 2, 0, 2, 1, 2, 0x5c, 4),
    e(1, 2, 1, 2, 0, 2, 0, 2, 0x64, 4),
    e(0, 2, 1, 2, 1, 2, 0, 2, 0x6c, 4),
    e(2, 2, 2, 2, 2, 2, 2, 2, 0x9f, 1),
    e(1, 2, 2, 2, 2, 2, 1, 2, 0x7b, 1),
];

/// `0x004D75C8`, 6 entries — open ground that borders hills, drawn as a fringe.
const GROUND_FRINGE: [Pattern; 6] = [
    e(1, 2, 2, 2, 2, 2, 1, 2, 0x7b, 1),
    e(1, 2, 2, 2, 2, 2, 0, 1, 0x76, 4),
    e(0, 2, 2, 2, 2, 2, 1, 1, 0x71, 4),
    e(0, 2, 2, 2, 2, 2, 0, 1, 0x75, 1),
    e(1, 2, 2, 2, 2, 2, 0, 0, 0x7a, 1),
    e(0, 2, 2, 2, 2, 2, 1, 0, 0x70, 1),
];

/// `0x004D7610`, 49 entries — water. The last two are unreachable behind the
/// catch-all at index 47; they are kept so the table matches the binary.
const WATER: [Pattern; 49] = [
    e(1, 1, 1, 1, 1, 1, 1, 1, 0xa0, 8),
    e(0, 2, 1, 1, 1, 1, 1, 2, 0xa8, 4),
    e(1, 2, 0, 2, 1, 1, 1, 1, 0xae, 4),
    e(1, 1, 1, 2, 0, 2, 1, 1, 0xb4, 4),
    e(1, 1, 1, 1, 1, 2, 0, 2, 0xba, 4),
    e(0, 2, 1, 2, 0, 2, 1, 2, 0xc5, 1),
    e(1, 2, 0, 2, 1, 2, 0, 2, 0xc6, 1),
    e(0, 2, 0, 2, 1, 1, 1, 2, 0xac, 2),
    e(1, 2, 0, 2, 0, 2, 1, 1, 0xb2, 2),
    e(1, 1, 1, 2, 0, 2, 0, 2, 0xb8, 2),
    e(0, 2, 1, 1, 1, 2, 0, 2, 0xbe, 2),
    e(0, 2, 0, 2, 1, 0, 1, 2, 0xc7, 1),
    e(1, 2, 0, 2, 0, 2, 1, 0, 0xc8, 1),
    e(1, 0, 1, 2, 0, 2, 0, 2, 0xc9, 1),
    e(0, 2, 1, 0, 1, 2, 0, 2, 0xca, 1),
    e(0, 2, 0, 2, 0, 2, 1, 2, 0xc0, 1),
    e(1, 2, 0, 2, 0, 2, 0, 2, 0xc1, 1),
    e(0, 2, 1, 2, 0, 2, 0, 2, 0xc2, 1),
    e(0, 2, 0, 2, 1, 2, 0, 2, 0xc3, 1),
    e(0, 2, 1, 0, 1, 2, 1, 2, 0xcc, 1),
    e(1, 2, 0, 2, 1, 0, 1, 2, 0xcf, 1),
    e(1, 2, 1, 2, 0, 2, 1, 0, 0xd2, 1),
    e(1, 0, 1, 2, 1, 2, 0, 2, 0xd5, 1),
    e(0, 2, 1, 2, 1, 0, 1, 2, 0xcb, 1),
    e(1, 2, 0, 2, 1, 2, 1, 0, 0xce, 1),
    e(1, 0, 1, 2, 0, 2, 1, 2, 0xd1, 1),
    e(1, 2, 1, 0, 1, 2, 0, 2, 0xd4, 1),
    e(0, 2, 1, 0, 1, 0, 1, 2, 0xcd, 1),
    e(1, 2, 0, 2, 1, 0, 1, 0, 0xd0, 1),
    e(1, 0, 1, 2, 0, 2, 1, 0, 0xd3, 1),
    e(1, 0, 1, 0, 1, 2, 0, 2, 0xd6, 1),
    e(1, 0, 1, 0, 1, 0, 1, 0, 0xe5, 1),
    e(1, 0, 1, 0, 1, 0, 1, 1, 0xe1, 1),
    e(1, 1, 1, 0, 1, 0, 1, 0, 0xe2, 1),
    e(1, 0, 1, 1, 1, 0, 1, 0, 0xe3, 1),
    e(1, 0, 1, 0, 1, 1, 1, 0, 0xe4, 1),
    e(1, 0, 1, 1, 1, 0, 1, 1, 0xdf, 1),
    e(1, 1, 1, 0, 1, 1, 1, 0, 0xe0, 1),
    e(1, 0, 1, 0, 1, 1, 1, 1, 0xdb, 1),
    e(1, 1, 1, 0, 1, 0, 1, 1, 0xdc, 1),
    e(1, 1, 1, 1, 1, 0, 1, 0, 0xdd, 1),
    e(1, 0, 1, 1, 1, 1, 1, 0, 0xde, 1),
    e(1, 0, 1, 1, 1, 1, 1, 1, 0xd7, 1),
    e(1, 1, 1, 0, 1, 1, 1, 1, 0xd8, 1),
    e(1, 1, 1, 1, 1, 0, 1, 1, 0xd9, 1),
    e(1, 1, 1, 1, 1, 1, 1, 0, 0xda, 1),
    e(0, 0, 0, 0, 0, 0, 0, 0, 0xc4, 1),
    e(2, 2, 2, 2, 2, 2, 2, 2, 0xc4, 1),
    e(0, 0, 0, 0, 0, 0, 0, 0, 0x00, 0),
];

/// `0x004D7860`, 17 entries
/// lines (id 0x0D), which apply different arithmetic to the same base.
const WOOD: [Pattern; 17] = [
    e(1, 2, 1, 2, 1, 2, 1, 2, 0x12, 1),
    e(1, 2, 1, 2, 1, 2, 0, 2, 0x0e, 1),
    e(0, 2, 1, 2, 1, 2, 1, 2, 0x0f, 1),
    e(1, 2, 0, 2, 1, 2, 1, 2, 0x10, 1),
    e(1, 2, 1, 2, 0, 2, 1, 2, 0x11, 1),
    e(1, 2, 1, 2, 0, 2, 0, 2, 0x04, 1),
    e(0, 2, 1, 2, 1, 2, 0, 2, 0x05, 1),
    e(0, 2, 0, 2, 1, 2, 1, 2, 0x06, 1),
    e(1, 2, 0, 2, 0, 2, 1, 2, 0x07, 1),
    e(1, 2, 0, 2, 1, 2, 0, 2, 0x00, 2),
    e(0, 2, 1, 2, 0, 2, 1, 2, 0x02, 2),
    e(1, 2, 0, 2, 0, 2, 0, 2, 0x08, 1),
    e(0, 2, 1, 2, 0, 2, 0, 2, 0x0a, 1),
    e(0, 2, 0, 2, 1, 2, 0, 2, 0x09, 1),
    e(0, 2, 0, 2, 0, 2, 1, 2, 0x0b, 1),
    e(0, 2, 0, 2, 0, 2, 0, 2, 0x0c, 2),
    e(2, 2, 2, 2, 2, 2, 2, 2, 0x0c, 2),
];

/// The rotating counters that `FUN_0046C2DE` advances. One per table entry,
/// reset at the start of every build.
struct Counters {
    hill: [u8; HILL.len()],
    fringe: [u8; GROUND_FRINGE.len()],
    water: [u8; WATER.len()],
    wood: [u8; WOOD.len()],
}

impl Counters {
    fn new() -> Self {
        Counters {
            hill: [0; HILL.len()],
            fringe: [0; GROUND_FRINGE.len()],
            water: [0; WATER.len()],
            wood: [0; WOOD.len()],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Hit {
    base: u8,
    counter: u8,
}

fn match_table(table: &[Pattern], counters: &mut [u8], mask: [bool; 8]) -> Option<Hit> {
    'entries: for (i, p) in table.iter().enumerate() {
        for k in 0..8 {
            let ok = match p.mask[k] {
                2 => true,
                0 => !mask[k],
                _ => mask[k],
            };
            if !ok {
                continue 'entries;
            }
        }
        // FUN_0046C2DE: increment, then wrap when the count is reached. A
        // one-variant entry therefore always reports 0.
        counters[i] = counters[i].wrapping_add(1);
        if p.variants <= counters[i] {
            counters[i] = 0;
        }
        return Some(Hit { base: p.base, counter: counters[i] });
    }
    None
}

/// The eight neighbours in the order `FUN_0047D816` tests them, derived from
/// the cell-array offsets it indexes: N, NE, E, SE, S, SW, W, NW. Off-map
/// neighbours take `edge`
fn neighbour_mask(terrain: &[u8], x: usize, y: usize, want: u8, edge: bool) -> [bool; 8] {
    let last = DIM - 1;
    let get = |dx: isize, dy: isize| -> bool {
        let nx = x as isize + dx;
        let ny = y as isize + dy;
        terrain[ny as usize * DIM + nx as usize] == want
    };
    [
        if y == 0 { edge } else { get(0, -1) },
        if y == 0 || x == last { edge } else { get(1, -1) },
        if x == last { edge } else { get(1, 0) },
        if x == last || y == last { edge } else { get(1, 1) },
        if y == last { edge } else { get(0, 1) },
        if x == 0 || y == last { edge } else { get(-1, 1) },
        if x == 0 { edge } else { get(-1, 0) },
        if x == 0 || y == 0 { edge } else { get(-1, -1) },
    ]
}

/// **The moat's tile, cell by cell** — `FUN_0047DCCE`, the castle builder's
/// `0xEE` arm, which is the same auto-tiler a field battle's water runs:
///
/// `FUN_0047D816(0x0B, 1)` for the eight-neighbour mask, then
/// `FUN_0046C2DE(0x4D7610, 0x31)` — the 49-entry table and its rotating
/// counters. **[V]**
pub fn moat_autotile(terrain: &[u8]) -> Vec<u8> {
    assert_eq!(terrain.len(), CELLS);
    let mut counters = Counters::new();
    let mut out = vec![0u8; CELLS];
    for y in 0..DIM {
        for x in 0..DIM {
            if terrain[y * DIM + x] != id::WATER {
                continue;
            }
            let mask = neighbour_mask(terrain, x, y, id::WATER, true);
            if let Some(h) = match_table(&WATER, &mut counters.water, mask) {
                out[y * DIM + x] = h.base.wrapping_add(h.counter);
            }
        }
    }
    out
}

pub fn build(layer: &[u8], seed: u32) -> Battlefield {
    assert_eq!(layer.len(), CELLS, "a .skr terrain layer is exactly 80 x 80 bytes");

    let mut src = layer.to_vec();
    let mut terrain = vec![0u8; CELLS];
    for y in 0..DIM {
        for x in 0..DIM {
            let i = y * DIM + x;
            terrain[i] = match src[i] {
                0x00 => id::OPEN,
                0x02 => id::OBSTACLE,
                0x10 => id::BRIDGE_NEAR,
                0x14 => id::BRIDGE_FAR,
                0x12 => {
                    for step in [DIM, 2 * DIM] {
                        if i + step < CELLS && src[i + step] == 0x10 {
                            src[i + step] = 0x14;
                        }
                    }
                    id::BRIDGE_SPAN
                }
                0x09 => id::WATER,
                0x04 => id::MARKER_SIDE0,
                0x0F => id::MARKER_SIDE4,
                0x20 => id::ROCKS,
                0x50 => id::UNUSED6,
                0x0A => id::WOODLAND,
                0x15 => id::LINE,
                _ => id::OPEN,
            };
        }
    }

    let mut deploy_side0 = [(0u8, 0u8); 12];
    let mut deploy_side4 = [(0u8, 0u8); 12];
    let mut home_side0 = (0u8, 0u8);
    let mut home_side4 = (0u8, 0u8);
    for y in 0..DIM {
        for x in 0..DIM {
            let i = y * DIM + x;
            let slots = match terrain[i] {
                id::MARKER_SIDE0 => {
                    home_side0 = (x as u8, y as u8);
                    &mut deploy_side0
                }
                id::MARKER_SIDE4 => {
                    home_side4 = (x as u8, y as u8);
                    &mut deploy_side4
                }
                _ => continue,
            };
            for (slot, (dx, dy)) in slots.iter_mut().zip(DEPLOY_OFFSETS) {
                let sx = (x as i32 + dx).clamp(1, 78) as u8;
                let sy = (y as i32 + dy).clamp(1, 78) as u8;
                *slot = (sx, sy);
            }
            terrain[i] = id::OPEN;
        }
    }

    let mut cells = vec![Cell::default(); CELLS];
    for (c, &t) in cells.iter_mut().zip(terrain.iter()) {
        c.terrain = t;
    }
    graphics_pass(&mut cells, &terrain, seed, true);

    bridge_pass(&mut cells);

    Battlefield { cells, deploy_side0, deploy_side4, home_side0, home_side4 }
}

/// **The third pass of both open-field builders** — `Battlefield_BuildFromSkr`
/// (`0x0047B8B2`) and `Battlefield_BuildRandom` (`0x0047AAA3`) run the same
/// code here: clear cell byte `+2`'s sheet bits, step the LFSR once per cell
/// whether or not the arm uses it, and give ground, hills, water, woodland and
/// the `0x15` lines their frame out of the four auto-tiling tables.
///
/// `random_rocks` is the one difference. The `.skr` builder picks a random
/// variant for ids 3 and 6 here; `Battlefield_BuildRandom` gave them their
/// frame in its *first* pass, straight out of the source byte, and has no arm
/// for them at all. **[V]** on both decompilations.
pub(super) fn graphics_pass(cells: &mut [Cell], terrain: &[u8], seed: u32, random_rocks: bool) {
    let mut rng = Lfsr::new(seed);
    let mut counters = Counters::new();
    for y in 0..DIM {
        for x in 0..DIM {
            let i = y * DIM + x;
            let r = rng.next();
            let t = terrain[i];
            let cell = &mut cells[i];
            cell.flags2 &= !tileset::MASK;
            match t {
                id::OPEN => {
                    let mask = neighbour_mask(&terrain, x, y, id::OBSTACLE, false);
                    cell.gfx = match match_table(&GROUND_FRINGE, &mut counters.fringe, mask) {
                        Some(h) => h.base.wrapping_add(h.counter),
                        None => r & 0x0F,
                    };
                }
                id::OBSTACLE => {
                    let mask = neighbour_mask(&terrain, x, y, id::OBSTACLE, true);
                    if let Some(h) = match_table(&HILL, &mut counters.hill, mask) {
                        cell.gfx = h.base.wrapping_add(h.counter);
                    }
                    cell.flags |= flag::IMPASSABLE | flag::BLOCKED;
                }
                id::WATER => {
                    let mask = neighbour_mask(&terrain, x, y, id::WATER, true);
                    if let Some(h) = match_table(&WATER, &mut counters.water, mask) {
                        cell.gfx = h.base.wrapping_add(h.counter);
                    }
                    cell.flags |= flag::IMPASSABLE;
                }
                id::ROCKS if random_rocks => {
                    cell.gfx = (r & 7) + 0x20;
                    cell.flags |= flag::IMPASSABLE;
                }
                id::UNUSED6 if random_rocks => {
                    cell.gfx = (r & 7) + 0x7C;
                    cell.flags |= flag::IMPASSABLE;
                }
                id::WOODLAND => {
                    let mask = neighbour_mask(&terrain, x, y, id::WOODLAND, true);
                    if let Some(h) = match_table(&WOOD, &mut counters.wood, mask) {
                        cell.gfx = if h.base < 0x10 {
                            h.counter.wrapping_add(h.base).wrapping_add(0x10)
                        } else {
                            h.base.wrapping_sub(0x17)
                        };
                        cell.surface = 0x0F;
                    }
                }
                id::LINE => {
                    let mask = neighbour_mask(&terrain, x, y, id::LINE, true);
                    if let Some(h) = match_table(&WOOD, &mut counters.wood, mask) {
                        cell.gfx = h.counter.wrapping_add(h.base).wrapping_sub(0x1A);
                    }
                    cell.flags |= flag::IMPASSABLE;
                }
                _ => {}
            }
        }
    }
}

/// **The fourth pass of both open-field builders** — the three `FUN_0047D6FE`
/// calls at the tail of `Battlefield_BuildFromSkr` and of
/// `Battlefield_BuildRandom`, byte for byte the same in each. Each bridge part
/// stamps a 4-wide block of consecutive graphics, clears the terrain id behind
/// it so the block is stamped once, and marks the two outer columns as
/// parapets. The cells either side get a fixed water graphic.
pub(super) fn bridge_pass(cells: &mut [Cell]) {
    for y in 0..DIM {
        for x in 0..DIM {
            let i = y * DIM + x;
            let (gfx0, rows, side_gfx, side_row) = match cells[i].terrain {
                id::BRIDGE_NEAR => (0x8Cu8, 2usize, 0xA8u8, 1usize),
                id::BRIDGE_SPAN => (0x94, 1, 0xA0, 0),
                id::BRIDGE_FAR => (0x98, 2, 0xB4, 0),
                _ => continue,
            };
            stamp(cells, x, y, gfx0, 4, rows);
            let sy = y + side_row;
            if sy < DIM {
                if x >= 1 {
                    cells[sy * DIM + x - 1].gfx = side_gfx;
                }
                if x + 4 < DIM {
                    cells[sy * DIM + x + 4].gfx = side_gfx;
                }
                cells[sy * DIM + x].flags |= flag::IMPASSABLE;
                if x + 3 < DIM {
                    cells[sy * DIM + x + 3].flags |= flag::IMPASSABLE;
                }
            }
        }
    }
}

/// `FUN_0047D6FE` — write a `cols x rows` block of consecutive graphic indices
/// and clear the terrain id behind it.
pub(super) fn stamp(cells: &mut [Cell], x: usize, y: usize, gfx0: u8, cols: usize, rows: usize) {
    let mut g = gfx0;
    for r in 0..rows {
        for c in 0..cols {
            let (cx, cy) = (x + c, y + r);
            if cx < DIM && cy < DIM {
                let cell = &mut cells[cy * DIM + cx];
                cell.gfx = g;
                cell.terrain = 0;
            }
            g = g.wrapping_add(1);
        }
    }
}

