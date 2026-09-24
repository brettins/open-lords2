#![allow(unused_imports)]
use super::*;
use super::siege_works::*;
use super::melee::*;
use super::super::*;

impl BattleRunner {
    /// ```c
    /// if (flags & 0x40) { /* drawbridge: a hole in the wall once it is down */ }
    /// if (flags & 0x20) return (side == 0) ? 1 : 5;   /* wall: 5 -> state 6  */
    /// if (flags & 0x08) { if (side == 4) { DAT_00553F3C = 1; return 2; } return 0; }
    /// ```
    pub(crate) fn strike_castle(&mut self, i: usize, dst: usize) -> bool {
        use crate::siege::{FLAG_DRAWBRIDGE, FLAG_KEEP, FLAG_WALL};
        let flags = self.field.cells[dst].flags;
        let side = self.fighters[i].side;
        let troop = self.fighters[i].troop;
        let engine = troop.index() >= 7;
        let is_ram = troop == Troop::BatteringRams;

        if flags & FLAG_DRAWBRIDGE != 0 && !is_ram {
            return false;
        }

        if flags & (FLAG_WALL | FLAG_DRAWBRIDGE) != 0 {
            if side == SIDE_A {
                return false;
            }
            if engine && !is_ram {
                self.stand(i);
                return true;
            }
            let standing = self.field.cells
                [self.fighters[i].y as usize * DIM + self.fighters[i].x as usize]
                .surface;
            let blow = crate::siege::strike_wall(&mut self.siege, standing, is_ram);
            // `BattleMan_StateAttackWall` (`0x00483A88`) opens with
            // `Anim_StrikeA2` and drops back to `dly state` when
            // `Cell_NeighbourHasSurface(.., 8)` finds no wall left, so the pose
            // ends with the wall: the tick after it is gone this arm answers
            // false and the mover stands him.
            //
            // **And it sets `role = 1` first** (`00480000.c:1556`), the line
            // before the call: `Anim_StrikeA2` runs its cycle only for the
            // swinging man, so a wall-batterer with the default
            // [`Role::Defending`] was drawn in the defender's standing pose,
            // hammering a gate without moving his arms.
            let sim = self.fighters[i].sim;
            self.sim.figures[sim].role = crate::Role::Attacking;
            let facing = self.fighters[i].facing;
            self.strike(i, facing);
            // `FUN_0049694F(mapX, mapY, 4)` — `Wall_Smash` — centred on the
// *attacker's* cell
            // enter, which is the original's argument list exactly. Every wall
            // cell in the square loses `0x20` and becomes
            // [`crate::siege::SURFACE_BAILEY`]; the elevation is left alone,
            //
            // > This used to open the single destination cell. A one-cell hole
            // > in a castle wall is a funnel: the first figure through it
            // > occupies the cell, the pathfinder marks a friendly-occupied
            // > cell 998, and everybody else waits outside for ever. That is
            // > the whole of *"848 men could not reach two figures through an
            // > open gate"*. `docs/decisions.md` `C100`.
            let (ax, ay) = (self.fighters[i].x as i32, self.fighters[i].y as i32);
            if blow != crate::siege::WallBlow::Absorbed {
                self.sim.cues.wall_smashed();
            }
            match blow {
                crate::siege::WallBlow::RampartBreached => {
                    crate::siege::smash_walls(
                        &mut self.field,
                        ax,
                        ay,
                        crate::siege::SMASH_RADIUS,
                    );
                    self.rebuild_blocked();
                    self.ai.breach_score += 1;
                    self.ai.approach_score += 1;
                }
                crate::siege::WallBlow::GateBreached => {
                    crate::siege::smash_walls(
                        &mut self.field,
                        ax,
                        ay,
                        crate::siege::SMASH_RADIUS,
                    );
                    self.rebuild_blocked();
                    self.ai.breach_score += crate::siege::GATE_BREACH_SCORE;
                    self.ai.approach_score += crate::siege::GATE_BREACH_SCORE;
                }
                crate::siege::WallBlow::Absorbed => {}
            }
            return true;
        }

        if flags & FLAG_KEEP != 0 {
            if side == SIDE_B {
                self.siege.broke_in = true;
            }
            self.stand(i);
            return true;
        }
        false
    }

    /// `FUN_0048EE46` — file a cell in the twenty-entry defence-post table.
    pub(super) fn register_defence_posts(&mut self, cell: usize) {
        for n in crate::siege::orthogonal_neighbours(cell) {
            if self.field.cells[n].surface != crate::siege::SURFACE_BAILEY {
                continue;
            }
            if self.ai_field.defence_posts.contains(&n) {
                continue;
            }
            if let Some(free) = self.ai_field.defence_posts.iter_mut().find(|p| **p == 0) {
                *free = n;
            }
        }
    }

    pub(in crate::runner) fn rebuild_blocked(&mut self) {
        for (c, cell) in self.field.cells.iter().enumerate() {
            self.blocked[c] = cell.impassable();
        }
        self.refresh_ai_surfaces();
    }

    pub(crate) fn refresh_ai_surfaces(&mut self) {
        for (c, cell) in self.field.cells.iter().enumerate() {
            self.ai_field.surface[c] = cell.surface;
            self.ai_field.elevation[c] = cell.elevation;
        }
    }

    pub(crate) fn swap_places(&mut self, a: usize, b: usize) {
        if a == b {
            return;
        }
        let (ax, ay) = (self.fighters[a].x, self.fighters[a].y);
        let (bx, by) = (self.fighters[b].x, self.fighters[b].y);
        self.fighters[a].x = bx;
        self.fighters[a].y = by;
        self.fighters[b].x = ax;
        self.fighters[b].y = ay;
        // **The two men are drawn walking past each other**, not teleported.
        // `BattleMen_SwapPlaces` (`0x0049005F`) exchanges `mapX`, `mapY` and
        // `cellOffset` and touches neither `walking` nor `dirc`, so the
        // original draws both of them a whole cell away in one frame; ours
        // may not — `no_drawn_man_ever_jumps_half_a_cell_in_one_tick` counts
        // 500 such jumps, worst 64 px. Each man takes the facing of the cell
        // he was given and starts that crossing, which is the same cell at the
        // same tick and a trail behind it. `[D]`.
        for (m, from, to) in [(a, (ax, ay), (bx, by)), (b, (bx, by), (ax, ay))] {
            if let Some(d) =
                facing_from_delta(to.0 as i32 - from.0 as i32, to.1 as i32 - from.1 as i32)
            {
                self.fighters[m].facing = d;
            }
            self.fighters[m].progress.begin_crossing();
            // **Both men are stood this frame.** `BattleMan_Step`
            // (`0x0048F1DD`) returns 0 from the swap arm and
            // `BattleMan_StateWalk` (`0x0048314E`) is `Anim_Walk(); if
            // (BattleMan_Step(0) == 0) Anim_Stand();`, so neither man's walk
            // counter is stepped by this move —
            // `a_marching_man_is_drawn_at_the_phase_he_ended_the_last_tick_with`
            // reads that counter. **[V]**.
            self.stand(m);
        }
        self.fighters[a].path.clear();
        self.fighters[b].path.clear();
        self.occupant[by as usize * DIM + bx as usize] = Some(a as u16);
        self.occupant[ay as usize * DIM + ax as usize] = Some(b as u16);
        // `BattleMan_Step`'s swap arm (`0x0048F1DD`): `other.state = 1;
        // other.delayState = 3; other.delay = (other & 1) + 1`. See
        // [`Fighter::delay`].
        self.fighters[b].delay = (b & 1) as u8 + 1;
        self.fighters[a].barred = 0;
        self.fighters[b].barred = 0;
    }

}

