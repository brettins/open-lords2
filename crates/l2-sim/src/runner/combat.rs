use super::*;

#[cfg(test)]
#[path = "tests_delay.rs"]
mod tests_delay;

impl BattleRunner {
    /// **`Missile_Step` does not raise the breach score itself** — the brief
    /// said it did, and it is one level removed. The shot adds one to the cell's
    /// own counter; only when that counter passes
    /// [`missile::WALL_HITS_PER_COLLAPSE`] does the cell collapse,
    /// collapse is what scores. The original's collapse routine
    /// (`FUN_0047DFE0`) then adds **one per orthogonal neighbour that is still
    /// rampart**,
    /// into its end.
    pub(super) fn strike_wall_with_shot(&mut self, slot: usize, cell: usize) {
        // ```c
        // if (cell.elevation < 4) { shooter.engaged = 0; cell.terrain++;
        //     if (0xF < cell.terrain) Wall_Collapse(cell); FUN_004262CF(0xF); }
        // else                    { shooter.engaged = 1; Sound_PlaySlot(0x10); }
        // ```
        //
        // **A rampart four or more high cannot be shot down**: the shot is not
        // counted,
        // — which `BattleMan_StateEngineFire` reads as "move to the approach
        // lane and try again", a state this crate does not model, so that one
        // write has nothing to land on. Either arm leaves debris. `[V]`.
        let high = self.field.cells[cell].elevation >= missile::WALL_TOO_HIGH;
        if high {
            self.sim.cues.wall_missed();
        } else {
            self.sim.cues.wall_struck();
            let t = &mut self.field.cells[cell].terrain;
            *t = t.saturating_add(1);
        }
        if !high && self.field.cells[cell].terrain > missile::WALL_DAMAGE_MAX {
            // `Wall_Collapse` (`FUN_0047DFE0`) — surface 9, flags 2, elevation
            // 0, and one point of breach score *and* one of wall damage for
            // each of the four orthogonal neighbours still at
            // [`crate::siege::SURFACE_BAILEY`]. Those two are the *same* count,
            // written in one statement per neighbour — `docs/bugs.md` B69.
            let score = crate::siege::collapse_wall(&mut self.field, &mut self.siege, cell);
            self.rebuild_blocked();
            self.ai.breach_score += score;
            self.ai.approach_score += score;
            // `FUN_0048EE46` files each billed neighbour in the twenty-entry
            // defence-post table the garrison's handlers claim from. It is the
            // table's **only** appender,
            // defence posts at all.
            self.register_defence_posts(cell);
        }
        let m = self.missiles.get_mut(slot);
        m.class = missile::CLASS_DEBRIS;
        m.ttl = missile::DEBRIS_TTL;
        m.sub_steps = 1;
        m.dx = 0;
        m.dy = 0;
        // `m[+0x0A] -= 0x10; m[+0x0C] -= 0x10;` — the last two statements of
        // the arm. Sixteen thirty-seconds is **half a cell**, so the rubble
// pile is drawn up and left of the masonry it came off
        // centred on it.
        m.x -= missile::DEBRIS_NUDGE;
        m.y -= missile::DEBRIS_NUDGE;
    }

    /// **Lower the drawbridge** — `FUN_00496B9F` (`0x00496B9F`), from the
    /// battlefield's third button.
    ///
    /// // arm: 0x00496B9F/lower-drawbridge left-press
    pub fn lower_drawbridge(&mut self) -> bool {
        if !self.siege.is_siege {
            return false;
        }
        let Some(_anchor) = crate::siege::lower_drawbridge(&mut self.field, &mut self.siege) else {
            return false;
        };
        self.ai.approach_score += crate::siege::GATE_BREACH_SCORE;
        self.ai.breach_score += crate::siege::GATE_BREACH_SCORE;
        for (c, cell) in self.field.cells.iter().enumerate() {
            self.blocked[c] = cell.impassable();
        }
        self.refresh_ai_surfaces();
        true
    }

    /// **What this siege did to the castle**, for
    /// `Siege_RecordCastleDamage` (`0x004784CA`) on the campaign side.
    pub fn castle_damage(&self) -> crate::siege::CastleDamage {
        crate::siege::CastleDamage {
            moat_filled: self.siege.moat_filled,
            wall_damage: self.siege.wall_damage,
            breach_score: self.ai.breach_score,
            approach_score: self.ai.approach_score,
            ramparts_breached: self.siege.ramparts_breached.min(u8::MAX as u32) as u8,
            gate_open: self.siege.gate_breached,
        }
    }

    /// **What the last siege on this castle left** — `FUN_004787A4`
    /// (`0x004787A4`), the **last statement but one of
    /// `Battlefield_BuildCastle`**, so it overwrites the fresh scores
    /// `Battle_Start` had just written.
    ///
    /// > **It restores the *numbers* and not the field.** The moat is water
    /// > again, the breaches are walls again, and only the six counters carry
    /// > over — plus the castle *level*, which `assault_castle_level` lowers on
    /// > the campaign side. That asymmetry is the original's and it has a
    /// > consequence worth naming: because the two accumulators come back
    /// > non-zero, the **next** assault's `Siege_RecordCastleDamage` fires even
    /// > if nothing new is damaged, and bills the same repair a second time on
    /// > top of the first. `docs/bugs.md` `B84`. `[V]` on the
    /// > round trip, `[I]` that nobody meant the double bill.
    pub fn restore_castle_damage(&mut self, d: crate::siege::CastleDamage) {
        self.siege.moat_filled = d.moat_filled;
        self.siege.wall_damage = d.wall_damage;
        self.siege.ramparts_breached = d.ramparts_breached as u32;
        self.siege.gate_breached = d.gate_open;
        self.ai.breach_score = d.breach_score;
        self.ai.approach_score = d.approach_score;
    }

    /// Whether this battlefield has a drawbridge at all — what
    /// `FUN_00496B9F`'s scan is looking for, asked without spending the latch.
    pub fn has_drawbridge(&self) -> bool {
        self.field.cells.iter().any(|c| c.flags & crate::siege::FLAG_DRAWBRIDGE != 0)
    }

    /// **`BattleMan_StateFillMoat` (`0x00483FE1`)** — slot 9 of
    /// `g_manStateTable`, one figure, one frame.
    ///
    /// > **A moat cell takes four loads, not fifteen**,
    /// > the counter does not start at zero. `Battlefield_BuildCastle` writes
    /// > `terrain = 11` — the water id — into every moat cell and `terrain = 1`
    /// > into everything else (`docs/battle.md` §3.0), and *this* is what reads
    /// > the byte back: it counts it up to `g_moatFillSteps`, which is 15. So
    /// > the cell's own terrain id is the fill counter's starting value, and a
    /// > moat is four loads deep at 101 frames each. `[V]` on both numbers,
/// > `[I]` that the reuse is deliberate — on a
    /// > castle battlefield the terrain byte holds nothing else.
    ///
    /// ```c
    /// if (!latched) {
    ///     if (FUN_004926FB(cur) == 0) { state = 5; }        /* no ditch left  */
    ///     else { Anim_Walk(); BattleMan_Step(0); … }        /* go to the next */
    /// }
    /// ```
    ///
    /// `FUN_004926FB` answers 1 if the figure's own destination is already
    /// water, and otherwise hunts radii 1…19 for a cell that is and **retargets
    /// the figure at it**.
    pub(super) fn fill_moat_tick(&mut self, i: usize) -> bool {
        let sim = self.fighters[i].sim;
        if self.sim.figures[sim].state != State::FillingMoat {
            return false;
        }
        let unlatch = |s: &mut Self| {
            s.fighters[i].moat_cell = None;
            s.fighters[i].moat_load = 0;
        };
        let Some(cell) = self.fighters[i].moat_cell.map(|c| c as usize) else {
            let (tx, ty) = self.fighters[i].target;
            if self.field.at(tx as usize, ty as usize).surface == crate::siege::SURFACE_WATER {
                return false;
            }
            match self.nearest_water(self.fighters[i].x, self.fighters[i].y) {
                Some((x, y)) => {
                    if self.fighters[i].target != (x, y) {
                        self.fighters[i].target = (x, y);
                        self.fighters[i].path.clear();
                        self.fighters[i].barred = 0;
                    }
                }
                None => self.sim.figures[sim].state = State::Idle,
            }
            return false;
        };
        if self.field.cells[cell].surface != crate::siege::SURFACE_WATER {
            unlatch(self);
            return false;
        }
        // `Anim_Dying` is the animation the original plays here, and this is
        // its only caller in the whole binary — a man bent double over a
        // shovel, reused. **Not the corpse pose**: state 2 runs
        // `Anim_Collapse` and nothing else (`00480000.c:1337`), so the two are
        // different bands of the sheet — [`BattleRunner::shovel`].
        self.shovel(i);
        if self.field.cells[cell].terrain < crate::siege::MOAT_FILL_STEPS {
            let human = self.sim.figures[self.fighters[i].sim].owner_is_human;
            let per_load = if human {
                crate::siege::MOAT_TICKS_PER_LOAD_HUMAN
            } else {
                crate::siege::MOAT_TICKS_PER_LOAD_AI
            };
            self.fighters[i].moat_load = self.fighters[i].moat_load.saturating_add(1);
            if self.fighters[i].moat_load > per_load {
                self.fighters[i].moat_load = 0;
                self.field.cells[cell].terrain += 1;
            }
            return true;
        }
        self.field.cells[cell].terrain = 0;
        let score = crate::siege::fill_moat_cell(&mut self.field, &mut self.siege, cell);
        self.ai.approach_score += score;
        self.blocked[cell] = self.field.cells[cell].impassable();
        self.refresh_ai_surfaces();
        unlatch(self);
        true
    }

    /// `FUN_00496768` through `FUN_004926FB` — the **nearest cell of surface 2**
    /// within nineteen, by the expanding-radius scan `Siege_FindCellSurface5`
    /// uses for the rampart.
    fn nearest_water(&self, x: u8, y: u8) -> Option<(u8, u8)> {
        let (x, y) = (x as i32, y as i32);
        for radius in 1..20i32 {
            let mut best: Option<(i32, u8, u8)> = None;
            for cy in (y - radius).max(0)..=(y + radius).min(DIM as i32 - 1) {
                for cx in (x - radius).max(0)..=(x + radius).min(DIM as i32 - 1) {
                    if self.field.at(cx as usize, cy as usize).surface
                        != crate::siege::SURFACE_WATER
                    {
                        continue;
                    }
                    let d = (cx - x).abs() + (cy - y).abs();
                    if best.is_none_or(|(b, _, _)| d < b) {
                        best = Some((d, cx as u8, cy as u8));
                    }
                }
            }
            if let Some((_, cx, cy)) = best {
                return Some((cx, cy));
            }
        }
        None
    }

    /// `Melee_ChooseChaseTarget` (`0x004954DD`): lowest score wins, where the
    /// score is the Chebyshev distance, **halved** if the enemy carries a
    /// missile weapon, plus that enemy's `targeted` count. No range limit at
    /// all, and siege engines are never chosen.
    pub(super) fn chase_target(&self, i: usize) -> Option<usize> {
        let me = self.fighters[i].sim;
        let mine = self.sim.figures[me].owner;
        let (mx, my) = (self.fighters[i].x as i16, self.fighters[i].y as i16);
        let mut best: Option<(i32, usize)> = None;
        for j in 0..self.fighters.len() {
            let sim = self.fighters[j].sim;
            let f = &self.sim.figures[sim];
            if !f.is_alive() || f.owner == mine || f.owner == 0 || f.troop.is_siege() {
                continue;
            }
            let mut score = chebyshev(mx, my, self.fighters[j].x as i16, self.fighters[j].y as i16);
            if WEAPON_CLASS[f.troop.index()] != 0 {
                score /= 2;
            }
            score += f.targeted as i32;
            if best.is_none_or(|(bs, _)| score < bs) {
                best = Some((score, sim));
            }
        }
        best.map(|(_, j)| j)
    }

    /// `BattleUnit_JoinMelee`: contact drops the rest of an AI unit into free
    /// pursuit. **[D]** `docs/battle-ai.md` §4.2 — skipped for a human unit, a
    /// missile unit, or a unit holding an order lock, which is the single
    /// biggest visible difference between how the two sides fight.
    pub(super) fn join_melee(&mut self, unit: usize) {
        if unit == 0 || unit > MAX_UNITS {
            return;
        }
        let u = *self.units.get(unit);
        if !u.is_live() || u.human || u.category == 1 || u.order_lock != 0 {
            return;
        }
        for j in 0..self.fighters.len() {
            let sim = self.fighters[j].sim;
            let f = &mut self.sim.figures[sim];
            if f.unit as usize != unit || !f.is_alive() || f.state == State::Melee {
                continue;
            }
            f.state = State::Chasing;
            f.target = None;
            self.fighters[j].path.clear();
            self.fighters[j].barred = 0;
        }
    }

    /// * **A besieger stepping onto a bridge sets it alight.** `Cell_TryEnter`'s
    ///   first statement is `if (surface == 7 && side == 4) FUN_0048551D(cell)`,
    ///   before it has decided anything, and it decides on the flags and height
    ///   it read *before* the fire cleared them. So the fire is lit after the
    ///   decision here, which is the same answer. Engines use
    ///   `Cell_TryEnterEngine`, which has no such test.
    pub(super) fn enter(&mut self, i: usize, next: Pos) {
        let troop = self.fighters[i].troop;
        if troop == Troop::SiegeTowers && self.tower_step(i, next) {
            return;
        }
        let dst = next.y as usize * DIM + next.x as usize;
        let lights = self.fighters[i].side == SIDE_B
            && !fire::is_engine(troop)
            && self.field.cells[dst].surface == fire::SURFACE_BRIDGE;
        self.enter_cell(i, next);
        if lights {
            self.bridge_fire_at(next.x as i32, next.y as i32);
        }
    }

    fn enter_cell(&mut self, i: usize, next: Pos) {
        let dst = next.y as usize * DIM + next.x as usize;
        if self.siege.is_siege && self.strike_castle(i, dst) {
            return;
        }
        // `docs/battle.md` §7: *"a step is only allowed when the two cells'
        // elevations differ by at most 1, unless the destination's elevation is
        // exactly 5"*, marked `[V]`. [`crate::movement::can_step_elevation`]
        // has said so since it was written and **nothing called it** — the
        // pathfinder enforced the rule,
        // walking straight at its target (which is what a figure does when the
        // line is clear, and no search ever runs) climbed cliffs. It is inert
        // on a `.skr` field, where every cell is at elevation 0, and it is the
        // difference between a castle wall and a ramp everywhere else.
        {
            let src = self.fighters[i].y as usize * DIM + self.fighters[i].x as usize;
            let (a, b) =
                (self.field.cells[src].elevation as i32, self.field.cells[dst].elevation as i32);
            if !crate::movement::can_step_elevation(a, b) {
                self.request_path(i);
                return;
            }
        }
        if self.blocked[dst] {
            // ```c
            // if (state == 9 && DAT_004EEA94 == 2) {
            //     man.field_0x190 = DAT_004EEAD4;   /* the cell it could not enter */
            //     man.field_0x18F = 1;
            //     return 0;
            // }
            // ```
            if self.sim.figures[self.fighters[i].sim].state == State::FillingMoat {
                self.fighters[i].moat_cell = Some(dst as u32);
                self.fighters[i].moat_load = 0;
                return;
            }
            self.request_path(i);
            return;
        }
        match self.occupant[dst] {
            None => {
                let src = self.fighters[i].y as usize * DIM + self.fighters[i].x as usize;
                if self.occupant[src] == Some(i as u16) {
                    self.occupant[src] = None;
                }
                self.occupant[dst] = Some(i as u16);
                let f = &mut self.fighters[i];
                f.x = next.x;
                f.y = next.y;
                if f.path.last() == Some(&next) {
                    f.path.pop();
                }
                f.barred = 0;
                f.hold = 0;
                // `barred = 0; holdIt = 0; stepFlags &= ~1; dirc = dir;
                // walking = 1; FUN_00491b1f(man)`. The move above is
                // `FUN_00491B1F`; this is the counter, and from here the
                // figure's cell is the one it is crossing *to* — which is what
                // `BattleFigure_Draw` trails him behind.
                f.progress.begin_crossing();
                self.march(i, true);
            }
            Some(other) => {
                let other = other as usize;
                if self.fighters[other].side == self.fighters[i].side {
                    // `BattleMan_Step` (`0x0048F1DD`, `00480000.c:6441`): an
                    // engine skips the swap arm, so `local_10` stays 0 and
                    // `00480000.c:6495` — `if ((local_10 == 1) ||
                    // (isSiegeEngine == 0))` — hands it to the engine arm
                    // instead of the side-step and the search. **[V]**.
                    //
                    // That arm, `00480000.c:6560-6567`: `FUN_00491492` (return
                    // if non-zero), `local_10 = FUN_004912EC`, then `if
                    // (field_0x169 < 3) { tgX = mapX; tgY = mapY; return 0; }`.
                    //
                    // `FUN_004912EC` — the engine's own side-step, three
                    // rotations each way — is not built, so `local_10` is never
                    // 1 here and the tail parks him (`00480000.c:6583-6586`).
                    //
                    // `[D]` on the missing step; [`Self::tower_step`] is the
                    // same arm reached from [`Self::enter`] for a tower.
                    if self.fighters[i].troop.is_siege() {
                        let f = &mut self.fighters[i];
                        if chebyshev(f.x as i16, f.y as i16, f.target.0 as i16, f.target.1 as i16)
                            < 3
                        {
                            f.target = (f.x, f.y);
                            f.path.clear();
                            return;
                        }
                        f.delay = 100;
                        return;
                    }
                    // `00480000.c:6443`: the swap arm needs `other.state != 2
                    // && cur.unit == other.unit`; a dead or cross-unit blocker
                    // is `local_10 = 3`, the side-step. **[V]**, decompiled.
                    let swap_arm =
                        self.is_alive(other) && self.unit_of(other) == self.unit_of(i);
                    match if swap_arm { self.swap_answer(i, other) } else { 1 } {
                        2 => self.swap_places(i, other),
                        // `00480000.c:6445,6459-6463`: `cur.onRoute = 0` ahead
                        // of the call, then `delayState = state; state = 1;
                        // delay = (other & 1) + 1; return 0` — he stands one or
                        // two frames and asks for no route. **[V]**, decompiled.
                        //
                        // Ours searched anyway; C103's deadlock fix is about
                        // `Path_LineIsClear` and does not cover this arm.
                        0 => {
                            self.fighters[i].path.clear();
                            self.fighters[i].delay = (other & 1) as u8 + 1;
                        }
                        _ => self.request_path(i),
                    }
                } else if self.is_alive(other) {
                    let (ua, ub) = (self.unit_of(i), self.unit_of(other));
                    self.join_melee(ua);
                    self.join_melee(ub);
                    // `BattleMan_Step`'s 999 arm locks the pot into state 4
                    // with him as its opponent — it tests the walker's troop,
                    // not the pot's —
                    // with `if (troopType == 10) FUN_0047A814(pot,
                    // opponent.mapX, opponent.mapY)`. Poured here at the
// contact: the
                    // walker has not moved, so the cell is the same. `[D]` on
                    // the frame. A walker that is itself oil, or an engine,
                    // never reaches the 999 arm.
                    if self.fighters[other].troop == Troop::Oil && self.fighters[i].troop.index() < 7 {
                        let at = (self.fighters[i].x, self.fighters[i].y);
                        self.pour_oil(other, at);
                        return;
                    }
                    let (a, b) = (self.fighters[i].sim, self.fighters[other].sim);
                    self.sim.engage(a, b);
                    let facing = self.fighters[i].facing;
                    self.strike(i, facing);
                } else {
                    self.occupant[dst] = None;
                }
            }
        }
    }

    /// ```c
    /// if (flags & 0x40) { /* drawbridge: a hole in the wall once it is down */ }
    /// if (flags & 0x20) return (side == 0) ? 1 : 5;   /* wall: 5 -> state 6  */
    /// if (flags & 0x08) { if (side == 4) { DAT_00553F3C = 1; return 2; } return 0; }
    /// ```
    pub(super) fn strike_castle(&mut self, i: usize, dst: usize) -> bool {
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
    fn register_defence_posts(&mut self, cell: usize) {
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

    fn rebuild_blocked(&mut self) {
        for (c, cell) in self.field.cells.iter().enumerate() {
            self.blocked[c] = cell.impassable();
        }
        self.refresh_ai_surfaces();
    }

    fn refresh_ai_surfaces(&mut self) {
        for (c, cell) in self.field.cells.iter().enumerate() {
            self.ai_field.surface[c] = cell.surface;
            self.ai_field.elevation[c] = cell.elevation;
        }
    }

    /// **`BattleMen_SwapPlaces` (`0x0049005F`) as its answer**: 2 exchange,
    /// 0 stand and wait, 1 fall through to the side-step.
    fn swap_answer(&self, i: usize, other: usize) -> u8 {
        // **`00490000.c:21-23` is held out.** `if (other.ownerIsHuman == 0) {
        // if (!siege) return 1; … }` — an AI-owned blocker is side-stepped and
        // never swapped with. Landing it turns the seam's militia from 4 of 8
        // into 3 of 8 (`seam::the_same_position_can_be_fought_for_real…`,
        // measured on this branch), so the whole guard stays out, its two siege
        // sub-guards (`cur.side == 4`, `cur.weaponClass == 0`,
        // `00490000.c:24-29`) with it. `[D]`.
        //
        // `00490000.c:31-33`: `cur.troopType == other.troopType &&
        // other.troopType < 7 && cur.troopType < 7`, each failure answering 0.
        let (cur, oth) = (self.fighters[i].troop, self.fighters[other].troop);
        if cur != oth || cur.index() >= 7 || oth.index() >= 7 {
            return 0;
        }
        // `00490000.c:34-41`: `other.state` 2, 3 and 8 answer 0; 3 is walking
        // and `progress.free` is our reading of it, `delay` of state 1. `[D]`
        // on the reading, **[V]** on the refusal. A man already standing out a
        // swap is not swapped again: two swaps in one tick moved a man two
        // cells between drawn frames, 28 of them at 64 px in
        // `no_drawn_man_ever_jumps_half_a_cell_in_one_tick`.
        if !self.fighters[other].progress.free || self.fighters[other].delay != 0 {
            return 0;
        }
        // `00490000.c:44-51`: `other.state` 4 (44-45), 9 (47-48) and 6 (50-51)
        // answer 1 — stepped
        // around, never pulled out of the cell. 9 is `BattleMan_StateFillMoat`
        // (`0x00483FE1`), landed here; **4, melee, is held out** — it turns the
        // seam's militia from 4 of 8 into 1 of 8
        // (`seam::the_same_position_can_be_fought_for_real…`, measured on this
        // branch), so a comrade in melee is still swapped with. We carry
        // nothing for 6. `[D]` on both.
        let busy = self.sim.figures[self.fighters[other].sim].state;
        if busy == State::FillingMoat {
            return 1;
        }
        // `00490000.c:53-55`: `if (cur.tgX == other.tgX && cur.tgY ==
        // other.tgY) return 0` — two men with one destination are never
        // swapped. Ours had that test the other way round, and with the
        // side-step landed it is what two men of a marching unit do instead of
        // shuffling: exchange cells every other tick for ever — 294 drawn jumps
        // of 32 px in `no_drawn_man_ever_jumps_half_a_cell_in_one_tick`, and
        // the jitter `no_man_alternates_between_two_cells` names. **[V]**.
        if self.fighters[i].target == self.fighters[other].target {
            return 0;
        }
        2
    }

    fn swap_places(&mut self, a: usize, b: usize) {
        if a == b {
            return;
        }
        let (ax, ay) = (self.fighters[a].x, self.fighters[a].y);
        let (bx, by) = (self.fighters[b].x, self.fighters[b].y);
        self.fighters[a].x = bx;
        self.fighters[a].y = by;
        self.fighters[b].x = ax;
        self.fighters[b].y = ay;
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
