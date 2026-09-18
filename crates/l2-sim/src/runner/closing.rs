use super::*;

/// What state 17 holds an AI unit's reform countdown at — `reTarg = 0x32`,
/// `BattleMan_StateCloseToAttack` (`0x00484BF9`, `00480000.c:1995-1998`).
/// `Battle_UpdateAllUnits` has already counted it down by then, so a shooting
/// unit's reform never reaches zero. `[V]`
pub const CLOSING_REFORM: i16 = 0x32;

impl BattleRunner {
    /// **`BattleMan_StateCloseToAttack` (`0x00484BF9`)** — state 17, one tick.
    ///
    /// Returns `true` when the figure walks this tick — every arm that calls
    /// `BattleMan_Step(1)` — and `false` when it stands.
    ///
    /// ```text
    /// :1995  reTarg>0, unit not human, figure not engaged -> reTarg = 0x32
    /// :1999  other.owner == 0 && targetCell == 0 -> own cell; stepFlags&2: state 5
    ///        other.owner == 0 && targetCell != 0 -> aim at figure 0, the cell
    /// :2013  stepFlags & 1 clear                  -> keep walking
    /// :2025  range>>3 < chebyshev + 2             -> walk at it (own cell if
    ///                                               movStraff); engaged = 1
    /// :2043  otherwise                            -> face; swingTimer++; loose
    /// ```
    ///
    /// It never calls `BattleMan_FireMissile`: it shoots from its range **less
    /// two** cells — 13 for a bow, 6 for a crossbow — and `movStraff` holds it
    /// where it shot from. `[V]` throughout.
    ///
    /// `[D]` "between cells" is `progress.free` clear rather than the
    /// original's `stepFlags & 1`, because ours accumulates [`Progress`] and
    /// moves at the end of a step where the original moves at the start.
    pub(crate) fn close_to_attack_tick(&mut self, i: usize) -> bool {
        let sim = self.fighters[i].sim;
        let unit = self.sim.figures[sim].unit as usize;
        let has_unit = (1..=MAX_UNITS).contains(&unit);

        // `:1995-1998`
        if has_unit {
            let engaged = self.sim.figures[sim].engaged;
            let u = self.units.get_mut(unit);
            if u.reform > 0 && !u.human && !engaged {
                u.reform = CLOSING_REFORM;
            }
        }
        let cell = if has_unit { self.units.get(unit).target_cell } else { 0 };

        let mid_step = !self.fighters[i].progress.free;
        let Some((tx, ty)) = self.closing_aim(i, cell) else {
            if mid_step {
                return true;
            }
            // `:2000-2010`: at its own cell with nothing to shoot, `stepFlags &
            // 2`, state 5.
            let f = &mut self.fighters[i];
            f.target = (f.x, f.y);
            f.anim = Motion::Idle;
            self.sim.figures[sim].state = State::Idle;
            return false;
        };
        // `:2013`
        if mid_step {
            return true;
        }

        let (sx, sy) = (self.fighters[i].x, self.fighters[i].y);
        // `Dist_Chebyshev`, `:2021`.
        let distance = (tx as i32 - sx as i32).abs().max((ty as i32 - sy as i32).abs());
        let class = WeaponClass::for_troop(self.fighters[i].troop);
        // `range >> 3`, in cells. No weapon is range 0, so it walks for ever.
        let range = class.map_or(0, |c| c.stats().range as i32);
        // `:2025-2039`
        if range < distance + 2 {
            let straffed = self.sim.figures[sim].straffed;
            self.fighters[i].target = if straffed { (sx, sy) } else { (tx, ty) };
            self.sim.figures[sim].engaged = true;
            return true;
        }

        // `:2043-2047`
        {
            let f = &mut self.fighters[i];
            if let Some(fc) = facing_from_delta(tx as i32 - sx as i32, ty as i32 - sy as i32) {
                f.facing = fc;
            }
            // `:2106` `Anim_DrawBow()` ends the arm every tick (oracle check 2026-09-17).
            f.anim = Motion::Shooting;
            f.facing_drawn = f.facing;
            f.progress = Progress::default();
        }
        let Some(class) = class else {
            return false;
        };
        let counter = {
            let f = &mut self.sim.figures[sim];
            f.reload_counter = f.reload_counter.saturating_add(1);
            f.reload_counter
        };
        // `:2049`, `reloadTicks < swingTimer`; `:2048` is the step.
        if counter <= class.stats().reload {
            return false;
        }
        // `:2050-2057`. The unit's destination becomes the shooter's own cell,
        // so its next reform forms it up where it last shot from.
        {
            let f = &mut self.sim.figures[sim];
            f.engaged = false;
            f.reload_counter = 0;
            f.straffed = true;
        }
        if has_unit {
            let u = self.units.get_mut(unit);
            u.target_x = sx as i16;
            u.target_y = sy as i16;
        }
        self.loose_from(i, class, (tx, ty), true);
        false
    }

    /// **What state 17 aims at** — `g_battleMen[g_otherBattleMan]`'s
    /// `mapX`/`mapY` at `:2021-2024`, `:2032-2033` and `:2059-2060`.
    ///
    /// `None` is `:1999`, `other.owner == 0 && targetCell == 0`: `owner != 0`
    /// is alive, or a body whose slot `BattleMan_StateDead` has not freed yet,
    /// which is why a shooter keeps aiming at a corpse for 80 frames.
    ///
    /// **The fire-at-ground order.** `target == 0` with `targetCell != 0` is
    /// figure slot 0, which `BattleUnit_Order` (`0x00479E90`,
    /// `00470000.c:4291-4292`) parks on the woodland cell the player clicked
    /// beside the `targetCell` it writes at `:4290`. Nobody stands there, so
    /// the volley needs that phantom to fly at; `Missile_Step`
    /// (`00490000.c:1232`) clears its `owner` when the cell lights.
    ///
    /// `[D]` ours decodes the unit's own `targetCell` — the same `x` and `y`
    /// `:4290` and `:4291` take — where the original keeps one global slot 0.
    /// The two differ only while two units hold woodland orders at once, when
    /// the original aims both at the later cell.
    fn closing_aim(&self, i: usize, cell: i32) -> Option<(u8, u8)> {
        let other = self.sim.figures[self.fighters[i].sim].target;
        let k = other.and_then(|t| self.fighters.iter().position(|f| f.sim == t));
        if !k.is_some_and(|k| !self.corpse_gone(k)) && cell == 0 {
            return None;
        }
        // A freed slot keeps its `mapX`/`mapY`; ours is the fighter where he
        // fell.
        Some(match k {
            Some(k) => (self.fighters[k].x, self.fighters[k].y),
            None => crate::fire::cell_of_byte_offset(cell),
        })
    }

    /// `BattleUnit_Order` (`0x00479E90`)'s first loop: `movStraff = 0` for
    /// every figure of the unit — the only writer of zero in the binary. `[V]`
    pub(super) fn clear_straff(&mut self, unit: usize) {
        for f in self.sim.figures.iter_mut() {
            if f.unit as usize == unit {
                f.straffed = false;
            }
        }
    }
}
