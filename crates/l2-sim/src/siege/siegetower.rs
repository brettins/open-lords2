#![allow(unused_imports)]
use super::*;
use super::castle::*;
use super::damage::*;
use super::drawbridge::*;
use super::moat::*;
use crate::terrain::{Battlefield, Cell, DIM};

/// The four orthogonal neighbours of a cell, clipped at the field edge — the
/// order `FUN_0047DD86` and `FUN_0047DFE0` both read them in: north, east,
/// south, west.
pub fn orthogonal_neighbours(cell: usize) -> impl Iterator<Item = usize> {
    let (x, y) = ((cell % DIM) as i32, (cell / DIM) as i32);
    [(0i32, -1i32), (1, 0), (0, 1), (-1, 0)].into_iter().filter_map(move |(dx, dy)| {
        let (nx, ny) = (x + dx, y + dy);
        (nx >= 0 && ny >= 0 && nx < DIM as i32 && ny < DIM as i32)
            .then_some(ny as usize * DIM + nx as usize)
    })
}

// The shipped `Readme.txt`, *Siege Towers*: **"Once siege towers reach a wall
// and 'dock' with it, they can not be moved again."** `docs/battle.md` §14.3b
// filed that rule as *unlocated*. It is `FUN_00491492` (`0x00491492`), and the
// reason a docked tower cannot be moved is that **there is no tower any more**:

/// `DAT_004D9D90` — **the cell two ahead** of a siege tower's centre in each
/// polar facing, as byte offsets `-1280, 0, 16, 0, 1280, 0, -16, 0` read out
/// of `Lords2.exe` and divided by eight. The odd rows are zero: a tower's
/// polar facing is always orthogonal. `[V]`.
pub const DOCK_REACH: [(i32, i32); 8] =
    [(0, -2), (0, 0), (2, 0), (0, 0), (0, 2), (0, 0), (-2, 0), (0, 0)];
/// `DAT_004D9DB0` — **the cell one ahead**, the tower's own leading edge:
///
/// `-640, 0, 8, 0, 640, 0, -8, 0`. `[V]`.
pub const DOCK_EDGE: [(i32, i32); 8] =
    [(0, -1), (0, 0), (1, 0), (0, 0), (0, 1), (0, 0), (-1, 0), (0, 0)];
/// **A tower docks only against ground exactly two high.** `FUN_00491492`
/// tests `elevation == 2` on the cell two ahead — not "at least two" — and
/// refuses when the cell one ahead is exactly 1. `[V]`.
pub const DOCK_WALL_ELEVATION: u8 = 2;
/// What a dock is worth to the besieger's order handlers: `g_siegeBreachScore
/// += 3`, `g_siegeApproachScore += 4`. `[V]`.
pub const DOCK_BREACH_SCORE: i32 = 3;
pub const DOCK_APPROACH_SCORE: i32 = 4;

/// `s__ABHIJPQR_004d9dd0` — the frames the 3 × 3 under a docked tower is
/// redrawn with, **nine a facing at offset `facing × 9`**, so rows 0, 18, 36
/// and 54 are live and the nine bytes after each are zero. Read out of the
/// executable; Ghidra typed it as a string because the first row is
/// `@ABHIJPQR`. `[V]`.
pub const DOCK_FRAMES: [u8; 72] = [
    64, 65, 66, 72, 73, 74, 80, 81, 82, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    67, 68, 69, 75, 76, 77, 83, 84, 85, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    88, 89, 90, 96, 97, 98, 104, 105, 106, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    91, 92, 93, 99, 100, 101, 107, 108, 109, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
];

/// `g_engineEdgeOrtho` (`0x004D9C90`) — **the three cells a 3 × 3 siege
/// engine sweeps as it steps orthogonally**, by facing, decoded from the
/// executable's byte offsets. The odd rows are dead. `[V]`.
pub const ENGINE_EDGE_ORTHO: [[(i32, i32); 3]; 8] = [
    [(-1, -2), (0, -2), (1, -2)],
    [(0, 0); 3],
    [(2, -1), (2, 0), (2, 1)],
    [(0, 0); 3],
    [(-1, 2), (0, 2), (1, 2)],
    [(0, 0); 3],
    [(-2, -1), (-2, 0), (-2, 1)],
    [(0, 0); 3],
];
/// `g_engineEdgeDiag` (`0x004D9CF0`) — the **five** a diagonal step exposes,
/// the L round the corner. The even rows are dead. `[V]`.
pub const ENGINE_EDGE_DIAG: [[(i32, i32); 5]; 8] = [
    [(0, 0); 5],
    [(0, -2), (1, -2), (2, -2), (2, -1), (2, 0)],
    [(0, 0); 5],
    [(2, 0), (2, 1), (2, 2), (1, 2), (0, 2)],
    [(0, 0); 5],
    [(0, 2), (-1, 2), (-2, 2), (-2, 1), (-2, 0)],
    [(0, 0); 5],
    [(-2, 0), (-2, -1), (-2, -2), (-1, -2), (0, -2)],
];

/// **A walking siege tower's polar facing** — `FUN_00488436` (`0x00488436`)'s
/// troop-8 arm, run every frame the tower walks.
///
/// The nearest orthogonal to the way it is going, and **on a diagonal it keeps
/// whichever of the two it already had**. It is also the sprite: a tower is
/// drawn in four facings, not eight. `[V]`.
pub fn tower_polar(dirc: u8, polar: u8) -> u8 {
    let c = match dirc {
        1 => u8::from(polar == 2),
        3 => {
            if polar == 2 {
                1
            } else {
                2
            }
        }
        5 => {
            if polar == 6 {
                3
            } else {
                2
            }
        }
        7 => {
            if polar == 6 {
                3
            } else {
                0
            }
        }
        d => d >> 1,
    };
    c * 2
}

/// **Where a tower at `(x, y)` would dock** — `FUN_00491492`'s search, which
/// `BattleMan_Step` runs whenever a siege engine's step is refused.
///
/// ```c
/// if (troopType == 8)
///   for (k = 0, d = polar; k < 4; k++, d = (d + 2) % 8)
///     if (cell[centre + DAT_004D9D90[d]].elevation == 2
///         && cell[centre + DAT_004D9DB0[d]].elevation != 1)  → dock facing d
/// ```
pub fn tower_dock_site(field: &Battlefield, x: i32, y: i32, polar: u8) -> Option<(u8, usize)> {
    let inside = |x: i32, y: i32| (0..DIM as i32).contains(&x) && (0..DIM as i32).contains(&y);
    let mut d = polar as usize % 8;
    for _ in 0..4 {
        let (ax, ay) = (x + DOCK_REACH[d].0, y + DOCK_REACH[d].1);
        let (bx, by) = (x + DOCK_EDGE[d].0, y + DOCK_EDGE[d].1);
        if inside(ax, ay)
            && inside(bx, by)
            && field.at(ax as usize, ay as usize).elevation == DOCK_WALL_ELEVATION
            && field.at(bx as usize, by as usize).elevation != 1
        {
            return Some((d as u8, ay as usize * DIM + ax as usize));
        }
        d = (d + 2) % 8;
    }
    None
}

/// **The ramp a docked tower leaves** — the battlefield half of
/// `FUN_00491492`, with `FUN_004921E5` (`0x004921E5`) inside it.
///
/// ```c
/// wall.flags = 0;                           /* the way onto the wall is open  */
/// BattleMan_Destroy(tower);
/// FUN_004921E5(x, y, d):                    /* the ramp                        */
///     centre.elevation = 1;  the four diagonals |= 0x10;
///     d == 0 || d == 4 ? { (x±1, y) |= 0x10;  (x, y±1).elevation = 1; }
///                      : { (x, y±1) |= 0x10;  (x±1, y).elevation = 1; }
/// for the 3 x 3: flags |= 1; frame = DAT_004D9DD0[k + d*9];
/// wall.frame = (d == 0 || d == 4) ? 1 : 2;
/// edge.elevation = 2;                       /* the top step                   */
/// ```
///
/// `FUN_004921E5`'s own edge guards test the wrong axis for two of its writes
/// (`x < 0x4F` guarding a write to `y − 1`); that differs only at the field's
/// edge, and here every write is clipped to the field instead. Returns every
/// cell written.
pub fn lay_tower_ramp(field: &mut Battlefield, x: i32, y: i32, d: u8, wall: usize) -> Vec<usize> {
    let mut touched = Vec::new();
    let inside = |x: i32, y: i32| (0..DIM as i32).contains(&x) && (0..DIM as i32).contains(&y);
    let mut write = |field: &mut Battlefield, cx: i32, cy: i32, f: &dyn Fn(&mut Cell)| {
        if inside(cx, cy) {
            let c = cy as usize * DIM + cx as usize;
            f(&mut field.cells[c]);
            touched.push(c);
        }
    };
    field.cells[wall].flags = 0;
    write(field, x, y, &|c| c.elevation = 1);
    for (dx, dy) in [(1, -1), (1, 1), (-1, 1), (-1, -1)] {
        write(field, x + dx, y + dy, &|c| c.flags |= crate::terrain::flag::IMPASSABLE);
    }
    let along_y = d == 0 || d == 4;
    let (flank, step) = if along_y { ((1, 0), (0, 1)) } else { ((0, 1), (1, 0)) };
    for s in [1, -1] {
        write(field, x + s * flank.0, y + s * flank.1, &|c| c.flags |= crate::terrain::flag::IMPASSABLE);
        write(field, x + s * step.0, y + s * step.1, &|c| c.elevation = 1);
    }
    let mut k = 0usize;
    for cy in y - 1..=y + 1 {
        for cx in x - 1..=x + 1 {
            let frame = DOCK_FRAMES[k + d as usize * 9];
            write(field, cx, cy, &|c| {
                c.flags |= 1;
                c.gfx = frame;
            });
            k += 1;
        }
    }
    field.cells[wall].gfx = if along_y { 1 } else { 2 };
    let (ex, ey) = (x + DOCK_EDGE[d as usize].0, y + DOCK_EDGE[d as usize].1);
    write(field, ex, ey, &|c| c.elevation = DOCK_WALL_ELEVATION);
    touched.push(wall);
    touched.sort_unstable();
    touched.dedup();
    touched
}

