use super::*;

impl BattleRunner {
    fn unit_dest(&self, unit: usize) -> (i16, i16) {
        let u = self.units.get(unit);
        (u.target_x.clamp(1, 78), u.target_y.clamp(1, 78))
    }

    /// `BattleUnit_Reform` (`0x0048970E`): re-issue every figure a destination,
    /// from the formation rectangle if it is clear and by search if it is not.
    pub(super) fn reform_unit(&mut self, unit: usize) {
        let members = self.members(unit);
        if members.is_empty() {
            return;
        }
        let (footprint, cols) = self.unit_geometry(unit, &members);
        let target = self.unit_dest(unit);
        // **`DAT_00553FE4` — "this unit was ordered onto water".**
        let dest_is_water = self.field.at(target.0 as usize, target.1 as usize).surface
            == crate::siege::SURFACE_WATER;
        if self.rect_is_clear(unit, target, members.len(), footprint, cols) {
            let rect = formation::compute_rect(target, members.len(), footprint, cols);
            let mut assigned = vec![false; members.len()];
            for i in 0..members.len() {
                let (x, y) = rect.slot(i);
                let Some(k) = self.nearest_free_figure(&members, &assigned, x, y) else {
                    continue;
                };
                assigned[k] = true;
                self.send_figure(unit, members[k], x, y, dest_is_water);
            }
        } else {
            self.assign_searched_slots(unit, &members, target, dest_is_water);
        }
        self.units.get_mut(unit).reform_gate = false;
    }

    /// `Formation_RectIsClear` (`0x00489F9D`): every slot on the map, at the
    /// destination's elevation, holding no other unit's figure, not impassable.
    fn rect_is_clear(
        &self,
        unit: usize,
        target: (i16, i16),
        figures: usize,
        footprint: i32,
        cols: i32,
    ) -> bool {
        let dest = self.field.at(target.0 as usize, target.1 as usize);
        if dest.surface == 2 {
            return false;
        }
        let rect = formation::compute_rect(target, figures, footprint, cols);
        for i in 0..figures {
            let (x, y) = rect.slot(i);
            if !(1..=0x4E).contains(&x) || !(1..=0x4E).contains(&y) {
                return false;
            }
            let cell = self.field.at(x as usize, y as usize);
            if cell.elevation != dest.elevation {
                return false;
            }
            if let Some(o) = self.occupant[y as usize * DIM + x as usize] {
                if self.unit_of(o as usize) != unit {
                    return false;
                }
            }
            if cell.impassable() {
                return false;
            }
        }
        true
    }

    /// `Formation_NearestFreeFigure` (`0x00489A62`): the member nearest `(x, y)`
    /// by Manhattan distance that has not been given a slot this pass. Greedy
    /// nearest-first,
    fn nearest_free_figure(
        &self,
        members: &[usize],
        assigned: &[bool],
        x: i32,
        y: i32,
    ) -> Option<usize> {
        let mut best: Option<(i32, usize)> = None;
        for (k, &m) in members.iter().enumerate() {
            if assigned[k] {
                continue;
            }
            let f = &self.fighters[m];
            let d = (f.x as i32 - x).abs() + (f.y as i32 - y).abs();
            if best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, k));
            }
        }
        best.map(|(_, k)| k)
    }

    /// `Formation_AssignSearchedSlots` (`0x00489798`): the destination's
    /// rectangle is blocked, so search outward from it once per figure and
    /// claim cells as they are taken.
    ///
    /// The claim list is the original's — it exists so two figures never get
    /// the same cell. The acceptance test is `Formation_SlotIsUsable`
    /// (`0x0048A672`), whose field-battle half is reproduced: a cell holding an
    /// enemy figure is usable, a cell holding another friendly unit's figure is
    /// not, and an impassable empty cell is not.
    fn assign_searched_slots(
        &mut self,
        unit: usize,
        members: &[usize],
        target: (i16, i16),
        dest_is_water: bool,
    ) {
        let mut assigned = vec![false; members.len()];
        let mut claimed: Vec<(i32, i32)> = Vec::with_capacity(members.len());
        for _ in 0..members.len() {
            let Some((x, y)) = self.find_nearby_slot(unit, target, &claimed) else {
                break;
            };
            claimed.push((x, y));
            let Some(k) = self.nearest_free_figure(members, &assigned, x, y) else {
                break;
            };
            assigned[k] = true;
            self.send_figure(unit, members[k], x, y, dest_is_water);
        }
    }

    /// `Formation_FindNearbySlot` (`0x0048A38E`), reduced to the field case:
    fn find_nearby_slot(
        &self,
        unit: usize,
        target: (i16, i16),
        claimed: &[(i32, i32)],
    ) -> Option<(i32, i32)> {
        let dest = self.field.at(target.0 as usize, target.1 as usize);
        for radius in 0..20i32 {
            let x0 = (target.0 as i32 - radius).max(0);
            let y0 = (target.1 as i32 - radius).max(0);
            let x1 = (target.0 as i32 + radius + 1).min(DIM as i32);
            let y1 = (target.1 as i32 + radius + 1).min(DIM as i32);
            for y in y0..y1 {
                for x in x0..x1 {
                    if claimed.contains(&(x, y)) {
                        continue;
                    }
                    if self.slot_is_usable(unit, x, y, dest.elevation) {
                        return Some((x, y));
                    }
                }
            }
        }
        None
    }

    /// `Formation_SlotIsUsable` (`0x0048A672`), field-battle branches only.
    ///
    /// **Two branches are still missing and both are the siege's.** The
    /// original also takes the *source* cell's `surface` and refuses a cell of
    /// any other surface when that one is
    /// [`crate::siege::SURFACE_RAMPART_WALK`] (4)
    /// is only ever slotted along it; and it refuses a side-0 unit an empty
    /// cell of surface under 4 in a siege. `[V]` — 359 bytes, four parameters.
    pub(super) fn slot_is_usable(&self, unit: usize, x: i32, y: i32, elevation: u8) -> bool {
        let cell = self.field.at(x as usize, y as usize);
        let occupant = self.occupant[y as usize * DIM + x as usize];
        if cell.impassable() && occupant.is_some() {
            return true;
        }
        if let Some(o) = occupant {
            let other = self.unit_of(o as usize);
            if self.units.owner_of(other) != self.units.owner_of(unit) {
                return true;
            }
            if other != unit {
                return false;
            }
        }
        if (cell.elevation as i32) < elevation as i32 - 1 {
            return false;
        }
        !cell.impassable()
    }

    /// `Formation_SendFigure` (`0x00489B8D`): one figure's destination and the
    /// state it walks there in.
    ///
    /// `dest_is_water` is `DAT_00553FE4`, and it is **the unit's ordered
    /// destination, not this figure's slot**.
    ///
    /// > **That distinction is the whole of why the moat had never once been
    /// > filled in.** This function used to read `cell.surface == 2` off the
    /// > slot it was sending the figure to, which reads like the same thing and
    /// > a slot is or by
    /// > the formation rectangle, and **both of them reject an impassable empty
    /// > cell** — which every moat cell is. So the water branch could not be
    /// > reached from any order, by the player or by the AI, and
    /// > `State::FillingMoat` had no writer at all. The original instead caches
    /// > the flag in `Formation_RectIsClear` off the *unit's* destination and
    /// > then rejects the rectangle for the same reason,
    /// > the ditch has every one of its figures enter state 9 while walking to a
    /// > **dry** slot beside it; `BattleMan_Step`'s state-9 arm then latches
    /// > whatever impassable cell stops it. `docs/decisions.md`
    /// > `C81`. `[V]` — two functions and one global.
    fn send_figure(&mut self, unit: usize, fighter: usize, x: i32, y: i32, dest_is_water: bool) {
        let sim = self.fighters[fighter].sim;
        let gate = self.units.get(unit).reform_gate;
        let state = self.sim.figures[sim].state;
        // A figure already filling in the moat is left alone unless the unit's
        // `+0x13` gate is set.
        if state == State::FillingMoat && !gate {
            return;
        }
        if state == State::Melee {
            return;
        }
        let x = x.clamp(0, DIM as i32 - 1);
        let y = y.clamp(0, DIM as i32 - 1);
        let cell = self.field.at(x as usize, y as usize);
        let troop = self.fighters[fighter].troop;
        let owner = self.sim.figures[sim].owner;

        let enemy_there = self.occupant[y as usize * DIM + x as usize].and_then(|o| {
            let o = o as usize;
            (self.sim.figures[self.fighters[o].sim].owner != owner).then_some(o)
        });

        let mut dest = (x as u8, y as u8);
        let mut new_state = State::Idle;
        if !troop.is_siege() {
            if dest_is_water {
                // Water: fill the moat in. Knights alone are returned unchanged
                // — `docs/battle-ai.md` §5, and `L2.eng` group 214 index 3 is
                // the corroborating help text. The original's `return` here is
                // total: a knight given a moat order keeps whatever destination
                // and state it already had.
                if troop == Troop::Knights {
                    return;
                }
                new_state = State::FillingMoat;
            }
        } else if self.sim.figures[sim].owner_is_human && !gate {
            return;
        }

        let weapon = WEAPON_CLASS[troop.index()];
        // **`|| target_cell != 0` is the other half of the original's test**,
        // and without it the player's fire arrow needed an enemy standing on
        // the destination to fire at all. `Formation_SendFigure`
        // (`0x00489B8D`) reads the unit's `targetCell` into `iVar1` before the
        // loop and its state-17 arm is `weaponClass in 1..3 &&
        // (g_otherBattleMan != 0 || iVar1 != 0)`. `[V]`. It matters now that
        // `Order_StopShortOfTarget` pulls the destination back: the click that
        // sets `targetCell` is on an *empty* woodland cell, and the unit's
        // pulled-back destination has nobody on it.
        if (1..3).contains(&weapon)
            && (enemy_there.is_some() || self.units.get(unit).target_cell != 0)
        {
            self.sim.figures[sim].target = enemy_there.map(|o| self.fighters[o].sim);
            new_state = State::Shooting;
            dest = (self.fighters[fighter].x, self.fighters[fighter].y);
        }
        if weapon > 2 && (1..4).contains(&cell.elevation) {
            dest = (self.fighters[fighter].x, self.fighters[fighter].y);
        }

        self.sim.figures[sim].state = new_state;
        let f = &mut self.fighters[fighter];
        if f.target != dest {
            f.path.clear();
            f.barred = 0;
        }
        f.target = dest;
    }


    pub(super) fn step_one(&mut self, i: usize) {
        if !self.is_alive(i) {
            // **State 2 does not step `animPhase`.** `BattleMan_StateDead`
            // (`0x004830E9`) is `Anim_Collapse(); if (0x50 < ++field_0x173)
            // Destroy();`, and `Anim_CollapseA2` reads that death timer —
            // [`Fighter::corpse`], stepped in `step` — not the phase. Ours
            // counted the phase to 95 here and drew the corpse off it.
            return;
        }

        // `Anim_WalkA2` steps it (wrapping at 0x17) and `Anim_StrikeA2` steps
        // it for the swinging man only (0x27); `Anim_StandA2` and
        // `Anim_DrawBowA2` never do — `00480000.c:2509`, `super::anim`. Ours
        // advanced it once per figure per tick here, which un-froze the
        // defender's strike cycle and made the role swap invisible.
        if self.fighters[i].hold > 0 {
            self.fighters[i].hold -= 1;
        }

        if self.fill_moat_tick(i) {
            return;
        }

        let sim = self.fighters[i].sim;
        if self.sim.figures[sim].state == State::Melee {
            let mutual = self.sim.figures[sim].opponent.is_some_and(|o| {
                self.sim.figures[o].is_alive() && self.sim.figures[o].opponent == Some(sim)
            });
            if !mutual {
                self.sim.figures[sim].state = State::Idle;
                self.sim.figures[sim].opponent = None;
                // **And he stops swinging.** `BattleMan_StateMelee`
                // (`0x004831D8`) runs `Anim_Strike` at its *top*, so the tick
                // that drops a figure back to state 0 is the last tick that
                // draws him striking; the state-0 handler then stands him.
                //
                // `00480000.c:1384`: `BattleMan_StateMelee` calls `Anim_Strike`
                // at its *top* and only then writes the new state, so the last
                // tick of a duel is drawn swinging and the figure reaches state
                // 8 on the next one. Ours stood him here.
                //
                // **[D]** The original *returns* from the handler here, giving
                // the figure a tick in which he only swings. Ours writes the
                // pose and falls through to the arms below, so an arm that acts
                // this tick overwrites it. Returning costs both of the gates
                // that measure this seam: a figure dropped mid-crossing has his
                // `walking` counter frozen while the trail is drawn from it,
                // eight jumps of a whole cell over a 42-figure battle
                // (`battle_picture`'s
                // `no_drawn_man_ever_jumps_half_a_cell_in_one_tick`), and the
                // skipped melee search turns the seam's militia from 6 of 8
                // into 8 of 8. The order of the writes is the original's; the
                // skipped tick is not.
                let facing = self.fighters[i].facing;
                self.strike(i, facing);
            }
        }

        // `BattleMan_StateMelee` (`0x004831D8`): `Anim_Strike(); … Melee_Tick();
        // if ((stepFlags & 1) == 0 && BattleMan_Step(1)) Anim_Walk();`. The
        // `noInterrupt = 1` argument makes the mover count and nothing else —
        // it returns at the landing without deciding a new step —
        // engaged mid-crossing keeps walking to the cell he committed to and
        // is drawn walking while he does. Ours zeroed his progress and struck
        // on the spot, which put him on that cell up to 30 pixels early.
        //
        // **His facing is not rewritten either.** The 999 arm writes `dirc2`
        // (`+0x19`, the strike frame); `dirc` (`+0x18`, the sub-cell offset)
        // is the crossing's and nothing touches it. We carry one facing, so
        // the crossing keeps it.
        if self.sim.figures[self.fighters[i].sim].state == State::Melee {
            if !self.fighters[i].progress.free {
                let troop = self.fighters[i].troop;
                self.fighters[i].progress.tick(troop);
                self.march(i, true);
                return;
            }
            match self.opponent_of(i) {
                Some(op) => {
                    let (ox, oy) = (self.fighters[op].x as i32, self.fighters[op].y as i32);
                    let f = &mut self.fighters[i];
                    if let Some(fc) = facing_from_delta(ox - f.x as i32, oy - f.y as i32) {
                        f.facing = fc;
                    }
                    let facing = f.facing;
                    f.progress = Progress::default();
                    self.strike(i, facing);
                }
                None => self.stand(i),
            }
            return;
        }

        self.retarget(i);

        // **`BattleMan_Step`'s head, and everything below it is that function's
        // decision section.** `0x0048F1DD` opens with the sub-cell counter and
        // returns from it while `(stepFlags & 1) == 0` — before
        // `Melee_AdjacentEnemyDir`, before `Dir_FromDelta` /
        // `BattleMan_NextPathDir`, before `BattleMan_TryStepDir`.
        //
        // Ours ran the whole thing every tick and entered on the last one. Two
        // visible faults fell out of that and `docs/battle.md` §13.6 counted
        // both — mid-crossing turns, and refusals *after* the walk that put a
        // man back on the square he never left. `docs/decisions.md`
        // C200.
        let troop = self.fighters[i].troop;
        if !self.fighters[i].progress.tick(troop) {
            self.march(i, true);
            return;
        }

        if let Some(enemy) = self.adjacent_enemy(i) {
            let (ex, ey) = (self.fighters[enemy].x as i32, self.fighters[enemy].y as i32);
            {
                let f = &mut self.fighters[i];
                if let Some(fc) = facing_from_delta(ex - f.x as i32, ey - f.y as i32) {
                    f.facing = fc;
                }
                f.progress = Progress::default();
            }
            let facing = self.fighters[i].facing;
            self.strike(i, facing);
            let (ua, ub) = (self.unit_of(i), self.unit_of(enemy));
            self.join_melee(ua);
            self.join_melee(ub);
            let (a, b) = (self.fighters[i].sim, self.fighters[enemy].sim);
            self.sim.engage(a, b);
            return;
        }

        if self.fighters[i].at_target() {
            self.stand(i);
            self.fighters[i].progress = Progress::default();
            self.fire_tick(i);
            return;
        }

        let Some(next) = self.next_step(i) else {
            self.stand(i);
            return;
        };
        {
            let f = &mut self.fighters[i];
            let d = facing_from_delta(next.x as i32 - f.x as i32, next.y as i32 - f.y as i32);
            if let Some(d) = d {
                f.facing = d;
            }
            // `FUN_00488436`, every frame a tower walks.
            if f.troop == Troop::SiegeTowers {
                f.polar = crate::siege::tower_polar(f.facing, f.polar);
            }
        }
        if troop.index() < 7 {
            if let Some(pot) = self.adjacent_oil(i) {
                let (ua, ub) = (self.unit_of(i), self.unit_of(pot));
                self.join_melee(ua);
                self.join_melee(ub);
                let at = (self.fighters[i].x, self.fighters[i].y);
                self.pour_oil(pot, at);
                return;
            }
        }
        self.stand(i);
        self.enter(i, next);
    }

    fn retarget(&mut self, i: usize) {
        let sim = self.fighters[i].sim;
        match self.sim.figures[sim].state {
            State::Chasing => {
                let alive = self.sim.figures[sim]
                    .target
                    .is_some_and(|t| self.sim.figures[t].is_alive());
                if !alive {
                    let chosen = self.chase_target(i);
                    self.sim.figures[sim].target = chosen;
                    if let Some(t) = chosen {
                        self.sim.figures[t].targeted =
                            self.sim.figures[t].targeted.saturating_add(2);
                    }
                }
                if let Some(t) = self.sim.figures[sim].target {
                    let (tx, ty) = (self.fighters[t].x, self.fighters[t].y);
                    self.fighters[i].target = (tx, ty);
                }
            }
            State::Shooting => {
                let alive = self.sim.figures[sim]
                    .target
                    .is_some_and(|t| self.sim.figures[t].is_alive());
                // **`&& targetCell == 0` is the rest of the original's test**,
                // and it is what makes the player's fire arrow survive
                // `Order_StopShortOfTarget`. `BattleMan_StateCloseToAttack`
                // (`0x00484BF9`) opens on
                // `other.owner == 0 && unit.targetCell == 0`, and that arm is
                // the one that *leaves* state 17 — by writing **state 5**,
                // which answers [`Self::fire_tick`]'s open question about what
                // puts a figure into the firing state.
                //
                // fire-arrow cell holds its bowmen in 17 with no live target,
                // and they acquire one there. `[V]`.
                let cell = {
                    let u = self.sim.figures[sim].unit as usize;
                    if (1..=MAX_UNITS).contains(&u) { self.units.get(u).target_cell } else { 0 }
                };
                if !alive {
                    if cell == 0 {
                        self.sim.figures[sim].state = State::Idle;
                        self.sim.figures[sim].target = None;
                        return;
                    }
                    let Some(shot) = WeaponClass::for_troop(self.fighters[i].troop) else {
                        return;
                    };
                    match self.missile_target(i, shot.stats().range as i32) {
                        Some(t) => self.sim.figures[sim].target = Some(t),
                        None => return,
                    }
                }
                // **`BattleMan_StateCloseToAttack` (`0x00484BF9`) looses.** Its
                // cadence is `BattleMan_FireMissile`'s own pair — swingTimer
                // `+0xE0` against reloadInterval `+0xCB`, `interval < timer`
                // shoots and resets — with no ten-tick acquisition, because the
                // target was chosen when the state was entered. It also sets
                // unit `+0x0F` to 0x78 and the unit's destination to the
                // shooter's own cell; neither is built.
                let Some(class) = WeaponClass::for_troop(self.fighters[i].troop) else {
                    return;
                };
                let counter = {
                    let f = &mut self.sim.figures[sim];
                    f.reload_counter = f.reload_counter.saturating_add(1);
                    f.reload_counter
                };
                if counter <= class.stats().reload {
                    return;
                }
                self.sim.figures[sim].reload_counter = 0;
                let Some(t) = self.sim.figures[sim].target else {
                    return;
                };
                let Some(t) = self.fighters.iter().position(|f| f.sim == t) else {
                    return;
                };
                self.loose(i, class, (self.fighters[t].x, self.fighters[t].y));
            }
            _ => {}
        }
    }


    /// **`BattleMan_FireMissile` (`0x00483337`)** — one tick of a figure that
    /// carries a bow, a crossbow or a catapult.
    ///
    /// Marked `[I]`; the mechanism it drives is `[V]` throughout.
    fn fire_tick(&mut self, i: usize) {
        self.reload_tick(i);
        let sim = self.fighters[i].sim;
        if self.sim.figures[sim].target.is_some() {
            self.shoot(i);
        }
    }

    fn reload_tick(&mut self, i: usize) {
        let Some(class) = WeaponClass::for_troop(self.fighters[i].troop) else {
            return;
        };
        let stats = class.stats();
        let sim = self.fighters[i].sim;
        let counter = {
            let f = &mut self.sim.figures[sim];
            f.reload_counter = f.reload_counter.saturating_add(1);
            f.reload_counter
        };

        if counter + missile::ACQUIRE_LEAD == stats.reload {
            match self.missile_target(i, stats.range as i32) {
                Some(t) => self.sim.figures[sim].target = Some(t),
                None => {
                    let f = &mut self.sim.figures[sim];
                    f.target = None;
                    f.reload_counter = missile::NO_TARGET_RESET;
                }
            }
            return;
        }
        if counter <= stats.reload {
            return;
        }
        self.sim.figures[sim].reload_counter = 0;
        let Some(target) = self.sim.figures[sim].target else {
            return;
        };
        if !self.sim.figures[target].is_alive() {
            return;
        }
        let Some(t) = self.fighters.iter().position(|f| f.sim == target) else {
            return;
        };
        self.loose(i, class, (self.fighters[t].x, self.fighters[t].y));
    }

    fn loose(&mut self, shooter: usize, class: WeaponClass, at: (u8, u8)) {
        let sim = self.fighters[shooter].sim;
        let (owner, band) = {
            let f = &self.sim.figures[sim];
            (f.owner, f.strength_band())
        };
        let here = (self.fighters[shooter].x, self.fighters[shooter].y);
        let elevation = self.field.cells[here.1 as usize * DIM + here.0 as usize].elevation;
        let power = missile::band_scaled(class.stats().damage, band);
        let Some(slot) = missile::spawn(
            &mut self.missiles,
            owner,
            class,
            sim,
            here,
            at,
            power,
            elevation,
        ) else {
            return;
        };
        self.sim.cues.loose(class);
        // **A fire arrow.** `BattleMan_FireMissile` writes `+0x44 = 1` when the
        // shooter's unit is an AI's, of side 0, and more than three of a
        // human's figures are standing in woodland — the garrison sets the
        // wood alight under a player who has hidden an army in it. Before the
        // launch steps, so an arrow loosed from inside the wood lights its
        // own cell. The catapult fires from `BattleMan_StateEngineFire`, which
        // has no such write.
        let unit = self.sim.figures[sim].unit as usize;
        if self.sim.figures[sim].state == State::Shooting {
            // **The player's fire arrow.** `BattleMan_StateCloseToAttack`
            // (`0x00484BF9`) copies the unit's `targetCell` (`+0x30`) onto
            // every arrow it looses, unconditionally — no woodland count and
            // no side test, because `BattleUnit_Order` has already refused to
            // write `targetCell` for anyone else.
            if (1..=MAX_UNITS).contains(&unit) {
                self.missiles.get_mut(slot).fire_arrow = self.units.get(unit).target_cell;
            }
        } else if class != WeaponClass::Catapult && self.humans_in_woods > fire::FIRE_ARROWS_ABOVE {
            if (1..=MAX_UNITS).contains(&unit)
                && !self.units.get(unit).human
                && self.units.get(unit).side == SIDE_A
            {
                self.missiles.get_mut(slot).fire_arrow = 1;
            }
        }
        for _ in 0..missile::LAUNCH_STEPS {
            if !self.step_missile(slot) {
                return;
            }
        }
    }

    /// `Missile_FindTarget` (`0x004956CC`) as the firing path uses it.
    fn missile_target(&self, i: usize, range: i32) -> Option<usize> {
        let me = self.fighters[i].sim;
        let mine = self.sim.figures[me].owner;
        let (sx, sy) = (self.fighters[i].x as i32, self.fighters[i].y as i32);
        let mut best: Option<(i32, usize)> = None;
        for j in 0..self.fighters.len() {
            let sim = self.fighters[j].sim;
            let f = &self.sim.figures[sim];
            if !f.is_alive() || f.owner == mine || f.owner == 0 {
                continue;
            }
            let (dx, dy) = (
                (self.fighters[j].x as i32 - sx).abs(),
                (self.fighters[j].y as i32 - sy).abs(),
            );
            if dx > range || dy > range {
                continue;
            }
            let mut score = (dx + dy).min(160);
            if f.troop.is_siege() {
                score += 35;
            }
            if score >= 160 {
                continue;
            }
            if best.is_none_or(|(b, _)| score < b) {
                best = Some((score, sim));
            }
        }
        best.map(|(_, j)| j)
    }

    /// **`Missile_UpdateAll` (`0x00485BB1`)** — every live missile, ascending
    /// slot, then the countdown that retires the ones that have hit.
    pub(super) fn update_missiles(&mut self) {
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
    pub(super) fn step_missile(&mut self, slot: usize) -> bool {
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
            if m.ticks_flown > m.range_ticks {
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

    fn missile_cell_tests(&mut self, slot: usize) -> bool {
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
        let Some(victim) = self.occupant[cell] else {
            return true;
        };
        let victim = victim as usize;
        let vsim = self.fighters[victim].sim;
        if !self.sim.figures[vsim].is_alive() {
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
}
