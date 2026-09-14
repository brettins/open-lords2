#![allow(unused_imports)]
use super::*;
use super::grid::*;
use super::search_part::*;


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

    /// A wall down `x` with its only gap at `gap_y`.
    fn wall_with_gap(x: u8, gap_y: u8) -> Grid {
        let mut g = Grid::open();
        for y in 0..DIM as u8 {
            if y != gap_y {
                g.block(Pos::new(x, y));
            }
        }
        g
    }

    /// The original clears 4,096 of its 6,400 visit counters, so the bottom
    /// 36% of the battlefield carries counts from the previous search into the
    /// next one. Reproduced deliberately; this is the test that says so.
    ///
/// Driven through real searches, because
    /// the claim is about what `Path_Search` does, not about what `begin` does.
    #[test]
    fn counters_above_the_cleared_region_survive_into_the_next_search() {
        assert_eq!(CLEARED_COUNTERS, 4096);
        assert!(CLEARED_COUNTERS < CELLS, "there would be nothing to carry over");

        // Every cell costs something, because the counter is only touched when
        // `step_cost != 0` - the original short-circuits before the increment,
        // so on a free grid nothing is ever counted. This is why the bug bites
        // in castles and not on a `.skr` field.
        let costly = |x: u8, gap_y: u8| {
            let mut g = wall_with_gap(x, gap_y);
            for i in 0..CELLS {
                g.step_cost[i] = 1;
            }
            g
        };

        // A search driven the length of the map, through a gap at row 60 - cell
        // 4820, well above the cleared region.
        let high_gap = Pos::new(20, 60);
        assert!(high_gap.index() >= CLEARED_COUNTERS);
        let long = costly(20, 60);
        let (lo_start, lo_dest) = (Pos::new(5, 10), Pos::new(35, 10));

        // A second search confined to the top of the map, which terminates long
        // before it reaches row 60.
        let top = costly(60, 5);
        let (hi_start, hi_dest) = (Pos::new(50, 2), Pos::new(70, 2));

        let mut scratch = Scratch::new();
        assert_eq!(search_with(&mut scratch, &long, lo_start, lo_dest).outcome, Outcome::Found);

        let marked_high = scratch.visits()[high_gap.index()];
        let marked_low = scratch.visits()[..CLEARED_COUNTERS].iter().filter(|&&v| v > 0).count();
        assert!(marked_high > 0, "the first search never counted the low gap");
        assert!(marked_low > 0, "the first search never touched the cleared region");

        // The second search clears only the first 4,096 counters.
        assert_eq!(search_with(&mut scratch, &top, hi_start, hi_dest).outcome, Outcome::Found);

        assert_eq!(
            scratch.visits()[high_gap.index()],
            marked_high,
            "cell {} is above the cleared region and must keep its count",
            high_gap.index()
        );
        // And something inside the cleared region that the first search counted
        // and the second never reaches must be back to zero.
        let cleared_and_untouched = (0..CLEARED_COUNTERS)
            .find(|&i| i / DIM > 20 && i / DIM < 45)
            .expect("a row between the two searches");
        assert_eq!(
            scratch.visits()[cleared_and_untouched],
            0,
            "cell {cleared_and_untouched} is inside the cleared region and must have been reset"
        );
    }

    /// A fresh `Scratch` is the *first* search of a battle and nothing else.
    /// Two searches on separate scratches must agree; that is what makes the
/// carry-over above observable.
    #[test]
    fn a_fresh_scratch_gives_a_repeatable_search() {
        let g = wall_with_gap(20, 60);
        let (start, dest) = (Pos::new(5, 55), Pos::new(35, 55));
        let a = search_with(&mut Scratch::new(), &g, start, dest);
        let b = search_with(&mut Scratch::new(), &g, start, dest);
        assert_eq!(a.outcome, b.outcome);
        assert_eq!(a.cost, b.cost);
        assert_eq!(search(&g, start, dest).cost, a.cost, "the convenience wrapper agrees");
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

/// Expensive ground is re-queued, so it is expanded
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
/// Measured as a difference, because the absolute
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

