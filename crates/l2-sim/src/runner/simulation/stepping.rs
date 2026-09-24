#![allow(unused_imports)]
use super::*;
use super::deploy_part::*;
use super::queries::*;
use super::super::*;

impl BattleRunner {
    pub fn step(&mut self) {
        self.sync_positions();
        self.units.rebuild_from_figures(&mut self.sim.figures);
        // Battle_Frame runs Battle_CountMenByType 0x00481B9A every frame, 004b0000.c:6703 [V]
        self.ai.count_men(&self.sim.figures);
        if self.siege.is_siege {
            self.recount_siege();
            self.assault_repulsed();
        }

        let reform = {
            let BattleRunner { units, sim, positions, ai_field, ai, .. } = self;
            ai::update_all_units(units, &mut sim.figures, positions, ai_field, ai)
        };
        if let Some(side) = self.ai.withdrawal {
            self.withdrawn = Some(side);
        }

        // `FUN_0047a814` (`0x0047A814`) is tested on every `BattleUnit_Order`, 00470000.c:4393 [V]
        for (u, tx, ty) in std::mem::take(&mut self.ai.orders) {
            self.pour_on_order(u, tx, ty);
        }
        for u in reform {
            self.reform_unit(u);
        }

        // `Battle_UpdateAllMen` (`0x004822ED`): `targeted` `:893-894` and
        // `firing` `:896-897`, each counted down to zero.
        for f in self.sim.figures.iter_mut() {
            // 00480000.c:893-894 `if (targeted != 0) targeted - 1` [V]
            if f.targeted != 0 {
                f.targeted -= 1;
            }
            f.firing = f.firing.saturating_sub(1);
        }

        self.humans_in_woods = 0;
        for i in 0..self.fighters.len() {
            self.update_man(i);
            self.step_one(i);
        }
        self.update_missiles();
        // **`FUN_004859E5`, the wood fire.** `Battle_Frame` runs it after the
        // unit sweep; this runner's unit sweep is at the top of the frame, so
        // it runs here, after the missiles, which is the nearest place with
        // the original's neighbours on both sides.
        if self.wood_fire {
            let touched = fire::spread_woodland(&mut self.field, &mut self.missiles, &mut self.wood_fire);
            for c in touched {
                self.sync_cell(c);
            }
        }
        // Melee_Tick is per man, in step_one: BattleMan_StateMelee 0x004831D8, 00480000.c:1395
        self.sim.tick += 1;
        for i in 0..self.fighters.len() {
            if !self.is_alive(i) && self.fighters[i].anim != Motion::Dying {
                let f = &self.fighters[i];
                let cell = f.y as usize * DIM + f.x as usize;
                if self.occupant[cell] == Some(i as u16) {
                    self.occupant[cell] = None;
                }
                self.fighters[i].anim = Motion::Dying;
                self.fighters[i].phase = 0;
                self.fighters[i].corpse = 0;
                self.fighters[i].path.clear();
            } else if self.fighters[i].anim == Motion::Dying {
                // The corpse state's own count, `+0x173`. It stops at the
                // bound instead of freeing the slot — see [`Fighter::corpse`].
                let limit = self.fighters[i].corpse_frames();
                if self.fighters[i].corpse < limit {
                    self.fighters[i].corpse += 1;
                }
            }
        }
        self.tick += 1;
    }

    pub fn run(&mut self, ticks: u32) {
        for _ in 0..ticks {
            self.step();
        }
    }

    /// * `g_attackersOnWall` (`0x00553E64`) — live side-4 figures standing on
    ///   surface 5. Every defender handler tests it, at 1, 2, 3, 4 and 6.
    ///
    /// * `g_siegeEngineCount` (`0x00553FF0`) — live figures of troop type 7, 8
    ///   or 9. Three attacker handlers will not move onto the castle objective
    ///   while it is zero, and `Battle_CheckOutcome` ends the battle when it
    /// and the breach score are both zero.
    fn recount_siege(&mut self) {
        let mut on_wall = 0;
        let mut engines = 0;
        // `Battle_CountMenByType` (`0x00481B9A`), 00480000.c:739 and :790 —
        // `if (troopType == 9) bVar1 = true;` … `if (!bVar1) DAT_004eeb6c = 0;`.
        // The orientation outlives the coin only while a ram does.
        let mut rams = 0;
        for f in self.fighters.iter() {
            if !self.sim.figures[f.sim].is_alive() {
                continue;
            }
            if f.troop.index() >= 7 && f.troop.index() <= 9 {
                engines += 1;
            }
            if f.troop == Troop::BatteringRams {
                rams += 1;
            }
            if f.side == SIDE_B
                && self.field.cells[f.y as usize * DIM + f.x as usize].surface
                    == crate::siege::SURFACE_BAILEY
            {
                on_wall += 1;
            }
        }
        self.ai.attackers_on_wall = on_wall;
        self.ai.siege_engine_count = engines;
        self.ai.ramparts_breached = self.siege.ramparts_breached as i32;
        if rams == 0 {
            self.ai_field.orientation = 0;
        }
    }


    pub(crate) fn members(&self, unit: usize) -> Vec<usize> {
        (0..self.fighters.len())
            .filter(|&i| {
                let f = &self.sim.figures[self.fighters[i].sim];
                f.is_alive() && f.unit as usize == unit
            })
            .collect()
    }

    /// `Formation_PickGeometry` (`0x004819CC`): the member with the highest
    /// [`TYPE_PRIORITY`] dictates footprint and figures per row.
    fn pick_geometry(&self, members: &[usize]) -> (i32, i32) {
        let mut best = (0i32, Troop::Peasants);
        for &m in members {
            let t = self.fighters[m].troop;
            if TYPE_PRIORITY[t.index()] > best.0 {
                best = (TYPE_PRIORITY[t.index()], t);
            }
        }
        (FOOTPRINT[best.1.index()], ROW_MAX[best.1.index()])
    }

    /// **a unit whose `+0x09` is 1 forms two columns whatever its troop type
    /// would give**. `Formation_ComputeRect` (`0x0048A1C9`) is one line —
    /// `if (unit.field_0x9 == 1) g_formationCols = 2;` — and it is the only
    /// effect the `V` key has. **[V]**
    pub(crate) fn unit_geometry(&self, unit: usize, members: &[usize]) -> (i32, i32) {
        let (footprint, cols) = self.pick_geometry(members);
        if self.units.get(unit).orientation == 1 {
            return (footprint, 2);
        }
        (footprint, cols)
    }

}

pub(crate) fn narrow((x, y): (i16, i16)) -> (u8, u8) {
    (x as u8, y as u8)
}

