#![allow(unused_imports)]
use super::*;
use super::search_part::*;
use super::tests_part::*;


/// Chebyshev distance — diagonals cost the same as orthogonals, which is what
/// makes "adjacent" mean any of the eight neighbours.
pub fn chebyshev(a: Pos, b: Pos) -> u32 {
    let dx = (a.x as i32 - b.x as i32).unsigned_abs();
    let dy = (a.y as i32 - b.y as i32).unsigned_abs();
    dx.max(dy)
}

impl Grid {
    pub fn open() -> Self {
        Grid {
            step_cost: vec![0; CELLS],
            elevation: vec![0; CELLS],
            blocked: vec![false; CELLS],
            occupied: vec![false; CELLS],
        }
    }

    pub fn block(&mut self, p: Pos) {
        self.blocked[p.index()] = true;
    }

    /// Mark a friendly figure. Enemy figures are deliberately *never* marked:
    /// the original routes straight through them and leaves contact to the mover.
    pub fn occupy(&mut self, p: Pos) {
        self.occupied[p.index()] = true;
    }

    pub fn set_cost(&mut self, p: Pos, cost: u8) {
        self.step_cost[p.index()] = cost;
    }

    pub fn set_elevation(&mut self, p: Pos, e: u8) {
        self.elevation[p.index()] = e;
    }

    /// A step is allowed when the two cells differ by at most one level, unless
    /// the destination is exactly 5 — the same rule movement uses.
    pub(super) fn step_allowed(&self, from: usize, to: usize) -> bool {
        let (a, b) = (self.elevation[from] as i32, self.elevation[to] as i32);
        (b - a).abs() <= 1 || b == 5
    }

/// **`Path_LineIsClear` (`0x004710F2`) as the original writes it**
    /// — and it is not a line
    ///
    /// It is a **two-pronged greedy walk that leaves a cost field behind**, and
    /// all three of those matter:
    ///
    /// * it seeds `g_pathCost` from the blocked template **and calls
    /// `Path_BuildBlockedMap`**
    /// here;
    /// * two walkers set out from the start together, each step choosing the
    ///   eight-way direction toward the target and, when that cell is taken,
    ///   rotating — one clockwise, the other anticlockwise, up to eight tries.
///   So the walk **slips around** a body in the way
    ///   it. Eighty rounds of the pair, then it gives up;
    /// * and the cost field it writes is left in place for `Path_Extract`. When
    ///   `Path_Search` skips its flood fill *because this succeeded*, the
    ///   caller still runs `Path_Extract` and gets **the walked route**,
    ///   detours and all.
    ///
    /// > **That third property is the deadlock.** Ours was a strict Bresenham
    /// > returning a bare `bool`, and `search` answered `NoSearchNeeded` with
    /// > an empty cost field —
    /// > comrade asked for a path, was told none was needed, and got nothing.
    /// > It then tried the same blocked step again, for ever. One figure does
    /// > that invisibly; an army packed six deep in front of a gate does it as
    /// > a permanent jam, and no test shorter than the jam could see it.
    /// > Measured: 45 besiegers frozen in a block eight cells wide for 200,000
    /// > frames, every one `Walking`, `barred` 0, path empty.
    /// > `docs/decisions.md` `C103`.
    ///
    /// Returns the cost field when the walk reached the destination. `[V]` —
    /// the two rotation directions, the eight-try inner loop, the 80-round cap
    /// and the `step_cost == 0` requirement are all read straight out of it.
    pub fn walk_line(&self, from: Pos, to: Pos) -> Option<Vec<u16>> {
        let mut cost = vec![0u16; CELLS];
        for (i, c) in cost.iter_mut().enumerate() {
            if self.blocked[i] {
                *c = IMPASSABLE;
            } else if self.occupied[i] {
                *c = OCCUPIED;
            }
        }
        let (si, di) = (from.index(), to.index());
        // The destination's own occupant is cleared, as in the flood fill:
        // walking *onto* a comrade is the mover's problem, not the router's.
        if cost[di] == OCCUPIED {
            cost[di] = 0;
        } else if cost[di] > OCCUPIED {
            return None;
        }
        cost[si] = 1;

        // Two walkers, both starting at `from`, rotating opposite ways.
        let mut at = [si, si];
        let mut live = [true, true];
        for _ in 0..80 {
            if cost[di] != 0 {
                break;
            }
            if !live[0] && !live[1] {
                break;
            }
            for w in 0..2 {
                if !live[w] {
                    continue;
                }
                live[w] = false;
                let (cx, cy) = ((at[w] % DIM) as i32, (at[w] / DIM) as i32);
                let (tx, ty) = (to.x as i32, to.y as i32);
                if cx == tx && cy == ty {
                    return Some(cost);
                }
                let Some(seed) = crate::facing::facing_from_delta(tx - cx, ty - cy) else {
                    continue;
                };
                let mut dir = seed as i32;
                for _ in 0..8 {
                    let (dx, dy) = crate::facing::FACING_DELTA[dir as usize];
                    let (nx, ny) = (cx + dx, cy + dy);
                    if (0..DIM as i32).contains(&nx) && (0..DIM as i32).contains(&ny) {
                        let n = ny as usize * DIM + nx as usize;
                        if cost[n] == 0
                            && self.step_allowed(at[w], n)
                            && self.step_cost[n] == 0
                        {
                            cost[n] = cost[at[w]] + 1;
                            at[w] = n;
                            live[w] = true;
                            break;
                        }
                    }
                    dir = if w == 0 { (dir + 1) & 7 } else { (dir + 7) & 7 };
                }
            }
        }
        (cost[di] != 0).then_some(cost)
    }

    /// Straight line of sight, kept for the tests that state the rule directly.
    ///
    /// # A friendly body blocks the line, and this used to ignore them
    ///
    /// `Path_LineIsClear` (`0x004710F2`) opens by copying the blocked template
    /// over `g_pathCost` and calling **`Path_BuildBlockedMap`** — the routine
    /// that writes **998** into every cell a *friendly* figure is standing on —
    /// and then walks the line testing that same array.
    /// makes the line not clear
    /// through into the flood fill that will route around him.
    ///
    /// The destination is the one exception: the original clears a 998 there
    /// back to 0 before it starts, so *walking onto* an occupied cell is
    /// allowed and the mover sorts it out by swapping or waiting. Terrain at
    /// the destination (≥ 999) still refuses outright.
    ///
    /// > **This omission is the whole of *"848 men could not reach two
    /// > figures"*
    /// > step is taken by a comrade asks for a path; the path search sees a
    /// > clear line — because it was not looking at comrades — answers *no
    /// > search needed*, and hands back nothing. The figure then tries the same
    /// > blocked step again, for ever. One figure does this invisibly; an army
    /// > packed six deep in front of a gate does it as a **permanent
    /// > deadlock**, and no test shorter than the jam could see it. Measured:
    /// > 45 besiegers frozen in a block eight cells wide for 200,000 frames,
    /// > every one of them `Walking`, `barred` 0, with an empty path.
    /// > `docs/decisions.md` `C103`.
    pub fn line_is_clear(&self, from: Pos, to: Pos) -> bool {
        // Integer Bresenham. No floating point: a line that differs by one cell
        // between two machines is a desync.
        let (mut x, mut y) = (from.x as i32, from.y as i32);
        let (tx, ty) = (to.x as i32, to.y as i32);
        let (dx, dy) = ((tx - x).abs(), -(ty - y).abs());
        let (sx, sy) = (if x < tx { 1 } else { -1 }, if y < ty { 1 } else { -1 });
        let mut err = dx + dy;
        loop {
            if x == tx && y == ty {
                return true;
            }
            let prev = y as usize * DIM + x as usize;
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
            let idx = y as usize * DIM + x as usize;
            if self.blocked[idx] || !self.step_allowed(prev, idx) {
                return false;
            }
        }
    }
}

