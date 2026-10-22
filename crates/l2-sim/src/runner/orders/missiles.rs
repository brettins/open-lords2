#![allow(unused_imports)]
use super::*;
use super::slots::*;
use super::movement::*;
use super::targeting::*;
use super::super::*;

impl BattleRunner {
    /// **`Missile_UpdateAll` (`0x00485BB1`)** — every live missile, ascending
    /// slot, then the countdown that retires the ones that have hit.
    pub(crate) fn update_missiles(&mut self) {
        for slot in 1..=missile::MAX_MISSILES {
            if !self.missiles.get(slot).is_live() {
                continue;
            }
            if !self.step_missile(slot) {
                continue;
            }
            match self.missiles.get(slot).class {
                missile::CLASS_FIRE => {
                    if self.missiles.get(slot).ttl == fire::FIRE_RESTORE_AT {
                        let m = *self.missiles.get(slot);
                        let c = fire::put_out(&mut self.field, &m);
                        self.sync_cell(c);
                    }
                }
                // **Debris falls on every other frame.** `Missile_UpdateAll`'s
                // class-4 arm ends `if (m[+0x31] == 1) m[+0x31] = 0; else
                // m[+0x31] = 1;` — `+0x31` is the sub-step count, so the
                // rubble is stepped one frame in two. `[V]`.
                missile::CLASS_DEBRIS => {
                    let m = self.missiles.get_mut(slot);
                    m.sub_steps = i8::from(m.sub_steps != 1);
                }
                missile::CLASS_OIL => self.oil_cross(slot),
                _ => {}
            }
            let m = self.missiles.get_mut(slot);
            if m.ttl != 0 {
                m.ttl -= 1;
                if m.ttl < 1 {
                    self.missiles.free(slot);
                }
            }
        }
    }

    /// **`Missile_Step` (`0x00492C8B`)** — one tick of one missile: four
    /// sub-steps, and after each of them the tests that can end it.
    pub(crate) fn step_missile(&mut self, slot: usize) -> bool {
        // **`Missile_Step`'s first test: a fire arrow over woodland.** Once a
        // frame, on the cell the arrow starts the frame in, before anything
        // else — `FUN_00485861` lights the wood, `DAT_0053E9D0 = 1` starts the
        // spread, the arrow is freed and the shooter's unit's `targetCell` is
        // cleared, so **one ordered volley lights one cell**:
        //
        // ```c
        // if (cell.surface == 0x0F && m.+0x44 != 0 &&
        //     (shooter.ownerIsHuman == 0 || m.+0x1C == m.+0x44)) { … }
        // ```
        {
            let m = *self.missiles.get(slot);
            let cell = m.cell_y as usize * DIM + m.cell_x as usize;
            let human = self
                .sim
                .figures
                .get(m.shooter as usize)
                .is_some_and(|f| f.owner_is_human);
            let here = fire::cell_byte_offset(m.cell_x as i32, m.cell_y as i32);
            if m.fire_arrow != 0
                && (!human || here == m.fire_arrow)
                && self.field.cells[cell].surface == fire::SURFACE_WOODLAND
            {
                let (x, y) = (m.cell_x as i32, m.cell_y as i32);
                if let Some(c) = fire::ignite_woodland(&mut self.field, &mut self.missiles, x, y) {
                    self.sync_cell(c);
                }
                self.wood_fire = true;
                self.missiles.free(slot);
                let unit = self
                    .sim
                    .figures
                    .get(m.shooter as usize)
                    .map_or(0, |f| f.unit as usize);
                if (1..=MAX_UNITS).contains(&unit) {
                    self.units.get_mut(unit).target_cell = 0;
                }
                return false;
            }
        }
        {
            let m = self.missiles.get_mut(slot);
            m.ticks_flown += 1;
            let spent = m.ticks_flown > m.range_ticks;
            if spent {
                // `:1264`, a shot that fell short.
                self.shots_wasted(slot, crate::unit::SHOTS_WASTED_SHORT);
                self.mark_shooter_engaged(slot);
                self.missiles.free(slot);
                return false;
            }
        }
        let sub_steps = self.missiles.get(slot).sub_steps;
        for _ in 0..sub_steps.max(0) {
            self.missiles.get_mut(slot).sub_step();
            if self.missiles.get(slot).off_map() {
                self.missiles.free(slot);
                return false;
            }
            if !self.missile_cell_tests(slot) {
                return false;
            }
        }
        true
    }

    /// `g_battleUnits[sVar3].field_0x2d = n`, where `sVar3` is
    /// `g_battleMen[m.+0x06].unit` (`00490000.c:1219-1220`).
    /// `g_battleMen[shooter].engaged = 1` beside each of `Missile_Step`'s three
    /// give-up arms — `00490000.c:1246`, `:1252` and `:1263`. State 17 reads it
    /// back to decide whether to hold an AI unit's reform. `[V]` — the hit arm
    /// at `:1353` writes `field_0x2d` and no `engaged`.
    fn mark_shooter_engaged(&mut self, slot: usize) {
        let shooter = self.missiles.get(slot).shooter as usize;
        if let Some(f) = self.sim.figures.get_mut(shooter) {
            f.engaged = true;
        }
    }

    fn shots_wasted(&mut self, slot: usize, n: u8) {
        let shooter = self.missiles.get(slot).shooter as usize;
        let u = self.sim.figures.get(shooter).map_or(0, |f| f.unit as usize);
        if u <= MAX_UNITS {
            self.units.get_mut(u).shots_wasted = n;
        }
    }

    pub(crate) fn missile_cell_tests(&mut self, slot: usize) -> bool {
        if self.missiles.get(slot).ttl != 0 {
            return true;
        }
        let cell = {
            let m = self.missiles.get(slot);
            m.cell_y as usize * DIM + m.cell_x as usize
        };
        let elevation = self.field.cells[cell].elevation;

        // **A missile over a bridge sets it alight.** `Missile_Step`, inside
        // the `+0x3C == 0` gate, for every class: `if (surface == 7) { +0x3C =
        // 8; FUN_0048551D(cell); }` — an arrow, a bolt, a catapult shot or a
        // stream of oil. The gate is read once for the sub-step, so the tests
        // below still run for this one; after it the missile is spent and
        // retires eight frames later.
        if self.field.cells[cell].surface == fire::SURFACE_BRIDGE {
            self.missiles.get_mut(slot).ttl = fire::MISSILE_ON_BRIDGE_TTL;
            let m = *self.missiles.get(slot);
            self.bridge_fire_at(m.cell_x as i32, m.cell_y as i32);
        }

        {
            let m = self.missiles.get_mut(slot);
            if (m.launch_elevation as i32) + 1 < elevation as i32 {
                m.blocked = true;
            }
        }
        if self.blocked[cell] {
            self.missiles.get_mut(slot).blocked = true;
        }
        {
            let m = self.missiles.get_mut(slot);
            let weapon_class = m.class < 3;
            if m.blocked && weapon_class {
                m.blocked_ticks = m.blocked_ticks.saturating_add(1);
                if m.blocked_ticks > missile::BLOCKED_LIMIT || m.launch_elevation == elevation {
                    // `:1247` and `:1253`, a shot the ground stopped.
                    self.shots_wasted(slot, crate::unit::SHOTS_WASTED_BLOCKED);
                    self.mark_shooter_engaged(slot);
                    self.missiles.free(slot);
                    return false;
                }
            }
        }

        // The gate is `Missile_Step`'s own — `elevation != 0 && surface == 4`,
        // masonry above ground level — in [`crate::siege::shot_damages_wall`],
        // not the `0x20` flag this used to read. The flag is the mover's
        // (`Cell_TryEnter`, `0x00490A44`); damage is the count in byte `+0`, so
        // the flag-free rampart walk `UnitOrder_SiegeAttCatapult` aims at takes
        // it, and a cell `Wall_Smash` has joined to the bailey stops taking it
        // because its surface changed.
        if self.missiles.get(slot).class == WeaponClass::Catapult.index()
            && crate::siege::shot_damages_wall(&self.field.cells[cell])
        {
            self.strike_wall_with_shot(slot, cell);
            return true;
        }

        if self.missiles.get(slot).class >= 3 {
            return true;
        }
        let Some(victim) = self.occupant[cell].map(usize::from).or_else(|| self.risen_at(cell)) else {
            return true;
        };
        let vsim = self.fighters[victim].sim;
        // Missile_Step 0x00492C8B 00490000.c:1353 `state != 2`, not `men` [V]
        if self.sim.figures[vsim].state == State::Dead {
            return true;
        }
        let (owner, shooter, power, class) = {
            let m = self.missiles.get(slot);
            (m.owner, m.shooter as usize, m.power, m.class)
        };
        if self.sim.figures[vsim].owner == owner {
            return true;
        }
        let weapon = match class {
            1 => WeaponClass::Bow,
            _ => WeaponClass::Crossbow,
        };
        let delta = elevation as i32 - self.missiles.get(slot).launch_elevation as i32;
        let size_class = 0;
        let resolved =
            missile::resolve_power(weapon, power, delta, &self.sim.figures[vsim], size_class);
        // `:1353`, the first statement of the hit arm: the shot landed, so the
        // shooter's unit stops being held out of `firing`.
        self.shots_wasted(slot, 0);
        {
            let f = &mut self.sim.figures[vsim];
            f.was_hit = true;
            f.hit_by = Some(shooter);
        }
        // **`Missile_Step`'s three sounding arms.** Slot 10 or 8 on the hit
        // itself, the same slot again when it crosses the casualty threshold —
        // dropped, always, because that buffer started a statement earlier — and
        // `FUN_004262CF(0xD)` when it was the last man. `[V]`.
        self.sim.cues.missile_hit(weapon);
        let killed = missile::apply_hit(&mut self.sim.figures[vsim], resolved);
        if killed > 0 {
            self.sim.cues.missile_casualty(weapon);
        }
        if !self.sim.figures[vsim].is_alive() {
            self.sim.cues.missile_death();
        }
        self.missiles.get_mut(slot).ttl = missile::HIT_TTL;
        true
    }

    /// [I] 00490000.c:1353 reads `g_battlefield.figure`, which keeps a body;
    /// ours frees the cell at death, so a body that stood up
    /// (B267) is found where it stands.
    fn risen_at(&self, cell: usize) -> Option<usize> {
        self.fighters.iter().position(|f| {
            let fig = &self.sim.figures[f.sim];
            f.y as usize * DIM + f.x as usize == cell
                && fig.men == 0
                && fig.owner != 0
                && fig.state != State::Dead
        })
    }
}

