#![allow(unused_imports)]
use super::*;
use super::deploy_part::*;
use super::stepping::*;
use super::super::*;

impl BattleRunner {
    /// `BattleUnit_Order` (`0x00479E90`) is the entry point for a click. Only
    /// the part this crate can honour is here — the destination and the
    /// re-issue to the figures. The original also runs `Dest_FindReachableNear`
    /// on the cell and `Order_StopShortOfTarget` for a missile unit, and sets a
    /// 64-frame order lock when the unit is already in melee; the lock is
    /// reproduced because `BattleUnit_JoinMelee` reads it.
    pub fn order_unit(&mut self, unit: usize, x: u8, y: u8) {
        if unit == 0 || unit > MAX_UNITS || !self.units.get(unit).is_live() {
            return;
        }
        {
            let u = self.units.get_mut(unit);
            u.target_x = x as i16;
            u.target_y = y as i16;
            u.withdrawing = false;
            // > It was set by nothing here, which left one figure state
            // > permanently out of a player's reach. `Formation_SendFigure`
            // > refuses to re-issue a destination to a figure **in state 9**
            // > *unless the gate is set* — that is what stops a reform pulling
            // > men out of the ditch every 500 frames — so with the gate never
            // > set, a man ordered to fill a moat in could never be ordered to
            // > do anything else for the rest of the battle. `[V]` — the
            // > original sets it unconditionally at `0x004A...`'s order tail and
            // > clears it again only for a *non-human* unit under one arm.
            u.reform_gate = true;
            if u.in_melee {
                u.order_lock = crate::unit::ORDER_LOCK;
            }
        }
        self.pour_on_order(unit, x as i16, y as i16);
        self.clear_straff(unit);
        self.reform_unit(unit);
    }

    pub fn order_side(&mut self, side: Side, x: u8, y: u8) {
        for u in 1..=MAX_UNITS {
            if self.units.get(u).is_live() && self.units.get(u).side == side {
                self.order_unit(u, x, y);
            }
        }
    }

    pub fn unit_of(&self, fighter: usize) -> usize {
        self.sim.figures[self.fighters[fighter].sim].unit as usize
    }

    pub fn home(&self, side: Side) -> (u8, u8) {
        if side == SIDE_A {
            self.field.home_side0
        } else {
            self.field.home_side4
        }
    }

    /// `Deploy_SlotForUnit` (`0x0048169E`): the `n`-th unit of a side takes the
    /// `n`-th of that side's twelve marker slots.
    ///
    /// The two tables are adjacent in the original's `.data` — side 0's twelve
    /// at `0x00553150` and side 4's twelve at `0x005531B0` — and the routine
    /// **does not bound-check the ordinal**.
    pub(crate) fn deploy_slot(&self, ordinal: usize, side: Side) -> (u8, u8) {
        let base = if side == SIDE_A { 0 } else { 12 };
        let idx = (base + ordinal).min(23);
        if idx < 12 {
            self.field.deploy_side0[idx]
        } else {
            self.field.deploy_side4[idx - 12]
        }
    }

    /// `FUN_0046E70E` — the placement search `BattleMan_Create` uses.
    pub(crate) fn free_cell_near(&self, x: i32, y: i32) -> Option<(u8, u8)> {
        for radius in 0..20i32 {
            let x0 = (x - radius).max(0);
            let y0 = (y - radius).max(0);
            let x1 = (x + radius + 1).min(DIM as i32);
            let y1 = (y + radius + 1).min(DIM as i32);
            for cy in y0..y1 {
                for cx in x0..x1 {
                    let i = cy as usize * DIM + cx as usize;
                    if self.occupant[i].is_none() && !self.blocked[i] {
                        return Some((cx as u8, cy as u8));
                    }
                }
            }
        }
        None
    }

    pub(crate) fn sync_positions(&mut self) {
        self.positions.clear();
        self.positions
            .extend(self.fighters.iter().map(|f| (f.x, f.y)));
    }

    /// `DAT_0053F028` / `DAT_00553C58` — **a side's living men, and the number
    /// the player is looking at.**
    ///
    /// `Battle_CountMenByType` recomputes both every frame, and
    /// `00420000.c:1072` draws them side by side on the battle HUD with
    /// `Ui_DrawNumberRight`. So the two counters that decide the battle are the
    /// two numbers on the screen, which is as good a confirmation as this layer
    /// offers. **[V]**
    pub fn men_of_side(&self, side: Side) -> u32 {
        self.sim
            .figures
            .iter()
            .filter(|f| f.side == side && f.is_alive() && f.troop.index() < 7)
            .map(|f| f.men as u32)
            .sum()
    }

    /// **Is the battle over, and who holds the field** — `FUN_00477DFC`
    /// (`0x00477DFC`), the per-frame outcome test.
    ///
    /// ```c
    /// if (DAT_0056d5c8 == 0) {
    ///     if      (menA < 1) { g_battleLoser = g_battleArmyB; }  /* B holds the field */
    ///     else if (menB < 1) { g_battleLoser = g_battleArmyA; }
    ///     else if (siege)    { ...three more arms... }
    /// } else {                                    /* a side withdrew */
    ///     g_battleLoser = (A.owner == withdrawer) ? B : A;
    /// }
    /// ```
    ///
    /// — remembering that `g_battleLoser` holds the **winner**, which this is a
    /// fourth site to confirm: `FUN_00478419` maps
    /// `g_localPlayer == g_units[g_battleLoser].owner` onto `L2.eng` group 82's
    /// *"won"* pair.
    pub fn conclusion(&self) -> Option<Conclusion> {
        if let Some(side) = self.withdrawn {
            return Some(Conclusion { winner: other_side(side), cause: End::Withdrawal });
        }
        if self.men_of_side(SIDE_B) < 1 {
            return Some(Conclusion { winner: SIDE_A, cause: End::Annihilation });
        }
        if self.men_of_side(SIDE_A) < 1 {
            return Some(Conclusion { winner: SIDE_B, cause: End::Annihilation });
        }
        if self.siege.is_siege {
            // `00470000.c:3344-3369`: the failed-assault arm runs after the
            // break-in arm in the same frame, and its `g_battleLoser` stands.
            if self.ai.breach_score == 0
                && self.ai.siege_engine_count == 0
                && self.siege.failed_assault_ends()
            {
                return Some(Conclusion { winner: SIDE_A, cause: End::AssaultFailed });
            }
            // `:3344-3347`: `g_castleEntered` credits `g_battleArmyA`, side 4.
            if self.siege.broke_in {
                return Some(Conclusion { winner: SIDE_B, cause: End::BrokeIn });
            }
        }
        None
    }

    pub(super) fn assault_repulsed(&mut self) {
        if !self.siege.is_siege
            || self.siege.failed_assault_ends()
            || self.ai.breach_score != 0
            || self.ai.siege_engine_count != 0
        {
            return;
        }
        self.ai.breach_score = ASSAULT_REPEAT_SCORE;
        self.ai.approach_score = ASSAULT_REPEAT_SCORE;
    }

    /// A side leaves the field — `DAT_0056D5C8` and `DAT_005656F8`.
    pub fn withdraw(&mut self, side: Side) {
        self.withdrawn = Some(side);
    }

    pub fn withdrawal(&self) -> Option<Side> {
        self.withdrawn
    }

    /// `g_battleWithdrawal = 0` — `Battle_Resume` (`0x00477C89`),
    /// `00470000.c:3220`. Neither `0x0056D5C8` nor `0x005656F8` is in
    /// `g_saveBlocks` (`0x004DE960`), so a load cannot carry a withdrawal.
    pub fn clear_withdrawal(&mut self) {
        self.withdrawn = None;
    }

    pub fn men_per_figure(&self, side: Side) -> u16 {
        self.men_per_figure[usize::from(side != SIDE_A)]
    }

    /// `g_battleSizeClass`, read by the selection marker `FUN_004BE070`.
    pub fn size_class(&self) -> u8 {
        self.size_class
    }

    /// **The casualty readback.** Living men of `side`, by
    /// [`Troop::index`] — the eleven counts a `g_units` record carries at
    /// `+0x16C`.
    pub fn survivors(&self, side: Side) -> [u32; 11] {
        let mut out = [0u32; 11];
        for f in &self.sim.figures {
            if f.side == side && f.is_alive() && !f.mercenary {
                out[f.troop.index()] += f.men as u32;
            }
        }
        out
    }

    /// The band's living men — `Battle_WriteBackCasualties` (`0x0047F474`):
    /// `if (mercenary) mercMen += men` in place of the troop slot.
    pub fn band_survivors(&self, side: Side) -> u32 {
        let alive = |f: &&crate::Figure| f.side == side && f.is_alive() && f.mercenary;
        self.sim.figures.iter().filter(alive).map(|f| f.men as u32).sum()
    }

    pub fn men(&self, side: Side) -> u32 {
        self.sim.men(side)
    }

    pub fn is_alive(&self, i: usize) -> bool {
        self.sim.figures[self.fighters[i].sim].is_alive()
    }

    /// **The body has been cleared away** — the corpse state has counted
    /// `+0x173` out and the original frees the record here. Ours keeps the
    /// index, so nothing may draw it from this frame on.
    ///
    /// This is what stops a spent pot of oil holding its pouring picture for
    /// the rest of the battle: `FUN_0047A814` puts it in state 2, and state 2
    /// runs out. `docs/battle.md` §17.8.
    pub fn corpse_gone(&self, i: usize) -> bool {
        let f = &self.fighters[i];
        f.anim == Motion::Dying && f.corpse > f.corpse_frames()
    }

    pub fn living(&self, side: Side) -> usize {
        self.sim.living(side)
    }

    pub fn is_decided(&self) -> bool {
        self.sim.is_decided()
    }

}
