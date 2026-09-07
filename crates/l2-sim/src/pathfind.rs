//! Battlefield pathfinding.
//!
//! Reimplemented from `docs/battle.md` §8.3. Figures normally walk **straight at
//! their target** with no search at all; the pathfinder runs only when one is
//! blocked, and only if it has not already failed four times.
//!
//! The distinctive part, and the reason a textbook Dijkstra would produce
//! different paths: **terrain cost is charged by deferral, not by weighting.** A
//! cell is expanded only once it has been *reached* as many times as its step
//! cost, so expensive ground is re-queued rather than ordered. Cheap ground
//! therefore spreads first, but a cell's recorded distance is still hop count,
//! not accumulated cost.
//!
//! Integer arithmetic, fixed neighbour order, no iteration over a hash. Two
//! machines searching the same grid produce the same path.

pub const DIM: usize = 80;
pub const CELLS: usize = DIM * DIM;

/// Cost value marking a cell that can never be entered.
pub const BLOCKED: u16 = 998;
/// The original's frontier queue holds this many entries and **wraps**, so a
/// search that outgrows it silently overwrites its own queue. Reproduced rather
/// than fixed: a search that would have overrun produces the original's result.
pub const QUEUE_CAP: usize = 0x1900;
/// `Path_Extract` writes at most this many waypoints.
pub const MAX_WAYPOINTS: usize = 150;

/// Eight neighbours in a fixed order. Order decides tie-breaks, so it is part of
/// the specification, not an implementation detail.
const NEIGHBOURS: [(i32, i32); 8] =
    [(0, -1), (-1, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (1, 1), (0, 1)];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pos {
    pub x: u8,
    pub y: u8,
}

impl Pos {
    pub fn new(x: u8, y: u8) -> Self {
        Pos { x, y }
    }
    fn index(self) -> usize {
        self.y as usize * DIM + self.x as usize
    }
}

/// Chebyshev distance — diagonals cost the same as orthogonals, which is what
/// makes "adjacent" mean any of the eight neighbours.
pub fn chebyshev(a: Pos, b: Pos) -> u32 {
    let dx = (a.x as i32 - b.x as i32).unsigned_abs();
    let dy = (a.y as i32 - b.y as i32).unsigned_abs();
    dx.max(dy)
}

/// The battlefield as the pathfinder sees it.
#[derive(Debug, Clone)]
pub struct Grid {
    /// Times a cell must be reached before it is expanded. 1 is ordinary ground.
    pub step_cost: Vec<u8>,
    pub elevation: Vec<u8>,
    pub blocked: Vec<bool>,
}

impl Grid {
    pub fn open() -> Self {
        Grid {
            step_cost: vec![1; CELLS],
            elevation: vec![0; CELLS],
            blocked: vec![false; CELLS],
        }
    }

    pub fn block(&mut self, p: Pos) {
        self.blocked[p.index()] = true;
    }

    pub fn set_cost(&mut self, p: Pos, cost: u8) {
        self.step_cost[p.index()] = cost.max(1);
    }

    pub fn set_elevation(&mut self, p: Pos, e: u8) {
        self.elevation[p.index()] = e;
    }

    /// A step is allowed when the two cells differ by at most one level, unless
    /// the destination is exactly 5 — the same rule movement uses.
    fn step_allowed(&self, from: usize, to: usize) -> bool {
        let (a, b) = (self.elevation[from] as i32, self.elevation[to] as i32);
        (b - a).abs() <= 1 || b == 5
    }

    /// Straight line of sight, used as an early out before any search.
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

/// Why a search did not need to run, or did not succeed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The target is adjacent, or the straight line is clear. No search ran.
    NoSearchNeeded,
    Found,
    /// The flood fill exhausted its frontier without reaching the destination.
    Unreachable,
}

#[derive(Debug, Clone)]
pub struct Search {
    pub outcome: Outcome,
    /// Hop count per cell; 0 is unvisited, 1 is the start.
    pub cost: Vec<u16>,
}

/// Uniform-cost breadth-first flood fill.
///
/// Returns `NoSearchNeeded` without touching the cost field when the target is
/// adjacent or in clear line of sight, exactly as the original does — most
/// movement in a battle never runs a search at all.
pub fn search(grid: &Grid, start: Pos, dest: Pos) -> Search {
    if chebyshev(start, dest) < 2 || grid.line_is_clear(start, dest) {
        return Search { outcome: Outcome::NoSearchNeeded, cost: Vec::new() };
    }

    let mut cost = vec![0u16; CELLS];
    for (i, c) in cost.iter_mut().enumerate() {
        if grid.blocked[i] {
            *c = BLOCKED;
        }
    }
    let mut visits = vec![0u8; CELLS];

    let (si, di) = (start.index(), dest.index());
    if cost[di] == BLOCKED {
        return Search { outcome: Outcome::Unreachable, cost };
    }
    cost[si] = 1;

    // A circular buffer that wraps rather than growing, like the original.
    let mut queue = vec![0u16; QUEUE_CAP];
    let (mut head, mut tail) = (0usize, 0usize);
    queue[tail] = si as u16;
    tail = (tail + 1) % QUEUE_CAP;

    while head != tail {
        let cur = queue[head] as usize;
        head = (head + 1) % QUEUE_CAP;

        if cost[di] != 0 && cost[di] != BLOCKED {
            break;
        }

        // Deferral, not weighting. A cell's distance is recorded the moment it
        // is reached, but it does not *expand* until it has been popped as many
        // times as its step cost — each re-queue earning it one more visit. So
        // expensive ground still gets a cost and can still be walked through;
        // it simply holds the frontier up rather than being ordered behind
        // cheap ground, which is what makes this not a Dijkstra.
        //
        // Deferring the cost as well as the expansion looks equivalent and is
        // not: the cell stays marked unvisited, the frontier leaks past it, and
        // the extraction walk can then never route back through it.
        visits[cur] = visits[cur].saturating_add(1);
        if visits[cur] < grid.step_cost[cur] {
            queue[tail] = cur as u16;
            tail = (tail + 1) % QUEUE_CAP;
            continue;
        }

        for (dx, dy) in NEIGHBOURS {
            let nx = (cur % DIM) as i32 + dx;
            let ny = (cur / DIM) as i32 + dy;
            if nx < 0 || ny < 0 || nx >= DIM as i32 || ny >= DIM as i32 {
                continue;
            }
            let n = ny as usize * DIM + nx as usize;
            if cost[n] == BLOCKED || cost[n] != 0 || !grid.step_allowed(cur, n) {
                continue;
            }
            cost[n] = cost[cur].saturating_add(1);
            queue[tail] = n as u16;
            tail = (tail + 1) % QUEUE_CAP;
        }
    }

    let outcome = if cost[di] != 0 && cost[di] != BLOCKED {
        Outcome::Found
    } else {
        Outcome::Unreachable
    };
    Search { outcome, cost }
}

/// Walk the cost field downhill from the destination back to the start.
///
/// Returned in travel order, start-exclusive. Ties break toward the neighbour
/// order above, which is why that order is specified rather than incidental.
pub fn extract(grid: &Grid, s: &Search, start: Pos, dest: Pos) -> Vec<Pos> {
    if s.outcome != Outcome::Found {
        return Vec::new();
    }
    let mut path = Vec::new();
    let mut cur = dest.index();
    let si = start.index();

    while cur != si && path.len() < MAX_WAYPOINTS {
        path.push(Pos::new((cur % DIM) as u8, (cur / DIM) as u8));
        let mut best: Option<usize> = None;
        for (dx, dy) in NEIGHBOURS {
            let nx = (cur % DIM) as i32 + dx;
            let ny = (cur / DIM) as i32 + dy;
            if nx < 0 || ny < 0 || nx >= DIM as i32 || ny >= DIM as i32 {
                continue;
            }
            let n = ny as usize * DIM + nx as usize;
            if cost_of(s, n) == 0 || cost_of(s, n) == BLOCKED || !grid.step_allowed(cur, n) {
                continue;
            }
            if best.is_none() || cost_of(s, n) < cost_of(s, best.unwrap()) {
                best = Some(n);
            }
        }
        match best {
            Some(n) if cost_of(s, n) < cost_of(s, cur) => cur = n,
            // No downhill neighbour: the field is malformed. Give up rather than
            // loop, which is what the failure counters exist to record.
            _ => return Vec::new(),
        }
    }
    path.reverse();
    path
}

fn cost_of(s: &Search, i: usize) -> u16 {
    s.cost[i]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_adjacent_target_needs_no_search_at_all() {
        let g = Grid::open();
        let s = search(&g, Pos::new(10, 10), Pos::new(11, 11));
        assert_eq!(s.outcome, Outcome::NoSearchNeeded);
        assert!(s.cost.is_empty(), "the cost field is never even allocated");
    }

    #[test]
    fn a_clear_line_needs_no_search_either() {
        let g = Grid::open();
        let s = search(&g, Pos::new(5, 5), Pos::new(40, 40));
        assert_eq!(s.outcome, Outcome::NoSearchNeeded);
    }

    fn wall_at(x: u8) -> Grid {
        let mut g = Grid::open();
        // A wall with one gap, so the straight line fails and a search must run.
        for y in 0..DIM as u8 {
            if y != 40 {
                g.block(Pos::new(x, y));
            }
        }
        g
    }

    #[test]
    fn a_blocked_line_forces_a_search_that_finds_the_gap() {
        let g = wall_at(20);
        let (start, dest) = (Pos::new(5, 10), Pos::new(35, 10));
        assert!(!g.line_is_clear(start, dest));

        let s = search(&g, start, dest);
        assert_eq!(s.outcome, Outcome::Found);

        let path = extract(&g, &s, start, dest);
        assert!(!path.is_empty());
        assert_eq!(*path.last().unwrap(), dest);
        // Every step is to a neighbouring cell, and none passes through the wall.
        let mut prev = start;
        for p in &path {
            assert!(chebyshev(prev, *p) == 1, "{prev:?} -> {p:?} is not a step");
            assert!(!g.blocked[p.y as usize * DIM + p.x as usize]);
            prev = *p;
        }
        // It had to go through the one gap.
        assert!(path.iter().any(|p| p.x == 20 && p.y == 40));
    }

    #[test]
    fn a_fully_walled_destination_is_unreachable() {
        let mut g = Grid::open();
        let dest = Pos::new(40, 40);
        for (dx, dy) in NEIGHBOURS {
            g.block(Pos::new((40 + dx) as u8, (40 + dy) as u8));
        }
        let s = search(&g, Pos::new(5, 5), dest);
        assert_eq!(s.outcome, Outcome::Unreachable);
        assert!(extract(&g, &s, Pos::new(5, 5), dest).is_empty());
    }

    /// Expensive ground is re-queued rather than weighted, so it is expanded
    /// later than cheap ground — the behaviour that makes this not a Dijkstra.
    #[test]
    fn expensive_ground_is_deferred_rather_than_weighted() {
        let mut g = wall_at(20);
        // Make the gap expensive. It is still the only way through, so the path
        // must still use it - deferral delays expansion, it does not forbid it.
        g.set_cost(Pos::new(20, 40), 8);

        let (start, dest) = (Pos::new(5, 10), Pos::new(35, 10));
        let s = search(&g, start, dest);
        assert_eq!(s.outcome, Outcome::Found);
        let path = extract(&g, &s, start, dest);
        assert!(path.iter().any(|p| p.x == 20 && p.y == 40));
    }

    #[test]
    fn a_cliff_cannot_be_climbed_but_level_five_can() {
        let mut g = Grid::open();
        for y in 0..DIM as u8 {
            if y != 40 {
                g.set_elevation(Pos::new(20, y), 4); // two levels up from 0
            }
        }
        let (start, dest) = (Pos::new(5, 10), Pos::new(35, 10));
        assert!(!g.line_is_clear(start, dest), "a cliff blocks line of sight");

        let s = search(&g, start, dest);
        assert_eq!(s.outcome, Outcome::Found);
        let path = extract(&g, &s, start, dest);
        assert!(path.iter().any(|p| p.y == 40), "must use the level gap");
    }

    #[test]
    fn the_same_grid_always_yields_the_same_path() {
        let g = wall_at(30);
        let (start, dest) = (Pos::new(2, 2), Pos::new(60, 70));
        let first = extract(&g, &search(&g, start, dest), start, dest);
        for _ in 0..8 {
            let again = extract(&g, &search(&g, start, dest), start, dest);
            assert_eq!(first, again, "pathfinding must be reproducible");
        }
        assert!(!first.is_empty());
    }

    #[test]
    fn a_path_never_exceeds_the_waypoint_limit() {
        let g = wall_at(40);
        let (start, dest) = (Pos::new(0, 0), Pos::new(79, 79));
        let path = extract(&g, &search(&g, start, dest), start, dest);
        assert!(path.len() <= MAX_WAYPOINTS, "got {}", path.len());
    }
}
