#![allow(unused_imports)]
use super::*;
use super::melee::*;
use super::castle::*;
use super::super::*;

impl BattleRunner {
    /// **`Missile_Step` does not raise the breach score itself** — the brief
    /// said it did, and it is one level removed. The shot adds one to the cell's
    /// own counter; only when that counter passes
    /// [`missile::WALL_HITS_PER_COLLAPSE`] does the cell collapse,
    /// collapse is what scores. The original's collapse routine
    /// (`FUN_0047DFE0`) then adds **one per orthogonal neighbour that is still
    /// rampart**,
    /// into its end.
    pub(crate) fn strike_wall_with_shot(&mut self, slot: usize, cell: usize) {
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
    pub(crate) fn fill_moat_tick(&mut self, i: usize) -> bool {
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

}
