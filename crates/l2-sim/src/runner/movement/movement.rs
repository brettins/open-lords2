#![allow(unused_imports)]
use super::*;
use super::pathfinding::*;
use super::*;

#[path = "sidestep.rs"]
mod sidestep;

#[cfg(test)]
#[path = "tests_march.rs"]
mod tests_march;

impl BattleRunner {
    pub(crate) fn next_step(&self, i: usize) -> Option<Pos> {
        let f = &self.fighters[i];
        // `BattleMan_NextPathDir` (`0x00491A34`) reads the waypoint and returns
        // `Dir_FromDelta(mapX, mapY, wpX, wpY)`; `FUN_00491B1F` then moves the
        // man **one cell** that way. Ours returned the waypoint itself, and
        // that was the 32 and 64 px teleports
        // `no_drawn_man_ever_jumps_half_a_cell_in_one_tick` sees.
        let (dx, dy) = match f.path.last() {
            Some(&wp) => (wp.x as i32 - f.x as i32, wp.y as i32 - f.y as i32),
            None => (f.target.0 as i32 - f.x as i32, f.target.1 as i32 - f.y as i32),
        };
        let facing = facing_from_delta(dx, dy)?;
        let (sx, sy) = FACING_DELTA[facing as usize];
        let nx = f.x as i32 + sx;
        let ny = f.y as i32 + sy;
        if !(0..DIM as i32).contains(&nx) || !(0..DIM as i32).contains(&ny) {
            return None;
        }
        Some(Pos::new(nx as u8, ny as u8))
    }


    pub fn battle_size_class(&self) -> u8 {
        self.size_class
    }

    pub(crate) fn sync_cell(&mut self, c: usize) {
        let cell = self.field.cells[c];
        self.blocked[c] = cell.impassable();
        self.ai_field.surface[c] = cell.surface;
        self.ai_field.elevation[c] = cell.elevation;
    }

    /// ```c
    /// if (surface == 10)   BattleMan_BurnTick(man);
    /// if (surface == 0x11) BattleMan_BurnTick(man);
    /// if (surface == 0x0F && man.ownerIsHuman) DAT_005530E8++;
    /// ```
    ///
    /// `[D]`.
    pub(crate) fn update_man(&mut self, i: usize) {
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

    pub(crate) fn oil_cross(&mut self, slot: usize) {
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
    /// This crate has no corpse lifetime and only the living pour. `[D]`.
    pub(crate) fn pour_oil(&mut self, pot: usize, to: (u8, u8)) {
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

    pub(crate) fn pour_on_order(&mut self, unit: usize, x: i16, y: i16) {
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
    pub(crate) fn adjacent_oil(&self, i: usize) -> Option<usize> {
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
    /// When the edge is refused and there is nowhere to dock, the original
    /// tries `FUN_004912EC` — a side-step through three rotations each way —
    /// and gives up on its destination within three cells of it, else waits a
    /// hundred frames. The side-step is not built; the tower gives up within
    /// three cells and otherwise stands.
    pub(crate) fn tower_step(&mut self, i: usize, next: Pos) -> bool {
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
        // **[D]** — a substitute, not the original's write. `BattleMan_Destroy`
        // (`0x0046EBE4`) calls `BattleMan_Clear` and never touches `+0x173`; it
        // frees the record outright, and a freed slot cannot be drawn. We keep
        // the slot, so we park the death timer at its bound instead, which is
        // what `corpse_gone` reads and what stops anything drawing this man.
        f.corpse = f.corpse_frames();
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
    pub(crate) fn bridge_fire_at(&mut self, x: i32, y: i32) {
        let touched = fire::bridge_fire(&mut self.field, &mut self.missiles, x, y);
        for c in touched {
            self.sync_cell(c);
        }
        self.sim.cues.bridge_fire();
    }

    pub(crate) fn opponent_of(&self, i: usize) -> Option<usize> {
        let op = self.sim.figures[self.fighters[i].sim].opponent?;
        self.fighters.iter().position(|f| f.sim == op)
    }

    pub(crate) fn adjacent_enemy(&self, i: usize) -> Option<usize> {
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

