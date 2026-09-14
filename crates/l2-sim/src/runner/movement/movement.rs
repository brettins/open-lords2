#![allow(unused_imports)]
use super::*;
use super::pathfinding::*;
use super::*;

impl BattleRunner {
    /// The next cell to try: the stored path if the figure is on one, else
    /// straight at the target. Figures normally walk straight and never search
    /// at all — the pathfinder is what happens when that fails.
    pub(super) fn next_step(&self, i: usize) -> Option<Pos> {
        let f = &self.fighters[i];
        if let Some(&wp) = f.path.last() {
            return Some(wp);
        }
        let dx = f.target.0 as i32 - f.x as i32;
        let dy = f.target.1 as i32 - f.y as i32;
        let facing = facing_from_delta(dx, dy)?;
        let (sx, sy) = FACING_DELTA[facing as usize];
        let nx = f.x as i32 + sx;
        let ny = f.y as i32 + sy;
        if !(0..DIM as i32).contains(&nx) || !(0..DIM as i32).contains(&ny) {
            return None;
        }
        Some(Pos::new(nx as u8, ny as u8))
    }

    /// Ask [`crate::pathfind`] for a route, subject to the original's two
    /// throttles: a cooldown after each attempt, and a hard stop after four
    /// consecutive failures.
    pub(super) fn request_path(&mut self, i: usize) {
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
        let f = &mut self.fighters[i];
        f.hold = 64;
        f.reroutes = f.reroutes.saturating_add(1);
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

    // -- fire, oil and the tower --------------------------------------------

    /// `g_battleSizeClass` — see [`crate::fire::burn`], its one reader here.
    pub fn battle_size_class(&self) -> u8 {
        self.size_class
    }

    /// Re-derive one cell's copies — the blocked map and the AI's surfaces —
    /// after a fire, a pour or a dock has written it. The original's order
    /// handlers read the live array, so ours have to see a burning cell the
    /// frame it burns.
    pub(super) fn sync_cell(&mut self, c: usize) {
        let cell = self.field.cells[c];
        self.blocked[c] = cell.impassable();
        self.ai_field.surface[c] = cell.surface;
        self.ai_field.elevation[c] = cell.elevation;
    }

    /// **`Battle_UpdateAllMen`'s head, for one figure** — what the sweep does
    /// before it dispatches the troop's tick.
    ///
    /// ```c
    /// if (surface == 10)   BattleMan_BurnTick(man);
    /// if (surface == 0x11) BattleMan_BurnTick(man);
    /// if (surface == 0x0F && man.ownerIsHuman) DAT_005530E8++;
    /// ```
    ///
    /// The original counts every figure with an owner, a corpse on its eighty
    /// frames included; ours has no corpse lifetime and counts the living.
    /// `[D]`.
    pub(super) fn update_man(&mut self, i: usize) {
        let (sim, troop, side, cell) = {
            let f = &self.fighters[i];
            (f.sim, f.troop, f.side, f.y as usize * DIM + f.x as usize)
        };
        let surface = self.field.cells[cell].surface;
        if surface == fire::SURFACE_BURNING || surface == fire::SURFACE_WOOD_BURNING {
            let class = self.size_class;
            if fire::burn(&mut self.sim.figures[sim], class, fire::is_engine(troop)) {
                self.sim.cues.burn_death(side);
            }
        }
        if surface == fire::SURFACE_WOODLAND
            && self.sim.figures[sim].owner_is_human
            && self.sim.figures[sim].is_alive()
        {
            self.humans_in_woods += 1;
        }
    }

    /// `Missile_UpdateAll`'s class-7 arm — the cross a stream of oil burns
    /// under itself. See [`fire::OIL_CROSS`].
    pub(super) fn oil_cross(&mut self, slot: usize) {
        let m = *self.missiles.get(slot);
        let (x, y) = (m.cell_x as i32, m.cell_y as i32);
        let base = m.ticks_flown.wrapping_sub(1).wrapping_mul(0x20);
        for (dx, dy, bias) in fire::OIL_CROSS {
            let (cx, cy) = (x + dx, y + dy);
            if !(0..DIM as i32).contains(&cx) || !(0..DIM as i32).contains(&cy) {
                continue;
            }
            let s = self.field.cells[cy as usize * DIM + cx as usize].surface;
            if s == fire::SURFACE_BRIDGE || s == fire::SURFACE_BURNING {
                continue;
            }
            let c = fire::ignite(&mut self.field, &mut self.missiles, cx, cy, base.wrapping_add(bias));
            self.sync_cell(c);
        }
    }

    /// **Pour a pot of oil at a cell** — `FUN_0047A814` (`0x0047A814`).
    ///
    /// ```c
    /// Missile_Spawn(pot.owner, pot.mapX, pot.mapY, x, y);  …class 7…
    /// for (i = 0; i < 4; i++) Missile_Step();
    /// pot.state = 2;
    /// pot.polarDirc = pot.dirc = the longer axis toward (x, y);
    /// FUN_004262CF(3);                                     /* pouroil.wav */
    /// ```
    ///
    /// **The pot is spent**: state 2, the corpse state. It pours once and is a
/// casualty of its own pour,
    /// pots and not in men. With a hundred records in flight the spawn fails
    /// and the original runs its four steps on a record past the array; here
    ///
    /// as those writes are unconditional.
    ///
    /// The original's own loop in `BattleUnit_Order` walks figures by owner,
    /// so a pot that poured less than eighty frames ago — still a corpse with
    /// an owner — would pour **again** if its unit were re-ordered downhill.
    /// This crate has no corpse lifetime and only the living pour. `[D]`.
    pub(super) fn pour_oil(&mut self, pot: usize, to: (u8, u8)) {
        let sim = self.fighters[pot].sim;
        let from = (self.fighters[pot].x, self.fighters[pot].y);
        let owner = self.sim.figures[sim].owner;
        if let Some(slot) = self.missiles.alloc() {
            *self.missiles.get_mut(slot) = fire::oil_record(owner, sim, from, to);
            for _ in 0..fire::OIL_LAUNCH_STEPS {
                if !self.step_missile(slot) {
                    break;
                }
            }
        }
        {
            let f = &mut self.sim.figures[sim];
            f.state = State::Dead;
            f.opponent = None;
        }
        let facing = fire::pour_facing(from, to);
        self.fighters[pot].polar = facing;
        self.fighters[pot].facing = facing;
        self.sim.cues.oil_pour();
    }

    /// **`BattleUnit_Order`'s oil loop** — every living pot of `unit` pours at
    /// `(x, y)` when [`fire::order_pours`] says the order is downhill.
    pub(super) fn pour_on_order(&mut self, unit: usize, x: i16, y: i16) {
        if unit == 0 || unit > MAX_UNITS || !self.units.get(unit).is_live() {
            return;
        }
        let at = (x.clamp(0, DIM as i16 - 1) as u8, y.clamp(0, DIM as i16 - 1) as u8);
        let dest = self.field.at(at.0 as usize, at.1 as usize).surface;
        for i in 0..self.fighters.len() {
            let sim = self.fighters[i].sim;
            if self.fighters[i].troop != Troop::Oil
                || self.sim.figures[sim].unit as usize != unit
                || !self.sim.figures[sim].is_alive()
            {
                continue;
            }
            let here =
                self.field.cells[self.fighters[i].y as usize * DIM + self.fighters[i].x as usize].surface;
            if fire::order_pours(here, dest) {
                self.pour_oil(i, at);
            }
        }
    }

    /// `Melee_AdjacentEnemyDir` (`0x004972F9`) reduced to the question
    /// [`Self::step_one`] asks of it: **is the first enemy it would step at a
    /// pot of oil?** Eight neighbours in facing order, an enemy by owner, at
    /// the figure's own height, and not within a cell of the field's edge.
    pub(super) fn adjacent_oil(&self, i: usize) -> Option<usize> {
        let f = &self.fighters[i];
        let (x, y) = (f.x as i32, f.y as i32);
        if x < 1 || y < 1 || x >= DIM as i32 - 1 || y >= DIM as i32 - 1 {
            return None;
        }
        let mine = self.sim.figures[f.sim].owner;
        let here = self.field.cells[y as usize * DIM + x as usize].elevation;
        for (dx, dy) in FACING_DELTA {
            let c = (y + dy) as usize * DIM + (x + dx) as usize;
            let Some(o) = self.occupant[c] else { continue };
            let o = o as usize;
            if self.sim.figures[self.fighters[o].sim].owner == mine
                || self.field.cells[c].elevation != here
            {
                continue;
            }
            return (self.fighters[o].troop == Troop::Oil && self.is_alive(o)).then_some(o);
        }
        None
    }

    /// **A siege tower's step** — `Cell_TryEnterEngine`, and `FUN_00491492`
    /// when it refuses.
    ///
    /// Returns `true` when the step was consumed: the tower docked, or it is
    /// stopped. `false` means the leading edge is clear and the ordinary mover
    /// takes it from here.
    ///
    /// When the edge is refused and there is nowhere to dock, the original
    /// tries `FUN_004912EC` — a side-step through three rotations each way —
    /// and gives up on its destination within three cells of it, else waits a
    /// hundred frames. The side-step is not built; the tower gives up within
    /// three cells and otherwise stands.
    pub(super) fn tower_step(&mut self, i: usize, next: Pos) -> bool {
        let (x, y) = (self.fighters[i].x, self.fighters[i].y);
        let Some(dir) = facing_from_delta(next.x as i32 - x as i32, next.y as i32 - y as i32) else {
            return false;
        };
        if self.engine_edge_is_clear(i, dir) {
            return false;
        }
        if self.dock_tower(i) {
            return true;
        }
        let f = &mut self.fighters[i];
        if chebyshev(f.x as i16, f.y as i16, f.target.0 as i16, f.target.1 as i16) < 3 {
            f.target = (f.x, f.y);
            f.path.clear();
        }
        f.anim = Motion::Idle;
        true
    }

    /// **`Cell_TryEnterEngine` (`0x00490C59`)** for a tower: the leading edge
    /// of a 3 × 3 footprint, from [`crate::siege::ENGINE_EDGE_ORTHO`] or
    /// [`crate::siege::ENGINE_EDGE_DIAG`].
    ///
    /// ```c
    /// every edge cell: elevation within 1 of the engine's own, else 2;
    ///                  flags & 0x90 -> 2;  flags & 0x20 or 0x40 -> 6 for a ram, 2 otherwise;
    ///                  a figure there -> counted;
    /// any figure counted -> 2, else 1
    /// ```
    ///
    /// **Any figure in the edge stops an engine**, friend or enemy, which is
    /// what makes a tower hard to push through its own army. Two cells from the
    /// field's edge it is stopped outright. **Only a tower is tested this
    /// way**: our rams and catapults still step as one cell, as they did
/// before, and that is a deviation of this crate's, noted
    /// widened.
    fn engine_edge_is_clear(&self, i: usize, dir: u8) -> bool {
        let (x, y) = (self.fighters[i].x as i32, self.fighters[i].y as i32);
        let (lo, hi) = (2, DIM as i32 - 2);
        let off_edge = match dir {
            0 => y < lo,
            1 => x >= hi || y < lo,
            2 => x >= hi,
            3 => x >= hi || y >= hi,
            4 => y >= hi,
            5 => x < lo || y >= hi,
            6 => x < lo,
            _ => x < lo || y < lo,
        };
        if off_edge {
            return false;
        }
        let own = self.field.cells[y as usize * DIM + x as usize].elevation as i32;
        let edge: &[(i32, i32)] = if dir & 1 == 0 {
            &crate::siege::ENGINE_EDGE_ORTHO[dir as usize]
        } else {
            &crate::siege::ENGINE_EDGE_DIAG[dir as usize]
        };
        let mut figures = 0;
        for &(dx, dy) in edge {
            let (cx, cy) = (x + dx, y + dy);
            if !(0..DIM as i32).contains(&cx) || !(0..DIM as i32).contains(&cy) {
                return false;
            }
            let c = cy as usize * DIM + cx as usize;
            let cell = self.field.cells[c];
            let e = cell.elevation as i32;
            if e < own - 1 || e > own + 1 {
                return false;
            }
            let refused = terrain::flag::NO_ENTRY | crate::siege::FLAG_WALL | crate::siege::FLAG_DRAWBRIDGE;
            if cell.flags & refused != 0 {
                return false;
            }
            if self.occupant[c].is_some() {
                figures += 1;
            }
        }
        figures == 0
    }

    /// **A siege tower docks** — `FUN_00491492` (`0x00491492`), the whole of
    /// its success arm. See [`crate::siege::lay_tower_ramp`] for what it writes.
    ///
    /// ```c
    /// wall.flags = 0;  BattleMan_Destroy(tower);  FUN_004921E5(…);  …frames…
    /// Path_BuildTerrainTemplate();  Path_BuildElevation();
    /// FUN_0048EE46(wall);                       /* a defence post */
    /// g_siegeBreachScore += 3;  g_siegeApproachScore += 4;
    /// FUN_004262CF(0x11);                       /* siegedoc.wav */
    /// ```
    fn dock_tower(&mut self, i: usize) -> bool {
        let (x, y, polar) = {
            let f = &self.fighters[i];
            (f.x as i32, f.y as i32, f.polar)
        };
        let Some((dir, wall)) = crate::siege::tower_dock_site(&self.field, x, y, polar) else {
            return false;
        };
        self.destroy_fighter(i);
        let touched = crate::siege::lay_tower_ramp(&mut self.field, x, y, dir, wall);
        for c in touched {
            self.sync_cell(c);
        }
        self.file_defence_post(wall);
        self.ai.breach_score += crate::siege::DOCK_BREACH_SCORE;
        self.ai.approach_score += crate::siege::DOCK_APPROACH_SCORE;
        self.sim.cues.tower_dock();
        true
    }

    /// `BattleMan_Destroy` (`0x0046EBE4`) — **the figure is gone**, not dead:
    /// the record is cleared
    /// the figure keeps its index and is left with no men, dead, off its cell,
    /// and already at the last frame of falling so nothing is drawn falling.
    fn destroy_fighter(&mut self, i: usize) {
        let sim = self.fighters[i].sim;
        {
            let f = &mut self.sim.figures[sim];
            f.men = 0;
            f.hits = 0;
            f.state = State::Dead;
            f.opponent = None;
            f.target = None;
        }
        let cell = self.fighters[i].y as usize * DIM + self.fighters[i].x as usize;
        if self.occupant[cell] == Some(i as u16) {
            self.occupant[cell] = None;
        }
        let f = &mut self.fighters[i];
        f.anim = Motion::Dying;
        f.phase = 95;
        f.path.clear();
    }

    /// `FUN_0048EE46` (`0x0048EE46`) — **append a cell to the defence-post
    /// table**: the first empty of the first nineteen slots, and when all
    /// nineteen are taken, **the twentieth is overwritten**, every time. No
    /// test for a cell already there. `[V]`.
    fn file_defence_post(&mut self, cell: usize) {
        let posts = &mut self.ai_field.defence_posts;
        match posts[..19].iter().position(|&p| p == 0) {
            Some(k) => posts[k] = cell,
            None => posts[19] = cell,
        }
    }

    /// [`fire::bridge_fire`], with the cells it wrote re-derived and the call
    /// heard — `FUN_0048551D` opens with `Sound_PlayFile("dest_ind.wav")`.
    pub(super) fn bridge_fire_at(&mut self, x: i32, y: i32) {
        let touched = fire::bridge_fire(&mut self.field, &mut self.missiles, x, y);
        for c in touched {
            self.sync_cell(c);
        }
        self.sim.cues.bridge_fire();
    }

    pub(super) fn opponent_of(&self, i: usize) -> Option<usize> {
        let op = self.sim.figures[self.fighters[i].sim].opponent?;
        self.fighters.iter().position(|f| f.sim == op)
    }

    /// The eight neighbours in `Melee_FindAdjacentEnemy`'s order: N, NW, NE, W,
    /// E, SW, SE, S. The order decides which enemy a figure picks, so it is
/// part of the behaviour.
    pub(super) fn adjacent_enemy(&self, i: usize) -> Option<usize> {
        const ORDER: [(i32, i32); 8] = [
            (0, -1),
            (-1, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (1, 1),
            (0, 1),
        ];
        let f = &self.fighters[i];
        if f.troop.is_siege() {
            return None;
        }
        for (dx, dy) in ORDER {
            let nx = f.x as i32 + dx;
            let ny = f.y as i32 + dy;
            if !(0..DIM as i32).contains(&nx) || !(0..DIM as i32).contains(&ny) {
                continue;
            }
            let Some(o) = self.occupant[ny as usize * DIM + nx as usize] else { continue };
            let o = o as usize;
            // Siege engines are skipped entirely as melee targets.
            if self.fighters[o].side != f.side
                && self.is_alive(o)
                && !self.fighters[o].troop.is_siege()
            {
                return Some(o);
            }
        }
        None
    }
}

