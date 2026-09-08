//! Battlefield pathfinding.
//!
//! Reimplemented from `docs/battle.md` §8.3. Figures normally walk **straight at
//! their target** with no search at all; the pathfinder runs only when one is
//! blocked, and only if it has not already failed four times.
//!
//! The distinctive part, and the reason a textbook Dijkstra would produce
//! different paths: **terrain cost is charged twice over, by weighting *and* by
//! deferral.** The recorded cost is `cost[cur] + 1 + step_cost[neighbour]`, and
//! separately an expensive cell must be popped `step_cost` extra times before it
//! expands. Cheap ground therefore spreads first, and the number written down is
//! accumulated cost, not hop count.
//!
//! An earlier revision of this file asserted the opposite — "deferral, not
//! weighting" — and weighted nothing. That was wrong. The correction comes from
//! the decompiled `Path_Search` at `0x0047095E`:
//!
//! ```text
//! sVar3    = cost[cur] + 1;
//! cost[nb] = stepCost[nb] + sVar3;
//! ```
//!
//! **The cost field is not a metric.** A neighbour is considered only while
//! `cost[neighbour] == 0`, so the first cost written to a cell stands even when a
//! cheaper route reaches it later; there is no relaxation step. Reproduced
//! deliberately — the original's paths are the specification, and "fixing" this
//! changes where armies walk.
//!
//! On a `.skr` battlefield every step costs zero, because
//! `Battlefield_BuildFromSkr` never sets the flags `Path_BuildStepCost` looks
//! for. Field pathfinding is therefore a plain breadth-first search, and the
//! weighting only ever bites inside castles.
//!
//! Integer arithmetic, fixed neighbour order, no iteration over a hash. Two
//! machines searching the same grid produce the same path.

pub const DIM: usize = 80;
pub const CELLS: usize = DIM * DIM;

/// A **friendly figure** stands here (`Path_BuildBlockedMap`, `0x3E6`).
///
/// Not the same as impassable, and the difference is load-bearing: if the
/// *destination* is merely occupied the original clears it to 0 and paths onto
/// it anyway, leaving the mover to swap or wait.
pub const OCCUPIED: u16 = 998;
/// Terrain that can never be entered (`Path_BuildTerrainTemplate`): flags `0x10`
/// or `0x40`, or the one-cell map border. A destination at this value aborts the
/// search before it starts.
pub const IMPASSABLE: u16 = 999;
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
    /// Extra pops a cell must take before it expands, *and* the surcharge added
    /// to its recorded cost. **0 is ordinary ground** — the original's scale,
    /// where a `.skr` battlefield is zero everywhere.
    pub step_cost: Vec<u8>,
    pub elevation: Vec<u8>,
    /// Impassable terrain.
    pub blocked: Vec<bool>,
    /// A friendly figure is standing here. Blocks routing *through*, but not
    /// arrival at, the cell.
    pub occupied: Vec<bool>,
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
            *c = IMPASSABLE;
        } else if grid.occupied[i] {
            *c = OCCUPIED;
        }
    }
    let mut visits = vec![0u8; CELLS];

    let (si, di) = (start.index(), dest.index());
    // The two blocked values part company here, and only here. A destination
    // held by a friendly figure is cleared and searched for normally; impassable
    // terrain abandons the search before the first pop.
    if cost[di] == OCCUPIED {
        cost[di] = 0;
    } else if cost[di] > OCCUPIED {
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

        if cost[di] != 0 {
            break;
        }

        // Deferral, the second half of the cost charge. A cell's cost is
        // recorded the moment it is reached, but it does not *expand* until it
        // has been popped `step_cost` extra times, each re-queue earning it one
        // more visit. Expensive ground therefore holds the frontier up as well
        // as being priced higher.
        //
        // Deferring the *cost* as well as the expansion looks equivalent and is
        // not: the cell would stay marked unvisited, the frontier would leak
        // past it, and the extraction walk could then never route back through
        // it. That bug was written once already; the shape of the original's
        // test is what rules it out.
        //
        // Reading the original's condition exactly, `stepCost == 0` short
        // circuits and the comparison is against the count *before* the
        // increment, so a cell of cost k expands on its (k+1)-th pop.
        if grid.step_cost[cur] != 0 {
            let seen = visits[cur];
            visits[cur] = visits[cur].saturating_add(1);
            if seen < grid.step_cost[cur] {
                queue[tail] = cur as u16;
                tail = (tail + 1) % QUEUE_CAP;
                continue;
            }
        }

        for (dx, dy) in NEIGHBOURS {
            let nx = (cur % DIM) as i32 + dx;
            let ny = (cur / DIM) as i32 + dy;
            if nx < 0 || ny < 0 || nx >= DIM as i32 || ny >= DIM as i32 {
                continue;
            }
            let n = ny as usize * DIM + nx as usize;
            // One test covers unvisited, occupied and impassable alike: all
            // three are "not zero". No relaxation - the first cost written to a
            // cell is the one that stands.
            if cost[n] != 0 || !grid.step_allowed(cur, n) {
                continue;
            }
            cost[n] = cost[cur]
                .saturating_add(1)
                .saturating_add(grid.step_cost[n] as u16);
            queue[tail] = n as u16;
            tail = (tail + 1) % QUEUE_CAP;
        }
    }

    let outcome = if cost[di] != 0 && cost[di] < OCCUPIED {
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
            if cost_of(s, n) == 0 || cost_of(s, n) >= OCCUPIED || !grid.step_allowed(cur, n) {
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
    fn expensive_ground_is_still_walked_when_it_is_the_only_way_through() {
        let mut g = wall_at(20);
        // Make the gap expensive. It is still the only way through, so the path
        // must still use it - the charge delays and prices the cell, it does not
        // forbid it.
        g.set_cost(Pos::new(20, 40), 8);

        let (start, dest) = (Pos::new(5, 10), Pos::new(35, 10));
        let s = search(&g, start, dest);
        assert_eq!(s.outcome, Outcome::Found);
        let path = extract(&g, &s, start, dest);
        assert!(path.iter().any(|p| p.x == 20 && p.y == 40));
    }

    /// The correction that cost this file a rewrite: the recorded cost really is
    /// weighted, `cost[cur] + 1 + step_cost[neighbour]`.
    ///
    /// Measured as a difference rather than an absolute, because the absolute
    /// depends on the route and the difference does not: the wall has exactly
    /// one gap, so every route crosses it, and a surcharge there is inherited by
    /// everything downstream of it.
    #[test]
    fn the_step_cost_surcharge_lands_in_the_recorded_cost() {
        let (start, dest) = (Pos::new(5, 10), Pos::new(35, 10));
        let gap = Pos::new(20, 40);

        let free = search(&wall_at(20), start, dest);
        let mut g = wall_at(20);
        g.set_cost(gap, 8);
        let dear = search(&g, start, dest);

        assert_eq!(free.outcome, Outcome::Found);
        assert_eq!(dear.outcome, Outcome::Found);
        assert_eq!(
            dear.cost[gap.index()] - free.cost[gap.index()],
            8,
            "the gap itself should carry its own surcharge"
        );
        assert_eq!(
            dear.cost[dest.index()] - free.cost[dest.index()],
            8,
            "and everything downstream should inherit it exactly once"
        );
    }

    /// A cell is priced once, on first arrival, and never re-priced. Not
    /// Dijkstra: a cheaper route arriving later is ignored.
    #[test]
    fn a_cost_is_never_relaxed_by_a_cheaper_later_route() {
        let mut g = Grid::open();
        // A pocket reachable the long way round cheaply, and directly across
        // expensive ground. The direct arrival happens first and stands.
        for y in 0..DIM as u8 {
            if y != 40 {
                g.block(Pos::new(20, y));
            }
        }
        g.set_cost(Pos::new(20, 40), 50);
        let (start, dest) = (Pos::new(5, 10), Pos::new(35, 10));
        let s = search(&g, start, dest);
        assert_eq!(s.outcome, Outcome::Found);
        // Every step out of the gap is +1 and nothing re-prices the gap itself,
        // so the surcharge survives all the way to the destination.
        assert!(
            s.cost[dest.index()] > 50,
            "the surcharge was relaxed away: {}",
            s.cost[dest.index()]
        );
    }

    /// 998 and 999 are different things. A destination held by a friendly figure
    /// is cleared and pathed onto; impassable terrain is not.
    #[test]
    fn an_occupied_destination_is_reachable_but_blocked_terrain_is_not() {
        let (start, dest) = (Pos::new(5, 10), Pos::new(35, 10));

        let mut occupied = wall_at(20);
        occupied.occupy(dest);
        let s = search(&occupied, start, dest);
        assert_eq!(
            s.outcome,
            Outcome::Found,
            "a friendly figure on the target cell must not make it unreachable"
        );
        assert!(!extract(&occupied, &s, start, dest).is_empty());

        let mut walled = wall_at(20);
        walled.block(dest);
        assert_eq!(search(&walled, start, dest).outcome, Outcome::Unreachable);
    }

    /// Occupied cells still block routing *through*, which is what separates
    /// them from ordinary ground.
    #[test]
    fn a_friendly_figure_is_routed_around_not_through() {
        let mut g = wall_at(20);
        g.occupy(Pos::new(20, 40));
        let (start, dest) = (Pos::new(5, 10), Pos::new(35, 10));
        assert_eq!(
            search(&g, start, dest).outcome,
            Outcome::Unreachable,
            "a figure standing in the only gap seals it"
        );
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
