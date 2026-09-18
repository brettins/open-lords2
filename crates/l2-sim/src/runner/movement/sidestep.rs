//! **`BattleMan_Step`'s blocked arm** (`0x0048F1DD`, the whole of
//! `local_10 == 2 || local_10 == 3`): a man whose step was refused side-steps
//! (`FUN_004904EC`), and only if he cannot, asks the pathfinder — then may
//! give up on the route he is handed (`Path_DetourTooLong`, `0x00472227`).

#![allow(unused_imports)]
use crate::pathfind::{self, Outcome, Pos};
use crate::runner::*;
use crate::*;

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
    /// **Siege engines are excluded**: the `isSiegeEngine` arm of
    /// `BattleMan_Step` goes to `FUN_00491492` and `FUN_004912EC` instead, and
    /// [`Self::tower_step`] is that arm.
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
            self.fighters[i].facing = d;
            self.enter(i, Pos::new(nx as u8, ny as u8));
            return true;
        }
        false
    }

    /// Answer 1 of **`BattleMan_TryStepDir` (`0x00490616`)** — the only answer
    /// the side-step accepts (`FUN_004904EC`, `00490000.c:114,118`) — with no
    /// side effects: in bounds, empty, passable, within one level. It is the
    /// direction-to-cell wrapper around `Cell_TryEnter` and clamps at the map
    /// edge.
    ///
    /// **A castle cell is refused.** `Cell_TryEnter` answers 5 or 6 there for
    /// the attacker and 1 for the garrison on its own walls; neither 5 nor 6
    /// is a step. `[D]`: it only narrows where a man may shuffle.
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

    pub(crate) fn request_path(&mut self, i: usize) {
        self.request_path_with(i, true)
    }

    /// As above, with the side-step withheld when `may_side_step` is false. No
    /// caller passes `false` now that the swap arm's refusal is the wait
    /// (`00480000.c:6459-6463`).
    pub(crate) fn request_path_with(&mut self, i: usize, may_side_step: bool) {
        // **`onRoute = 0`, at the head of the arm.** `BattleMan_Step`
        // (`0x0048F1DD`) clears it as the first statement under `local_10 == 2
        // || local_10 == 3`, before `FUN_004904EC`, and again before
        // `BattleMen_SwapPlaces` in the friendly arm. **[V]**, decompiled.
        self.fighters[i].path.clear();
        // **The side-step comes first** — before the cooldown, before `barred`,
        // before any search: `DAT_00554474 = FUN_004904EC(); if (DAT_00554474
        // != 8) local_10 = 1;`, and `local_10 == 1` is the tail that moves him.
        if may_side_step && self.side_step(i) {
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
        //
        // `Outcome::NoSearchNeeded` returns an empty cost field, and the
        // original's skipped search leaves `g_pathCost` holding the last
        // search's. Reading 0 there is "not too long", the only reading that
        // does not depend on another figure's leftovers. `[D]`.
        let cost = search.cost.get(dest.y as usize * DIM + dest.x as usize).copied().unwrap_or(0);
        let straight = chebyshev(f.x as i16, f.y as i16, dest.x as i16, dest.y as i16);
        if crate::movement::detour_too_long(human, is_siege, f.side, cost, straight) {
            // `00480000.c:6525-6529`: `tgX = mapX; tgY = mapY; return 0` —
            // ahead of the tail, so no `state = 1` and no `delay`; the
            // `holdIt = 64` above is what holds him. **[V]**.
            f.target = (f.x, f.y);
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
            // `BattleMan_Step` runs `Path_Extract` on it whether or not the
            // flood fill ran. [`pathfind::Grid::walk_line`] is that walk, out
            // of `0x004710F2`. Applied only here.
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
        // **`00480000.c:6583-6586`, the tail of the blocked arm**: `local_10`
        // still 2 or 3 — `FUN_004904EC` answered 8 and no route came back — is
        // `delayState = state; state = 1; delay = 100`. **[V]**, decompiled.
        //
        // The two early returns above — `hold > 0`, `barred >= 4` — are left
        // out: they stand for `holdIt`'s own cooldown, which the original
        // reloads on the way through this arm rather than at its tail. `[D]`.
        if self.fighters[i].path.is_empty() {
            self.fighters[i].delay = PARK;
        }
    }
}

/// `delay = 100`, `00480000.c:6585`.
const PARK: u8 = 100;
