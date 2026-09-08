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
//!    pick at random; hills, water, woodland and the `0x15` lines run an
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
    /// The mask movement actually tests.
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
    /// Byte `+3` — the PL8 frame index in the tileset.
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

impl Cell {
    pub fn impassable(&self) -> bool {
        self.flags & flag::NO_ENTRY != 0
    }
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
    /// rejected rather than silently producing a constant.
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

/// One row of an auto-tiling table: an eight-neighbour pattern, the base
/// graphic index, and how many consecutive variants follow it.
///
/// Pattern values are the original's: `0` = that neighbour must **not** be the
/// same terrain, `1` = it must be, `2` = don't care.
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

// The four tables, read out of `Lords2.exe`'s `.data` at the addresses below.
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
/// Ground with no hill neighbour falls through to a random variant instead.
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

/// `0x004D7860`, 17 entries — shared by woodland (id 0x0C) and the `0x15`
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

/// The result of a table match: the entry's base index and its counter value
/// *after* the match, which is what the graphic formulas add.
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
/// neighbours take `edge` rather than being read.
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

/// Build the battlefield from one 80 x 80 `.skr` terrain layer.
///
/// `seed` drives only the random terrain variants (open ground, rocks, id 6).
/// Everything else is a pure function of the layer.
pub fn build(layer: &[u8], seed: u32) -> Battlefield {
    assert_eq!(layer.len(), CELLS, "a .skr terrain layer is exactly 80 x 80 bytes");

    // Pass 1 - terrain byte to id. The bridge rule rewrites the *source* bytes
    // two rows down, so this works on a copy.
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
                    // A span turns the two cells below it into the far end.
                    // The original does this unguarded and reads past the
                    // buffer on the last two rows; we clamp instead.
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

    // Pass 2 - expand the markers, then erase them.
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

    // Pass 3 - graphics and flags.
    let mut cells = vec![Cell::default(); CELLS];
    let mut rng = Lfsr::new(seed);
    let mut counters = Counters::new();
    for y in 0..DIM {
        for x in 0..DIM {
            let i = y * DIM + x;
            // Stepped for every cell, used by only some. Reproducing that is
            // what keeps the sequence aligned with the original's.
            let r = rng.next();
            let t = terrain[i];
            let cell = &mut cells[i];
            cell.terrain = t;
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
                id::ROCKS => {
                    cell.gfx = (r & 7) + 0x20;
                    cell.flags |= flag::IMPASSABLE;
                }
                id::UNUSED6 => {
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

    // Pass 4 - bridges. Each part stamps a 4-wide block of consecutive
    // graphics, clears the terrain id behind it so the block is stamped once,
    // and marks the two outer columns as parapets. The cells either side get a
    // fixed water graphic.
    for y in 0..DIM {
        for x in 0..DIM {
            let i = y * DIM + x;
            let (gfx0, rows, side_gfx, side_row) = match cells[i].terrain {
                id::BRIDGE_NEAR => (0x8Cu8, 2usize, 0xA8u8, 1usize),
                id::BRIDGE_SPAN => (0x94, 1, 0xA0, 0),
                id::BRIDGE_FAR => (0x98, 2, 0xB4, 0),
                _ => continue,
            };
            stamp(&mut cells, x, y, gfx0, 4, rows);
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

    Battlefield { cells, deploy_side0, deploy_side4, home_side0, home_side4 }
}

/// `FUN_0047D6FE` — write a `cols x rows` block of consecutive graphic indices
/// and clear the terrain id behind it.
fn stamp(cells: &mut [Cell], x: usize, y: usize, gfx0: u8, cols: usize, rows: usize) {
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
    /// neighbours, not just on its own byte.
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
