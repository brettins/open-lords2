#![allow(unused_imports)]
use super::*;
use super::grid::*;
use super::tests_part::*;


/// A single search with fresh scratch space.
///
/// Convenient, and **not** what the original does across a battle: a fresh
/// [`Scratch`] has every counter zero, which is only true of the very first
/// search. Use [`search_with`] and keep one `Scratch` for the whole battle to
/// reproduce the original's behaviour.
pub fn search(grid: &Grid, start: Pos, dest: Pos) -> Search {
    search_with(&mut Scratch::new(), grid, start, dest)
}

/// Weighted breadth-first flood fill, carrying the original's scratch space.
///
/// Returns `NoSearchNeeded` without touching the cost field when the target is
/// adjacent or in clear line of sight — most
/// movement in a battle never runs a search at all.
pub fn search_with(scratch: &mut Scratch, grid: &Grid, start: Pos, dest: Pos) -> Search {
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
    scratch.begin();
    let visits = &mut scratch.visits;

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

// A circular buffer that wraps, like the original.
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
        // past it
        // it. That bug was written once already; the shape of the original's
        // test is what rules it out.
        //
        // Reading the original's condition exactly, `stepCost == 0` short
        // circuits and the comparison is against the count *before* the
        // increment
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
/// order above, so that order is specified.
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
// No downhill neighbour: the field is malformed. Give up
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

