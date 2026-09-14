//! **`BattleMan_Step`'s blocked arm** (`0x0048F1DD`, the whole of
//! `local_10 == 2 || local_10 == 3`): a man whose step was refused side-steps
//! (`FUN_004904EC`), and only if he cannot, asks the pathfinder — and then may
//! give up on the route he is handed (`Path_DetourTooLong`).
//!
//! `docs/battle.md` §8.3a.

#![allow(unused_imports)]
use crate::pathfind::{self, Outcome, Pos};
use crate::runner::*;
use crate::*;

/// The side-step and the give-up, tested where they are written.
#[cfg(test)]
#[path = "tests_sidestep.rs"]
mod tests_sidestep;

impl BattleRunner {
    /// **`FUN_004904EC` (`0x004904EC`) — a blocked man side-steps.** He
    /// re-derives the direction to his target and tries it and its rotations,
    /// five each way ([`crate::movement::side_step_order`]), taking the first
    /// cell `Cell_TryEnter` would let him into. `true` means he moved and no
    /// search happens.
    ///
    /// Guarded by [`crate::movement::SIDE_STEP_RANGE`], so it only fires within
    /// one cell of the destination — which is what makes it a shuffle for a
    /// slot.
    ///
    /// **Siege engines are excluded**, as they are in the original: the
    /// `isSiegeEngine` arm of `BattleMan_Step` goes to `FUN_00491492` and
    /// `FUN_004912EC` instead, and [`Self::tower_step`] is that arm.
    fn side_step(&mut self, i: usize) -> bool {
        let (x, y, target, troop) = {
            let f = &self.fighters[i];
            (f.x, f.y, f.target, f.troop)
        };
        if troop.is_siege() {
            return false;
        }
        if chebyshev(x as i16, y as i16, target.0 as i16, target.1 as i16)
            >= crate::movement::SIDE_STEP_RANGE
        {
            return false;
        }
        let Some(dir) = facing_from_delta(target.0 as i32 - x as i32, target.1 as i32 - y as i32)
        else {
            return false;
        };
        for d in crate::movement::side_step_order(dir) {
            let (dx, dy) = FACING_DELTA[d as usize];
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            if !(0..DIM as i32).contains(&nx) || !(0..DIM as i32).contains(&ny) {
                continue;
            }
            if !self.side_step_cell_is_free(i, nx as usize, ny as usize) {
                continue;
            }
            // `local_10 == 1`'s tail: `dirc = dir` and then the move.
            self.fighters[i].facing = d;
            self.enter(i, Pos::new(nx as u8, ny as u8));
            return true;
        }
        false
    }

    /// `Cell_TryEnter`'s answer 1 — the only answer the side-step accepts —
    /// reduced to a test with no side effects: in bounds, empty, passable, and
    /// within one level.
    ///
    /// **A castle cell is refused.** `Cell_TryEnter` answers 5 or 6 there for
    /// the attacker and 1 for the garrison on its own walls; neither 5 nor 6 is
    /// a step. `[D]`: it only narrows where a man may shuffle.
    fn side_step_cell_is_free(&self, i: usize, nx: usize, ny: usize) -> bool {
        let dst = ny * DIM + nx;
        if self.blocked[dst] || self.occupant[dst].is_some() {
            return false;
        }
        let src = self.fighters[i].y as usize * DIM + self.fighters[i].x as usize;
        if !crate::movement::can_step_elevation(
            self.field.cells[src].elevation as i32,
            self.field.cells[dst].elevation as i32,
        ) {
            return false;
        }
        let castle =
            crate::siege::FLAG_WALL | crate::siege::FLAG_DRAWBRIDGE | crate::siege::FLAG_KEEP;
        !(self.siege.is_siege && self.field.cells[dst].flags & castle != 0)
    }

    /// Ask [`crate::pathfind`] for a route, subject to the original's two
    /// throttles: a cooldown after each attempt, and a hard stop after four
    /// consecutive failures.
    pub(crate) fn request_path(&mut self, i: usize) {
        // **The side-step comes first** — before the cooldown, before `barred`,
        // before any search: `DAT_00554474 = FUN_004904EC(); if (DAT_00554474
        // != 8) local_10 = 1;`, and `local_10 == 1` is the tail that moves him.
        if self.side_step(i) {
            return;
        }
        {
            let f = &self.fighters[i];
            if f.hold > 0 || f.barred >= 4 {
                return;
            }
        }
        let (start, dest, side) = {
            let f = &self.fighters[i];
            (f.pos(), Pos::new(f.target.0, f.target.1), f.side)
        };

        let mut grid = Grid::open();
        for (c, blocked) in self.blocked.iter().enumerate() {
            if *blocked {
                grid.blocked[c] = true;
            }
        }
        // Only *friendly* figures are marked. Enemies are deliberately left
        // out: the original routes straight through them and leaves contact to
        // the mover.
        for (c, occ) in self.occupant.iter().enumerate() {
            if let Some(o) = occ {
                if self.fighters[*o as usize].side == side && *o as usize != i {
                    grid.occupied[c] = true;
                }
            }
        }

        let search = pathfind::search(&grid, start, dest);
        let is_siege = self.siege.is_siege;
        let human = self.sim.figures[self.fighters[i].sim].owner_is_human;
        let f = &mut self.fighters[i];
        f.hold = 64;
        f.reroutes = f.reroutes.saturating_add(1);
        // **`Path_DetourTooLong` (`0x00472227`), where the original asks it**:
        // after the search and the `holdIt` reload, before `Path_Extract`. A
        // route over the cap and over five times the straight line is
        // abandoned — `tgX, tgY = mapX, mapY` — and the figure stands. The two
        // guards that make this the player's rule only are in
        // [`crate::movement::detour_too_long`].
        //
        // `Outcome::NoSearchNeeded` returns an empty field, and the original's
        // skipped search leaves `g_pathCost` holding whatever the last one
        // wrote. Reading 0 there is "not too long", which is the only reading
        // that does not depend on another figure's leftovers. `[D]`.
        let cost = search.cost.get(dest.y as usize * DIM + dest.x as usize).copied().unwrap_or(0);
        let straight = chebyshev(f.x as i16, f.y as i16, dest.x as i16, dest.y as i16);
        if crate::movement::detour_too_long(human, is_siege, f.side, cost, straight) {
            f.target = (f.x, f.y);
            // The original cleared `onRoute` at the top of this arm; ours has
            // no such flag, so the stale waypoints go here — without it the
            // figure keeps walking the route it just gave up on.
            f.path.clear();
            return;
        }
        match search.outcome {
            Outcome::Found => {
                let mut path = pathfind::extract(&grid, &search, start, dest);
                path.reverse(); // consumed from the end
                if path.is_empty() {
                    f.barred = f.barred.saturating_add(1);
                } else {
                    f.path = path;
                    f.barred = 0;
                }
            }
            // **The line is clear and the figure still could not move**,
            // means a comrade is standing in the one cell it wanted. This arm
            // used to do nothing at all, and *nothing* is a deadlock: the
            // figure retries the same taken step, frame after frame, with
            // `barred` at 0 and an empty path, and nothing anywhere times it
            // out. One figure does that invisibly. An army pressing a breach
            // does it as a permanent jam — measured at 45 besiegers frozen in a
            // block eight cells wide for 200,000 frames, every one `Walking`.
            //
            // The original has no such hole, because **`Path_LineIsClear` is
            // not a predicate**: it seeds `g_pathCost` through
            // `Path_BuildBlockedMap` — which marks friendly figures 998 — walks
            // two greedy walkers that *rotate around* whatever is in the way,
            // and **leaves the cost field behind**. `BattleMan_Step` then runs
            // `Path_Extract` on it whether or not the flood fill ran, so the
            // figure comes away with the walked route, comrade-avoiding
            // detours and all. [`pathfind::Grid::walk_line`] is that walk, read
            // out of `0x004710F2`.
            //
            // It is applied **only here** — where the straight line is clear
            // and the step was refused anyway —
            // position in which the two readings differ. A figure that is not
            // blocked never asks for a path at all.
            Outcome::NoSearchNeeded => {
                if let Some(cost) = grid.walk_line(start, dest) {
                    let walked = pathfind::Search { outcome: Outcome::Found, cost };
                    let mut path = pathfind::extract(&grid, &walked, start, dest);
                    path.reverse();
                    let f = &mut self.fighters[i];
                    if !path.is_empty() {
                        f.path = path;
                        f.barred = 0;
                    }
                }
            }
            Outcome::Unreachable => f.barred = f.barred.saturating_add(1),
        }
    }
}
