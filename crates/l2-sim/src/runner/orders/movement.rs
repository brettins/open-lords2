#![allow(unused_imports)]
use super::*;
use super::slots::*;
use super::targeting::*;
use super::missiles::*;
use super::super::*;

impl BattleRunner {
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
    /// > the ditch has every one of its figures enter state 9;
    /// > `BattleMan_Step`'s state-9 arm then latches whatever impassable cell
    /// > stops it. `docs/decisions.md` `C81`. `[V]` — two functions and one
    /// > global.
    ///
    /// C81's "a dry slot beside it" is wrong: `Formation_FindNearbySlot`
    /// (`0x0048A38E`, `00480000.c:4077`) hands the moat cell itself back to
    /// every figure. Correction `C236`.
    pub(crate) fn send_figure(&mut self, unit: usize, fighter: usize, x: i32, y: i32, dest_is_water: bool) {
        let sim = self.fighters[fighter].sim;
        let gate = self.units.get(unit).reform_gate;
        let state = self.sim.figures[sim].state;
        // A figure already filling in the moat is left alone unless the unit's
        // `+0x13` gate is set.
        if state == State::FillingMoat && !gate {
            return;
        }
        let x = x.clamp(0, DIM as i32 - 1);
        let y = y.clamp(0, DIM as i32 - 1);
        let cell = self.field.at(x as usize, y as usize);
        let troop = self.fighters[fighter].troop;
        let owner = self.sim.figures[sim].owner;

        // **The occupant of the *unit's* destination, not of this slot.**
        // `DAT_005681FC`'s one writer is `Formation_RectIsClear`
        // (`0x00489F9D`, `00480000.c:3963`):
        // `DAT_005681fc = g_battlefield[targY * 0x50 + targX].figure`, cached
        // off the unit's `targX`/`targY` before a slot is assigned. `[V]` A
        // slot is by construction a cell a figure can walk to, so reading the
        // slot's occupant put almost nobody into state 17.
        let (ux, uy) = self.unit_dest(unit);
        let enemy_there = self.occupant[uy as usize * DIM + ux as usize].and_then(|o| {
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


    pub(crate) fn step_one(&mut self, i: usize) {
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

        // **State 1 stands and counts.** `BattleMan_StateDelay` (`0x00482F91`)
        // is slot 1 of `g_manStateTable`: `Anim_Stand()`, then `delay -= 1` and
        // at zero `state = delayState`, so a delayed man reaches neither the
        // melee search nor the mover. Three writers put him here —
        // `BattleMan_Step`'s swap arm ([`BattleRunner::swap_places`]), its
        // refused-swap wait and the blocked arm's `delay = 100`
        // ([`BattleRunner::request_path_with`]).
        //
        // Its `if (weaponClass != 0 && 5 < delay) BattleMan_FireMissile()` —
        // a waiting archer still shoots — is not modelled here. `[D]`.
        // state 4 overwrites state 1: g_manStateTable dispatches 0x004831D8, not the delay count [V]
        if self.fighters[i].delay > 0 && self.sim.figures[self.fighters[i].sim].state != State::Melee {
            self.fighters[i].delay -= 1;
            self.fighters[i].anim = Motion::Idle;
            return;
        }

        if self.fill_moat_tick(i) {
            return;
        }

        let sim = self.fighters[i].sim;
        if self.sim.figures[sim].state == State::Melee {
            let mutual = self.sim.figures[sim].opponent.is_some_and(|o| {
                // `owner == 0 / state == 2 -> 8; else if (state == 4) Melee_Tick(); else 8` 0x004831D8 [V]
                let other = &self.sim.figures[o];
                other.owner != 0 && other.state == State::Melee
            });
            if !mutual {
                self.sim.figures[sim].state = State::Chasing;
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
                // this tick overwrites it; returning instead turns the seam's
                // militia from 4 of 8 into 3 of 8
                // (`seam::the_same_position_can_be_fought_for_real…`).
                //
                // **The counter step stays behind with the return.** Not
                // because a handler animates a man once — `BattleMan_StateMelee`
                // runs `Anim_Strike` at `00480000.c:1384` and `Anim_Walk` at
                // `00480000.c:1396` in the one tick whose opponent is in state
                // 4 — but because this arm and the arm that acts below would
                // each step `animPhase`: `Anim_StrikeA2` (`0x00486249`,
                // `00480000.c:2510`) and then `Anim_WalkA2` (`00480000.c:2756`)
                // stepped `animPhase` twice in one tick — figure 3, 21 → 23,
                // read through `(phase + 23) % 24` by `battle_picture`'s
                // `a_marching_man_is_drawn_at_the_phase_he_ended_the_last_tick_with`.
                let facing = self.fighters[i].facing;
                let phase = self.fighters[i].phase;
                self.strike(i, facing);
                self.fighters[i].phase = phase;
            } else {
                // `else if (other.state == 4) { Melee_Tick();` 0x004831D8 00480000.c:1395 [V]
                self.sim.duel(sim);
                if !self.is_alive(i) {
                    return;
                }
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

        if self.retarget(i) {
            return;
        }

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

        if self.sim.figures[self.fighters[i].sim].state == State::Shooting {
            // **State 17 is its own handler.** It never reaches
            // `BattleMan_FireMissile`; it decides for itself whether to walk.
            if !self.close_to_attack_tick(i) {
                return;
            }
            if self.fighters[i].at_target() {
                self.fighters[i].anim = Motion::Idle;
                self.fighters[i].progress = Progress::default();
                return;
            }
        } else if self.fighters[i].at_target() {
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

}
